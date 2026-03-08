# Storage Interface

This document defines the Rust storage trait that the gateway uses to interact with ScyllaDB, the CQL queries behind each operation, and the data flow between the two.

## Architecture

The gateway never speaks CQL directly in handler code. All database access goes through a `Storage` trait, which is implemented by `ScyllaStorage`. This gives us:

- Testability — mock the trait in unit tests without a database.
- Separation of concerns — handlers deal with S3 semantics, the storage layer deals with CQL.
- Future flexibility — swap the backend without touching handler code.

```
S3 Handler → Storage trait → ScyllaStorage → ScyllaDB
                                  ↓
                            scylla-rust-driver
```

## Storage Trait

```rust
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>>;

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    // Buckets
    async fn create_bucket(&self, params: CreateBucketParams) -> Result<(), StorageError>;
    async fn delete_bucket(&self, bucket: &str) -> Result<(), StorageError>;
    async fn head_bucket(&self, bucket: &str) -> Result<BucketMeta, StorageError>;
    async fn list_buckets(&self, owner: &str) -> Result<Vec<BucketMeta>, StorageError>;
    async fn get_bucket_versioning(&self, bucket: &str) -> Result<VersioningState, StorageError>;
    async fn put_bucket_versioning(&self, bucket: &str, state: VersioningState) -> Result<(), StorageError>;
    async fn get_bucket_policy(&self, bucket: &str) -> Result<Option<String>, StorageError>;
    async fn put_bucket_policy(&self, bucket: &str, policy: &str) -> Result<(), StorageError>;
    async fn get_bucket_lifecycle(&self, bucket: &str) -> Result<Option<String>, StorageError>;
    async fn put_bucket_lifecycle(&self, bucket: &str, lifecycle: &str) -> Result<(), StorageError>;
    async fn get_bucket_location(&self, bucket: &str) -> Result<String, StorageError>;

    // Objects
    async fn put_object(&self, params: PutObjectParams, body: ByteStream) -> Result<PutObjectResult, StorageError>;
    async fn get_object(&self, params: GetObjectParams) -> Result<GetObjectResult, StorageError>;
    async fn head_object(&self, params: HeadObjectParams) -> Result<ObjectMeta, StorageError>;
    async fn delete_object(&self, params: DeleteObjectParams) -> Result<DeleteObjectResult, StorageError>;
    async fn copy_object(&self, params: CopyObjectParams) -> Result<CopyObjectResult, StorageError>;
    async fn list_objects_v2(&self, params: ListObjectsV2Params) -> Result<ListObjectsV2Result, StorageError>;
    async fn list_object_versions(&self, params: ListVersionsParams) -> Result<ListVersionsResult, StorageError>;

    // Multipart
    async fn create_multipart_upload(&self, params: CreateMultipartParams) -> Result<String, StorageError>;
    async fn upload_part(&self, params: UploadPartParams, body: ByteStream) -> Result<String, StorageError>;
    async fn complete_multipart_upload(&self, params: CompleteMultipartParams) -> Result<CompleteMultipartResult, StorageError>;
    async fn abort_multipart_upload(&self, params: AbortMultipartParams) -> Result<(), StorageError>;
    async fn list_multipart_uploads(&self, params: ListMultipartUploadsParams) -> Result<ListMultipartUploadsResult, StorageError>;
    async fn list_parts(&self, params: ListPartsParams) -> Result<ListPartsResult, StorageError>;
}
```

## Types

```rust
#[derive(Debug, Clone)]
pub enum VersioningState {
    Disabled,
    Enabled,
    Suspended,
}

#[derive(Debug, Clone)]
pub enum Consistency {
    Eventual,
    Quorum,
    Strong,
    All,
}

#[derive(Debug, Clone)]
pub struct BucketMeta {
    pub name: String,
    pub owner: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub region: String,
}

#[derive(Debug, Clone)]
pub struct ObjectMeta {
    pub bucket: String,
    pub key: String,
    pub version_id: uuid::Uuid,
    pub etag: String,
    pub size: u64,
    pub content_type: String,
    pub user_metadata: HashMap<String, String>,
    pub chunk_count: u32,
    pub delete_marker: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateBucketParams {
    pub name: String,
    pub owner: String,
    pub region: String,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct PutObjectParams {
    pub bucket: String,
    pub key: String,
    pub content_type: String,
    pub content_length: u64,
    pub user_metadata: HashMap<String, String>,
    pub if_none_match: bool,        // If-None-Match: * (prevent overwrite)
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct PutObjectResult {
    pub etag: String,
    pub version_id: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct GetObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<uuid::Uuid>,
    pub range: Option<ByteRange>,
    pub if_match: Option<String>,
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<chrono::DateTime<chrono::Utc>>,
    pub if_unmodified_since: Option<chrono::DateTime<chrono::Utc>>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct GetObjectResult {
    pub meta: ObjectMeta,
    pub body: ByteStream,
    pub content_range: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,   // None means to end of object
}

#[derive(Debug, Clone)]
pub struct HeadObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<uuid::Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct DeleteObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<uuid::Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct DeleteObjectResult {
    pub version_id: uuid::Uuid,
    pub delete_marker: bool,
}

#[derive(Debug, Clone)]
pub struct CopyObjectParams {
    pub source_bucket: String,
    pub source_key: String,
    pub source_version_id: Option<uuid::Uuid>,
    pub dest_bucket: String,
    pub dest_key: String,
    pub metadata_directive: MetadataDirective,
    pub user_metadata: HashMap<String, String>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub enum MetadataDirective {
    Copy,
    Replace,
}

#[derive(Debug, Clone)]
pub struct CopyObjectResult {
    pub etag: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ListObjectsV2Params {
    pub bucket: String,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub max_keys: u32,
    pub start_after: Option<String>,
    pub continuation_token: Option<String>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListObjectsV2Result {
    pub contents: Vec<ListEntry>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListEntry {
    pub key: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub etag: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct ListVersionsParams {
    pub bucket: String,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub max_keys: u32,
    pub key_marker: Option<String>,
    pub version_id_marker: Option<uuid::Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListVersionsResult {
    pub versions: Vec<VersionEntry>,
    pub delete_markers: Vec<DeleteMarkerEntry>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_key_marker: Option<String>,
    pub next_version_id_marker: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub key: String,
    pub version_id: uuid::Uuid,
    pub is_latest: bool,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub etag: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct DeleteMarkerEntry {
    pub key: String,
    pub version_id: uuid::Uuid,
    pub is_latest: bool,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateMultipartParams {
    pub bucket: String,
    pub key: String,
    pub content_type: String,
    pub user_metadata: HashMap<String, String>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct UploadPartParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: uuid::Uuid,
    pub part_number: u32,
    pub content_length: u64,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct PartInfo {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone)]
pub struct CompleteMultipartParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: uuid::Uuid,
    pub parts: Vec<PartInfo>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct CompleteMultipartResult {
    pub etag: String,
    pub version_id: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct AbortMultipartParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: uuid::Uuid,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListMultipartUploadsParams {
    pub bucket: String,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub max_uploads: u32,
    pub key_marker: Option<String>,
    pub upload_id_marker: Option<uuid::Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListMultipartUploadsResult {
    pub uploads: Vec<MultipartUploadEntry>,
    pub is_truncated: bool,
    pub next_key_marker: Option<String>,
    pub next_upload_id_marker: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub struct MultipartUploadEntry {
    pub key: String,
    pub upload_id: uuid::Uuid,
    pub initiated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ListPartsParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: uuid::Uuid,
    pub max_parts: u32,
    pub part_number_marker: Option<u32>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListPartsResult {
    pub parts: Vec<PartDetail>,
    pub is_truncated: bool,
    pub next_part_number_marker: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct PartDetail {
    pub part_number: u32,
    pub etag: String,
    pub size: u64,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("bucket not found: {0}")]
    BucketNotFound(String),

    #[error("bucket already exists: {0}")]
    BucketAlreadyExists(String),

    #[error("bucket not empty: {0}")]
    BucketNotEmpty(String),

    #[error("object not found: {bucket}/{key}")]
    ObjectNotFound { bucket: String, key: String },

    #[error("version not found: {bucket}/{key}/{version_id}")]
    VersionNotFound { bucket: String, key: String, version_id: uuid::Uuid },

    #[error("precondition failed")]
    PreconditionFailed,

    #[error("not modified")]
    NotModified,

    #[error("invalid range")]
    InvalidRange,

    #[error("upload not found: {bucket}/{key}/{upload_id}")]
    UploadNotFound { bucket: String, key: String, upload_id: uuid::Uuid },

    #[error("invalid part: {part_number}")]
    InvalidPart { part_number: u32 },

    #[error("invalid part order")]
    InvalidPartOrder,

    #[error("database error: {0}")]
    Database(#[from] scylla::errors::QueryError),

    #[error("internal error: {0}")]
    Internal(String),
}
```

`StorageError` maps directly to S3 error codes in the handler layer. The handler converts `StorageError::BucketNotFound` to a `404 NoSuchBucket` XML response, etc.

## ScyllaStorage Implementation

### Connection Setup

```rust
use scylla::{Session, SessionBuilder};
use scylla::load_balancing::DefaultPolicy;
use scylla::transport::Compression;

pub struct ScyllaStorage {
    session: Session,
    default_consistency: Consistency,
}

impl ScyllaStorage {
    pub async fn new(config: ScyllaConfig) -> Result<Self, StorageError> {
        let policy = DefaultPolicy::builder()
            .prefer_datacenter(config.local_dc.clone())
            .token_aware(true)
            .build();

        let session = SessionBuilder::new()
            .known_nodes(&config.contact_points)
            .default_execution_profile_handle(
                ExecutionProfile::builder()
                    .load_balancing_policy(policy)
                    .compression(Some(Compression::Lz4))
                    .build()
                    .into_handle(),
            )
            .build()
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(Self {
            session,
            default_consistency: config.default_consistency,
        })
    }
}
```

**Key driver settings:**
- `token_aware(true)` — routes queries to the node that owns the partition, avoiding extra network hops.
- `prefer_datacenter` — reads go to local DC first.
- `Compression::Lz4` — compresses CQL frames. Significant for chunk data.

### Consistency Mapping

```rust
impl Consistency {
    fn to_write_cl(&self) -> scylla::frame::types::Consistency {
        match self {
            Consistency::Eventual => Consistency::LocalOne,
            Consistency::Quorum => Consistency::LocalQuorum,
            Consistency::Strong => Consistency::EachQuorum,
            Consistency::All => Consistency::All,
        }
    }

    fn to_read_cl(&self) -> scylla::frame::types::Consistency {
        match self {
            Consistency::Eventual => Consistency::LocalOne,
            Consistency::Quorum => Consistency::LocalQuorum,
            Consistency::Strong => Consistency::LocalQuorum,
            Consistency::All => Consistency::All,
        }
    }
}
```

Note: `strong` uses `EACH_QUORUM` for writes (every DC acknowledges) but `LOCAL_QUORUM` for reads — since the write was confirmed everywhere, a local quorum read is sufficient to see it.

### Prepared Statements

All queries use prepared statements. They are prepared once at startup and reused for every request. This avoids per-request parsing overhead and enables the driver's token-aware routing.

```rust
pub struct PreparedQueries {
    // Buckets
    create_bucket: PreparedStatement,
    delete_bucket: PreparedStatement,
    get_bucket: PreparedStatement,
    list_buckets: PreparedStatement,
    update_bucket_versioning: PreparedStatement,
    update_bucket_policy: PreparedStatement,
    update_bucket_lifecycle: PreparedStatement,

    // Objects
    put_object_meta: PreparedStatement,
    get_object_latest: PreparedStatement,
    get_object_version: PreparedStatement,
    delete_object_meta: PreparedStatement,

    // Chunks
    put_chunk: PreparedStatement,
    get_chunk: PreparedStatement,
    delete_chunks: PreparedStatement,

    // Listings
    put_listing: PreparedStatement,
    delete_listing: PreparedStatement,
    list_by_prefix: PreparedStatement,

    // Multipart
    create_upload: PreparedStatement,
    get_upload: PreparedStatement,
    update_upload_status: PreparedStatement,
    put_part: PreparedStatement,
    get_parts: PreparedStatement,
    delete_parts: PreparedStatement,
    list_uploads: PreparedStatement,
}
```

## CQL Queries by Operation

### PutObject

```
1. Generate version_id (timeuuid)
2. Compute ETag (MD5) while streaming body
3. For each 1 MB chunk:
     INSERT INTO rux.chunks (bucket, key, version_id, chunk_index, data, checksum)
     VALUES (?, ?, ?, ?, ?, ?)
4. INSERT INTO rux.objects (bucket, key, version_id, etag, size, content_type,
     user_metadata, chunk_count, delete_marker, created_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, false, ?)
5. INSERT INTO rux.listings (bucket, prefix, name, is_common_prefix, version_id,
     size, etag, last_modified)
     VALUES (?, ?, ?, false, ?, ?, ?, ?)
```

Chunks are written as they stream in — the gateway does not buffer the full object. The metadata row is written last so the object becomes visible atomically.

With `If-None-Match: *` on a non-versioned bucket:
```
INSERT INTO rux.objects (...) VALUES (...) IF NOT EXISTS
```

**Listing prefix extraction:** For key `photos/vacation/beach.jpg` with delimiter `/`:
- Insert listing with `prefix = "photos/vacation/"`, `name = "photos/vacation/beach.jpg"`
- Insert listing with `prefix = "photos/"`, `name = "photos/vacation/"`, `is_common_prefix = true`
- Insert listing with `prefix = ""`, `name = "photos/"`, `is_common_prefix = true`

This allows `ListObjectsV2` at any prefix depth to find results in a single partition query.

### GetObject

```
1. SELECT * FROM rux.objects
     WHERE bucket = ? AND key = ?
     LIMIT 1
   (returns latest version due to DESC clustering)

   OR for specific version:
   SELECT * FROM rux.objects
     WHERE bucket = ? AND key = ? AND version_id = ?

2. Check conditional headers (If-Match, If-None-Match, etc.)

3. For each chunk (parallel, configurable concurrency of 16):
     SELECT data FROM rux.chunks
       WHERE bucket = ? AND key = ? AND version_id = ? AND chunk_index = ?

4. Stream chunks to client in order
```

For range requests, the gateway calculates:
- `start_chunk = range_start / CHUNK_SIZE`
- `end_chunk = range_end / CHUNK_SIZE`
- `start_offset = range_start % CHUNK_SIZE` (byte offset within first chunk)
- `end_offset = range_end % CHUNK_SIZE` (byte offset within last chunk)

Only the required chunks are read. The first and last chunks are trimmed to the requested byte range.

### DeleteObject

**Non-versioned:**
```
1. SELECT version_id, chunk_count FROM rux.objects
     WHERE bucket = ? AND key = ? LIMIT 1

2. For each chunk:
     DELETE FROM rux.chunks
       WHERE bucket = ? AND key = ? AND version_id = ? AND chunk_index = ?

3. DELETE FROM rux.objects
     WHERE bucket = ? AND key = ? AND version_id = ?

4. DELETE FROM rux.listings
     WHERE bucket = ? AND prefix = ? AND name = ?
```

**Versioned (no versionId — insert delete marker):**
```
1. Generate new timeuuid

2. INSERT INTO rux.objects (bucket, key, version_id, delete_marker, created_at)
     VALUES (?, ?, ?, true, ?)
```

**Versioned (with versionId — permanent delete):**
```
1. SELECT chunk_count FROM rux.objects
     WHERE bucket = ? AND key = ? AND version_id = ?

2. Delete chunks (same as non-versioned)

3. DELETE FROM rux.objects
     WHERE bucket = ? AND key = ? AND version_id = ?

4. If this was the latest version, update the listing to point to the
   next version, or delete the listing if no versions remain.
```

### ListObjectsV2

```
SELECT name, is_common_prefix, version_id, size, etag, last_modified
  FROM rux.listings
  WHERE bucket = ? AND prefix = ?
  AND name > ?          -- start_after or continuation_token
  ORDER BY name ASC
  LIMIT ?               -- max_keys + 1 (to detect truncation)
```

The gateway separates results into `Contents` (where `is_common_prefix = false`) and `CommonPrefixes` (where `is_common_prefix = true`). If more than `max_keys` results are returned, `IsTruncated` is set and `NextContinuationToken` is the last key returned (base64 encoded).

### CompleteMultipartUpload

```
1. SELECT * FROM rux.multipart_uploads
     WHERE bucket = ? AND key = ? AND upload_id = ?
   Verify status = 'in_progress'

2. UPDATE rux.multipart_uploads SET status = 'completing'
     WHERE bucket = ? AND key = ? AND upload_id = ?

3. For each part in order:
     SELECT data FROM rux.multipart_parts
       WHERE bucket = ? AND upload_id = ? AND part_number = ?
   Verify ETag matches

4. Re-chunk all part data into 1 MB chunks

5. Write chunks:
     INSERT INTO rux.chunks (...) VALUES (...)

6. Write metadata:
     INSERT INTO rux.objects (...) VALUES (...)

7. Write listing:
     INSERT INTO rux.listings (...) VALUES (...)

8. Delete parts:
     DELETE FROM rux.multipart_parts
       WHERE bucket = ? AND upload_id = ?

9. UPDATE rux.multipart_uploads SET status = 'completed'
     WHERE bucket = ? AND key = ? AND upload_id = ?
```

The `completing` status acts as a lock — if the gateway crashes mid-completion, another gateway can detect this and either resume or abort. Parts are read and re-chunked in a streaming fashion to avoid buffering the full object in memory.

## Connection Pooling and Concurrency

The `scylla-rust-driver` manages a connection pool per node internally. Key tuning parameters:

| Parameter | Default | Description |
|---|---|---|
| `pool_size` | 1 per shard | Connections per ScyllaDB shard (shard-aware routing) |
| `request_timeout` | 12s | Per-query timeout |
| `retry_policy` | DefaultRetryPolicy | Retries on timeouts and overloaded errors |
| `speculative_execution` | disabled | Can be enabled for latency-sensitive reads |

For chunk reads, the gateway spawns up to 16 concurrent queries using `futures::stream::buffer_unordered`. Chunks are yielded in order to the response stream using a reorder buffer.

## Schema Migrations

Schema changes are applied by the gateway on startup. The gateway checks a version table and applies any pending migrations:

```cql
CREATE TABLE IF NOT EXISTS rux.schema_version (
    version int,
    applied_at timestamp,
    description text,
    PRIMARY KEY (version)
);
```

Migrations are idempotent CQL statements embedded in the gateway binary. On startup, the gateway reads the current schema version, applies any unapplied migrations in order, and proceeds. Multiple gateways starting simultaneously is safe — `INSERT IF NOT EXISTS` on the version table prevents duplicate application.
