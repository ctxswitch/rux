# Distributed Storage System Design

Rux is a globally distributed, S3-compatible object storage system built on ScyllaDB. Rux itself is a stateless API gateway that translates S3 requests into ScyllaDB queries, delegating all distributed systems concerns — replication, consistency, scaling, failure recovery — to ScyllaDB's proven infrastructure.

## Architecture Overview

```
              Client Request (S3 API)
                      │
                      ▼
            ┌───────────────────┐
            │   Rux Gateway     │  ← stateless S3→CQL translation
            │   (Deployment)    │  ← horizontally scaled
            └────────┬──────────┘
                     │  CQL
                     ▼
            ┌───────────────────┐
            │    ScyllaDB       │  ← multi-datacenter, handles all
            │    Cluster        │     distribution, replication,
            │                   │     consistency, and scaling
            └───────────────────┘

    us-east-1                          eu-west-1
┌──────────────────┐            ┌──────────────────┐
│  Rux Gateways    │            │  Rux Gateways    │
│  ┌────┐ ┌────┐  │            │  ┌────┐ ┌────┐  │
│  │ GW │ │ GW │  │            │  │ GW │ │ GW │  │
│  └──┬─┘ └─┬──┘  │            │  └──┬─┘ └─┬──┘  │
│     │     │     │            │     │     │     │
│  ScyllaDB Nodes  │◄──────────►│  ScyllaDB Nodes  │
│  ┌───┐┌───┐┌───┐│  async     │  ┌───┐┌───┐┌───┐│
│  │ S ││ S ││ S ││  repl      │  │ S ││ S ││ S ││
│  └───┘└───┘└───┘│            │  └───┘└───┘└───┘│
└──────────────────┘            └──────────────────┘
```

### Component Roles

- **Rux Gateway**: Stateless HTTP service. Parses S3 requests (SigV4 auth, XML/JSON marshaling, multipart handling), translates them into CQL queries, streams object data to/from ScyllaDB, and assembles S3 responses. Runs as a Kubernetes Deployment, horizontally autoscaled.
- **ScyllaDB**: Handles all data storage, distribution, replication, consistency, and scaling. Deployed via the [scylla-operator](https://github.com/scylladb/scylla-operator) as a `ScyllaCluster` CRD. Multi-datacenter replication is configured natively.

## Data Model

### Keyspace Configuration

```cql
CREATE KEYSPACE rux WITH replication = {
    'class': 'NetworkTopologyStrategy',
    'us-east-1': 3,
    'eu-west-1': 3
};
```

`NetworkTopologyStrategy` ensures replicas are spread across racks within each datacenter. Adding or removing a region is a keyspace alteration.

### Schema

```cql
-- Bucket configuration
CREATE TABLE rux.buckets (
    name text,
    owner text,
    region text,                 -- creation region
    versioning text,             -- 'disabled', 'enabled', 'suspended'
    policy text,                 -- JSON bucket policy
    lifecycle text,              -- JSON lifecycle configuration
    created_at timestamp,
    PRIMARY KEY (name)
);

-- Object metadata
-- Partition by (bucket, key) for single-object lookups.
-- Cluster by version_id DESC so the latest version is always first.
CREATE TABLE rux.objects (
    bucket text,
    key text,
    version_id timeuuid,
    etag text,
    size bigint,
    content_type text,
    user_metadata map<text, text>,
    chunk_count int,
    delete_marker boolean,
    created_at timestamp,
    PRIMARY KEY ((bucket, key), version_id)
) WITH CLUSTERING ORDER BY (version_id DESC)
  AND gc_grace_seconds = 3600;

-- Object data chunks
-- Each chunk gets its own partition key so ScyllaDB distributes
-- chunks across nodes. This enables parallel reads/writes for
-- large objects and avoids hot partitions.
CREATE TABLE rux.chunks (
    bucket text,
    key text,
    version_id timeuuid,
    chunk_index int,
    data blob,                   -- up to 1 MB per chunk
    checksum text,               -- SHA-256 of chunk data
    PRIMARY KEY ((bucket, key, version_id, chunk_index))
);

-- Prefix listing index
-- Supports efficient ListObjectsV2 with prefix and delimiter.
-- Partition by (bucket, prefix) where prefix is the directory-like
-- component (e.g., "photos/" for key "photos/cat.jpg").
CREATE TABLE rux.listings (
    bucket text,
    prefix text,                 -- extracted directory prefix
    name text,                   -- full key or common prefix (for delimiter grouping)
    is_common_prefix boolean,    -- true if this is a delimiter-grouped prefix
    version_id timeuuid,
    size bigint,
    etag text,
    last_modified timestamp,
    PRIMARY KEY ((bucket, prefix), name)
) WITH CLUSTERING ORDER BY (name ASC);

-- Multipart upload tracking
CREATE TABLE rux.multipart_uploads (
    bucket text,
    key text,
    upload_id timeuuid,
    content_type text,
    user_metadata map<text, text>,
    status text,                 -- 'in_progress', 'completing', 'completed', 'aborted'
    initiated timestamp,
    PRIMARY KEY ((bucket, key), upload_id)
) WITH CLUSTERING ORDER BY (upload_id DESC);

-- Denormalized listing table for ListBuckets
CREATE TABLE rux.buckets_by_owner (
    owner text,
    name text,
    region text,
    created_at timestamp,
    PRIMARY KEY (owner, name)
) WITH CLUSTERING ORDER BY (name ASC);

-- Denormalized table for ListMultipartUploads
CREATE TABLE rux.active_multipart_uploads (
    bucket text,
    key text,
    upload_id timeuuid,
    initiated timestamp,
    PRIMARY KEY (bucket, key, upload_id)
);

-- Multipart upload parts
CREATE TABLE rux.multipart_parts (
    bucket text,
    upload_id timeuuid,
    part_number int,
    data blob,
    size bigint,
    etag text,
    created_at timestamp,
    PRIMARY KEY ((bucket, upload_id), part_number)
) WITH CLUSTERING ORDER BY (part_number ASC);
```

### Design Decisions

**Chunk size (1 MB)**: ScyllaDB performs best with cells under 1 MB. A 5 GB object produces ~5000 chunk rows. This is well within ScyllaDB's capability — chunk reads are sequential within a partition (all chunks share the same partition key), so reassembly is a single range scan.

**Listings table**: S3's `ListObjectsV2` requires prefix-based iteration with delimiter support. ScyllaDB is not optimized for arbitrary prefix scans across partitions, so the `listings` table denormalizes this. Every `PutObject` writes to both `objects` and `listings`; every `DeleteObject` removes from both. The write amplification is acceptable because listing is a critical S3 operation that must be fast.

**Separate multipart tables**: Multipart upload state is kept in its own tables rather than mixing with `objects`. Parts are stored with their data inline — on `CompleteMultipartUpload`, parts are assembled into chunks and written to the `chunks` table, then part rows are deleted.

**Versioning**: `timeuuid` clustering with `DESC` order means the latest version is always the first row returned. Non-versioned buckets use LWT (lightweight transactions) on the `objects` table to enforce single-version semantics.

### TTL and Lifecycle

ScyllaDB's native TTL support handles object expiration. When a lifecycle rule applies, the gateway sets a TTL on the object's rows in `objects`, `chunks`, and `listings`:

```cql
INSERT INTO rux.objects (...) VALUES (...) USING TTL 2592000;  -- 30 days
```

ScyllaDB automatically tombstones expired rows. A background process in the gateway reconciles lifecycle rules on existing objects (applying TTLs retroactively when rules change).

## Consistency Model

Rux maps the `X-Rux-Consistency` header to ScyllaDB consistency levels:

| `X-Rux-Consistency` | CQL Write CL | CQL Read CL | Behavior |
|---|---|---|---|
| `eventual` (default) | `LOCAL_ONE` | `LOCAL_ONE` | Fastest. Write acknowledged by one local replica. Read may return stale data. |
| `quorum` | `LOCAL_QUORUM` | `LOCAL_QUORUM` | Write/read acknowledged by majority of local replicas. Strong within a datacenter. |
| `strong` | `EACH_QUORUM` | `LOCAL_QUORUM` | Write acknowledged by a quorum in every datacenter. Read from local quorum guarantees seeing the latest global write. |
| `all` | `ALL` | `ALL` | Write/read acknowledged by all replicas everywhere. Highest latency, strongest guarantee. |

### Conflict Resolution

ScyllaDB uses **last-write-wins (LWW)** based on write timestamps, which aligns with S3's PUT semantics. For concurrent writes to the same key:
- Without versioning: the write with the highest timestamp wins. This is ScyllaDB's default and requires no application-level conflict resolution.
- With versioning: each write produces a distinct `timeuuid` version. All versions are retained and accessible via `ListObjectVersions`.

### Lightweight Transactions (LWT)

For operations that require conditional semantics, the gateway uses ScyllaDB's LWT (Paxos-based):

- **Non-versioned PutObject with `If-None-Match: *`**: `INSERT ... IF NOT EXISTS`
- **DeleteObject with version check**: `DELETE ... IF version_id = ?`
- **CreateBucket**: `INSERT ... IF NOT EXISTS`

LWTs are expensive (4 round trips vs 1) and should be used sparingly. Most S3 operations don't need them.

## Request Flows

### PUT Object

```
Client              Rux Gateway                    ScyllaDB
  │                      │                            │
  │── PUT /bucket/key ──►│                            │
  │   (with body)        │                            │
  │                      │── chunk body into 1MB ────►│
  │                      │   segments, stream to      │
  │                      │   rux.chunks               │
  │                      │                            │
  │                      │── INSERT rux.objects ──────►│
  │                      │   (metadata)               │
  │                      │                            │
  │                      │── INSERT rux.listings ────►│
  │                      │   (prefix index)           │
  │                      │                            │
  │                      │◄── ack (CL met) ──────────│
  │◄── 200 OK ──────────│                            │
  │   (ETag)             │                            │
```

For large objects, chunks are streamed as they arrive — the gateway doesn't buffer the full object in memory. The `objects` metadata row is written last, making the object visible atomically (readers won't see an object until all chunks are written and the metadata row exists).

### GET Object

```
Client              Rux Gateway                    ScyllaDB
  │                      │                            │
  │── GET /bucket/key ──►│                            │
  │                      │── SELECT rux.objects ─────►│
  │                      │   WHERE bucket=? AND key=? │
  │                      │   LIMIT 1 (latest version) │
  │                      │◄── metadata ──────────────│
  │                      │                            │
  │                      │── SELECT rux.chunks ──────►│
  │                      │   WHERE bucket=? AND key=? │
  │                      │   AND version_id=?         │
  │◄── 200 OK ──────────│   (stream chunks)          │
  │   (stream body)      │◄── chunk data ────────────│
```

The gateway issues parallel reads for chunks across ScyllaDB nodes and streams them back to the client in order — no full object buffering. Since each chunk is its own partition, reads for a large object naturally fan out across the cluster. Range requests (`Range: bytes=X-Y`) are translated to the appropriate `chunk_index` values.

### ListObjectsV2

```
Client              Rux Gateway                    ScyllaDB
  │                      │                            │
  │── GET /bucket?      ─►│                            │
  │   prefix=photos/     │── SELECT rux.listings ────►│
  │   delimiter=/        │   WHERE bucket=?           │
  │   max-keys=1000      │   AND prefix=?             │
  │                      │   ORDER BY name ASC        │
  │                      │   LIMIT 1000               │
  │                      │◄── listing rows ──────────│
  │◄── 200 OK ──────────│                            │
  │   (XML response)     │                            │
```

The `listings` table is partitioned by `(bucket, prefix)`, so this is a single-partition range scan — fast and efficient. The gateway handles delimiter grouping, continuation tokens (`start-after`), and XML response formatting.

### Multipart Upload

```
CreateMultipartUpload:
  → INSERT rux.multipart_uploads (status='in_progress')
  ← return upload_id

UploadPart (for each part):
  → INSERT rux.multipart_parts (data=part_body)
  ← return ETag

CompleteMultipartUpload:
  → SELECT all parts from rux.multipart_parts
  → reassemble into 1MB chunks
  → INSERT chunks into rux.chunks
  → INSERT metadata into rux.objects
  → INSERT into rux.listings
  → DELETE parts from rux.multipart_parts
  → UPDATE rux.multipart_uploads SET status='completed'
  ← return ETag
```

`CompleteMultipartUpload` is the most complex operation. It reads all parts, re-chunks them into 1 MB segments, writes the final object, and cleans up. This runs in the gateway and may take time for large objects. The `multipart_uploads` status field tracks progress for crash recovery — if a gateway crashes mid-completion, another gateway can resume or abort.

## S3 API Compatibility

### Supported Operations

**Bucket operations:**
- `CreateBucket`, `DeleteBucket`, `HeadBucket`, `ListBuckets`
- `GetBucketVersioning`, `PutBucketVersioning`
- `GetBucketLifecycleConfiguration`, `PutBucketLifecycleConfiguration`
- `GetBucketPolicy`, `PutBucketPolicy`
- `GetBucketLocation`

**Object operations:**
- `PutObject`, `GetObject`, `DeleteObject`, `HeadObject`
- `CopyObject`
- `ListObjectsV2`, `ListObjectVersions`
- `GetObjectAttributes`

**Multipart upload:**
- `CreateMultipartUpload`, `UploadPart`, `CompleteMultipartUpload`, `AbortMultipartUpload`
- `ListMultipartUploads`, `ListParts`

**Versioning:**
- Full version support — `GetObject?versionId=`, `DeleteObject?versionId=`

### Authentication

S3 Signature Version 4 (SigV4). Access keys and secret keys managed via Kubernetes Secrets. Bucket policies stored in the `buckets` table and evaluated in the gateway.

### Custom Headers

| Header | Direction | Description |
|---|---|---|
| `X-Rux-Consistency` | Request | `eventual`, `quorum`, `strong`, or `all` |
| `X-Rux-Source-Region` | Response | ScyllaDB datacenter that served the read |
| `X-Rux-Version-Id` | Response | `timeuuid` version of the object |

## Kubernetes Integration

### Deployment Topology

Each region runs in its own Kubernetes cluster with:
- **Rux Gateway**: `Deployment` with HPA, scaled by request rate.
- **ScyllaDB**: `ScyllaCluster` CRD managed by the scylla-operator.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rux-gateway
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: rux
          image: rux:latest
          ports:
            - containerPort: 8080  # S3 API
            - containerPort: 9090  # metrics
          env:
            - name: SCYLLA_CONTACT_POINTS
              value: "scylla-client.scylla.svc.cluster.local"
            - name: SCYLLA_LOCAL_DC
              value: "us-east-1"
            - name: RUX_DEFAULT_CONSISTENCY
              value: "eventual"
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8080
          readinessProbe:
            httpGet:
              path: /readyz
              port: 8080
---
apiVersion: scylla.scylladb.com/v1
kind: ScyllaCluster
metadata:
  name: rux-scylla
spec:
  agentVersion: latest
  version: 6.2
  datacenter:
    name: us-east-1
    racks:
      - name: a
        members: 3
        storage:
          capacity: 1Ti
          storageClassName: gp3
        resources:
          requests:
            cpu: 4
            memory: 16Gi
      - name: b
        members: 3
        storage:
          capacity: 1Ti
          storageClassName: gp3
        resources:
          requests:
            cpu: 4
            memory: 16Gi
```

### Multi-Datacenter Setup

ScyllaDB handles multi-datacenter replication natively. To add a new region:

1. Deploy a `ScyllaCluster` in the new region's Kubernetes cluster.
2. Configure ScyllaDB's seed nodes to point to existing datacenters.
3. Alter the keyspace to include the new datacenter:
   ```cql
   ALTER KEYSPACE rux WITH replication = {
       'class': 'NetworkTopologyStrategy',
       'us-east-1': 3,
       'eu-west-1': 3,
       'ap-southeast-1': 3
   };
   ```
4. Run `nodetool rebuild` on the new datacenter to stream data from existing ones.
5. Deploy Rux gateways in the new region.

Removing a region is the reverse: drain the datacenter, alter the keyspace, decommission nodes.

### Rux Operator

The Rux operator manages gateway-level concerns (ScyllaDB lifecycle is handled by the scylla-operator):

```yaml
apiVersion: rux.io/v1
kind: RuxCluster
metadata:
  name: global-store
spec:
  defaultConsistency: eventual
  regions:
    - name: us-east-1
      gatewayReplicas: 3
      scyllaClusterRef: rux-scylla
    - name: eu-west-1
      gatewayReplicas: 3
      scyllaClusterRef: rux-scylla-eu
---
apiVersion: rux.io/v1
kind: RuxBucket
metadata:
  name: my-bucket
spec:
  clusterRef: global-store
  versioning: enabled
  lifecycleRules:
    - prefix: "logs/"
      expirationDays: 30
```

The Rux operator:
- Reconciles `RuxBucket` CRDs into `buckets` table entries and applies lifecycle rules.
- Manages gateway Deployments and HPAs per region.
- Runs background tasks: lifecycle expiration, stale multipart upload cleanup, listing table consistency checks.
- Exposes Prometheus metrics: request latency, error rates, ScyllaDB query latency, chunk throughput.

## Scaling

### Gateway Scaling

Gateways are stateless — scaling is a standard Kubernetes HPA on request rate or CPU. No data migration, no coordination.

### ScyllaDB Scaling

ScyllaDB handles scaling natively:

**Adding nodes**: The scylla-operator scales the rack's `members` count. ScyllaDB automatically streams data to the new node, rebalancing token ranges. Reads and writes continue uninterrupted during streaming.

**Removing nodes**: The scylla-operator decommissions nodes via `nodetool decommission`. Data is streamed to remaining nodes before the pod terminates.

**Adding a datacenter**: As described in Multi-Datacenter Setup. ScyllaDB streams all data to the new datacenter via `nodetool rebuild`.

**Removing a datacenter**: Alter the keyspace to remove the datacenter's replication, then decommission all nodes in that datacenter.

No custom ring management, vnode leases, or handoff coordination required — ScyllaDB's built-in mechanisms handle all of this.

## Operational Considerations

### Write Amplification

Every `PutObject` writes to three tables (`objects`, `chunks`, `listings`). For a 10 MB object with 10 chunks, that's 12 CQL writes. With replication factor 3, that's 36 replica writes. This is the cost of denormalization for fast listing.

Mitigation:
- Use `UNLOGGED BATCH` for the `objects` + `listings` writes to reduce round trips (they share no partition key, so no atomicity guarantee, but the gateway can retry on partial failure).
- Chunks are written individually and streamed — no batching needed.

### Consistency of Listings

The `listings` table can become inconsistent with `objects` if a write partially fails (metadata written but listing not). A background reconciliation process in the gateway periodically scans for:
- Objects that exist in `objects` but not in `listings` (missing listing entries).
- Listing entries that point to non-existent objects (orphaned listings).

This runs per-bucket on a configurable schedule (default: hourly).

### Tombstone Management

S3 deletes produce tombstones in ScyllaDB. Rux uses aggressive tombstone cleanup:
- `gc_grace_seconds` is set to 1 hour. This is safe because ScyllaDB's built-in repair runs on a shorter cycle than the grace period — replicas converge well within this window.
- Repair must be scheduled to run more frequently than `gc_grace_seconds` (e.g., every 30 minutes) to prevent tombstone-purged deletes from being resurrected by an unrepaired replica. The scylla-operator's repair CRD handles this.
- For buckets with high churn, the operator can trigger compaction via `nodetool compact` to reclaim space immediately.

### Large Object Handling

The 1 MB chunk size means large objects produce many rows. For a 5 GB object:
- 5000 chunk rows in the `chunks` table.
- Each chunk is its own partition, so they're distributed across ScyllaDB nodes automatically.
- The gateway reads chunks in parallel (configurable concurrency, default 16) and reassembles them in order for streaming.

This avoids ScyllaDB's partition size limits entirely — every partition is exactly 1 MB. Write throughput also benefits: the gateway can stream incoming data to multiple ScyllaDB nodes simultaneously.

### Monitoring

Key metrics to expose:
- **Gateway**: S3 request latency (p50/p95/p99), error rate by operation, active connections, chunk throughput (MB/s).
- **ScyllaDB** (via scylla-operator monitoring): read/write latency, coordinator/replica latency, compaction pending, tombstone count, partition size, cross-DC replication lag.

## Open Questions

- **Encryption at rest**: ScyllaDB Enterprise supports encryption at rest. For open-source ScyllaDB, encryption would need to happen at the gateway level (encrypt chunks before writing) or at the volume level (LUKS on PVCs).
- **Server-side copy**: `CopyObject` for large objects currently requires reading all chunks and rewriting them. ScyllaDB has no server-side copy primitive. For same-bucket copies, could write new metadata pointing to the same chunk partition (requires refcounting).
- **Cost optimization**: ScyllaDB nodes need significant memory (16+ GB recommended). For cost-sensitive deployments, could consider tiered storage — hot data in ScyllaDB, cold data in object storage — but this adds complexity.
- **Rate limiting**: Per-tenant rate limiting at the gateway layer. Could use ScyllaDB's per-partition rate limiting feature or implement token-bucket in the gateway.
