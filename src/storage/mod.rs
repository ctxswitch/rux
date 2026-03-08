pub mod error;
pub mod scylla;
pub mod types;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

use error::StorageError;
use types::*;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>>;

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    // Buckets
    async fn create_bucket(&self, params: CreateBucketParams) -> Result<(), StorageError>;
    async fn delete_bucket(&self, bucket: &str) -> Result<(), StorageError>;
    async fn head_bucket(&self, bucket: &str) -> Result<BucketMeta, StorageError>;
    async fn list_buckets(&self, owner: &str) -> Result<Vec<BucketMeta>, StorageError>;
    async fn get_bucket_versioning(&self, bucket: &str) -> Result<VersioningState, StorageError>;
    async fn put_bucket_versioning(
        &self,
        bucket: &str,
        state: VersioningState,
    ) -> Result<(), StorageError>;
    async fn get_bucket_policy(&self, bucket: &str) -> Result<Option<String>, StorageError>;
    async fn put_bucket_policy(&self, bucket: &str, policy: &str) -> Result<(), StorageError>;
    async fn get_bucket_lifecycle(&self, bucket: &str) -> Result<Option<String>, StorageError>;
    async fn put_bucket_lifecycle(&self, bucket: &str, lifecycle: &str)
    -> Result<(), StorageError>;
    async fn get_bucket_location(&self, bucket: &str) -> Result<String, StorageError>;

    // Objects
    async fn put_object(
        &self,
        params: PutObjectParams,
        body: ByteStream,
    ) -> Result<PutObjectResult, StorageError>;
    async fn get_object(&self, params: GetObjectParams) -> Result<GetObjectResult, StorageError>;
    async fn head_object(&self, params: HeadObjectParams) -> Result<ObjectMeta, StorageError>;
    async fn delete_object(
        &self,
        params: DeleteObjectParams,
    ) -> Result<DeleteObjectResult, StorageError>;
    async fn copy_object(&self, params: CopyObjectParams)
    -> Result<CopyObjectResult, StorageError>;
    async fn list_objects_v2(
        &self,
        params: ListObjectsV2Params,
    ) -> Result<ListObjectsV2Result, StorageError>;
    async fn list_object_versions(
        &self,
        params: ListVersionsParams,
    ) -> Result<ListVersionsResult, StorageError>;

    // Multipart
    async fn create_multipart_upload(
        &self,
        params: CreateMultipartParams,
    ) -> Result<String, StorageError>;
    async fn upload_part(
        &self,
        params: UploadPartParams,
        body: ByteStream,
    ) -> Result<String, StorageError>;
    async fn complete_multipart_upload(
        &self,
        params: CompleteMultipartParams,
    ) -> Result<CompleteMultipartResult, StorageError>;
    async fn abort_multipart_upload(
        &self,
        params: AbortMultipartParams,
    ) -> Result<(), StorageError>;
    async fn list_multipart_uploads(
        &self,
        params: ListMultipartUploadsParams,
    ) -> Result<ListMultipartUploadsResult, StorageError>;
    async fn list_parts(&self, params: ListPartsParams) -> Result<ListPartsResult, StorageError>;
}
