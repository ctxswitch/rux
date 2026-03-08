# rux

A globally distributed, S3-compatible object storage system built on ScyllaDB and Kubernetes. Rux is a stateless API gateway that translates S3 requests into ScyllaDB CQL queries, providing active-active multi-region storage with per-request tunable consistency.

## How It Works

```
Client (S3 API) → Rux Gateway (S3→CQL) → ScyllaDB (multi-datacenter)
```

Rux gateways are stateless and horizontally autoscaled. ScyllaDB handles all distributed systems concerns — replication, consistency, scaling, and failure recovery.

## Features

- **S3 compatible** — PutObject, GetObject, DeleteObject, ListObjectsV2, multipart uploads, versioning, lifecycle policies, SigV4 auth
- **Globally distributed** — active-active multi-region via ScyllaDB's native multi-datacenter replication
- **Tunable consistency** — per-request via `X-Rux-Consistency` header: `eventual`, `quorum`, `strong`, `all`
- **Kubernetes native** — stateless gateways as Deployments, ScyllaDB via scylla-operator, CRDs for bucket and cluster management
- **Horizontally scalable** — add gateway pods for throughput, add ScyllaDB nodes for capacity, add datacenters for new regions
- **Parallel I/O** — object chunks are distributed across ScyllaDB nodes for parallel reads and writes

## Architecture

Rux consists of two components:

- **Rux Gateway** — stateless Rust service that handles S3 HTTP protocol, authentication, chunking, and CQL query generation. Deployed as a Kubernetes Deployment with HPA.
- **ScyllaDB** — distributed database that stores object metadata, data chunks, and listing indexes. Deployed via the [scylla-operator](https://github.com/scylladb/scylla-operator).

Objects are chunked at 1 MB boundaries and each chunk is its own ScyllaDB partition, distributing data across the cluster automatically. Metadata and a denormalized listing index enable fast `ListObjectsV2` prefix queries.

## Consistency

| `X-Rux-Consistency` | Write CL | Read CL | Guarantee |
|---|---|---|---|
| `eventual` | `LOCAL_ONE` | `LOCAL_ONE` | Lowest latency, may read stale data |
| `quorum` | `LOCAL_QUORUM` | `LOCAL_QUORUM` | Strong within a datacenter |
| `strong` | `EACH_QUORUM` | `LOCAL_QUORUM` | Write confirmed in all datacenters |
| `all` | `ALL` | `ALL` | Strongest guarantee, highest latency |

## Status

Early development.

## Local ScyllaDB (Docker Compose)

Start ScyllaDB for local testing:

```bash
docker compose up -d scylla
docker compose ps
```

Run Rux against this container:

```bash
RUX_SCYLLA_CONTACT_POINTS=127.0.0.1 \
RUX_SCYLLA_LOCAL_DC=datacenter1 \
cargo run
```

Stop and clean up:

```bash
docker compose down
```

This setup is ephemeral: after `docker compose down`, the next `up` starts with empty data.
