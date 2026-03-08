mod bucket;
mod chunks;
mod listings;
mod migrations;
mod multipart;
mod object;
mod queries;

use scylla::client::execution_profile::ExecutionProfile;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::policies::load_balancing::DefaultPolicy;

use async_trait::async_trait;

use crate::config::ScyllaConfig;
use crate::storage::error::StorageError;
use crate::storage::types::*;
use crate::storage::{ByteStream, Storage};

use queries::PreparedQueries;

pub struct ScyllaStorage {
    session: Session,
    queries: PreparedQueries,
    default_consistency: Consistency,
}

impl ScyllaStorage {
    pub async fn new(
        config: &ScyllaConfig,
        default_consistency: Consistency,
    ) -> Result<Self, StorageError> {
        let policy = DefaultPolicy::builder()
            .prefer_datacenter(config.local_dc.clone())
            .token_aware(true)
            .build();

        let profile = ExecutionProfile::builder()
            .load_balancing_policy(policy)
            .build();

        let session: Session = SessionBuilder::new()
            .known_nodes(&config.contact_points)
            .default_execution_profile_handle(profile.into_handle())
            .build()
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let replication = config.replication_map().map_err(StorageError::Internal)?;
        migrations::run(&session, &replication).await?;

        let queries = PreparedQueries::prepare(&session).await?;

        Ok(Self {
            session,
            queries,
            default_consistency,
        })
    }
}

#[async_trait]
impl Storage for ScyllaStorage {
    async fn create_bucket(&self, _params: CreateBucketParams) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn delete_bucket(&self, _bucket: &str) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn head_bucket(&self, _bucket: &str) -> Result<BucketMeta, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn list_buckets(&self, _owner: &str) -> Result<Vec<BucketMeta>, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn get_bucket_versioning(&self, _bucket: &str) -> Result<VersioningState, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn put_bucket_versioning(
        &self,
        _bucket: &str,
        _state: VersioningState,
    ) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn get_bucket_policy(&self, _bucket: &str) -> Result<Option<String>, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn put_bucket_policy(&self, _bucket: &str, _policy: &str) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn get_bucket_lifecycle(&self, _bucket: &str) -> Result<Option<String>, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn put_bucket_lifecycle(
        &self,
        _bucket: &str,
        _lifecycle: &str,
    ) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn get_bucket_location(&self, _bucket: &str) -> Result<String, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }

    async fn put_object(
        &self,
        _params: PutObjectParams,
        _body: ByteStream,
    ) -> Result<PutObjectResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn get_object(&self, _params: GetObjectParams) -> Result<GetObjectResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn head_object(&self, _params: HeadObjectParams) -> Result<ObjectMeta, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn delete_object(
        &self,
        _params: DeleteObjectParams,
    ) -> Result<DeleteObjectResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn copy_object(
        &self,
        _params: CopyObjectParams,
    ) -> Result<CopyObjectResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn list_objects_v2(
        &self,
        _params: ListObjectsV2Params,
    ) -> Result<ListObjectsV2Result, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn list_object_versions(
        &self,
        _params: ListVersionsParams,
    ) -> Result<ListVersionsResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }

    async fn create_multipart_upload(
        &self,
        _params: CreateMultipartParams,
    ) -> Result<String, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn upload_part(
        &self,
        _params: UploadPartParams,
        _body: ByteStream,
    ) -> Result<String, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn complete_multipart_upload(
        &self,
        _params: CompleteMultipartParams,
    ) -> Result<CompleteMultipartResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn abort_multipart_upload(
        &self,
        _params: AbortMultipartParams,
    ) -> Result<(), StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn list_multipart_uploads(
        &self,
        _params: ListMultipartUploadsParams,
    ) -> Result<ListMultipartUploadsResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
    async fn list_parts(&self, _params: ListPartsParams) -> Result<ListPartsResult, StorageError> {
        Err(StorageError::Internal("not implemented".into()))
    }
}
