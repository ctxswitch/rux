use uuid::Uuid;

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
    VersionNotFound {
        bucket: String,
        key: String,
        version_id: Uuid,
    },

    #[error("precondition failed")]
    PreconditionFailed,

    #[error("not modified")]
    NotModified,

    #[error("invalid range")]
    InvalidRange,

    #[error("upload not found: {bucket}/{key}/{upload_id}")]
    UploadNotFound {
        bucket: String,
        key: String,
        upload_id: Uuid,
    },

    #[error("invalid part: {part_number}")]
    InvalidPart { part_number: u32 },

    #[error("invalid part order")]
    InvalidPartOrder,

    #[error("database error: {0}")]
    Database(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<scylla::errors::PrepareError> for StorageError {
    fn from(e: scylla::errors::PrepareError) -> Self {
        StorageError::Database(e.to_string())
    }
}

impl From<scylla::errors::ExecutionError> for StorageError {
    fn from(e: scylla::errors::ExecutionError) -> Self {
        StorageError::Database(e.to_string())
    }
}
