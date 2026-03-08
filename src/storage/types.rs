use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::ByteStream;

#[derive(Debug, Clone)]
pub enum VersioningState {
    Disabled,
    Enabled,
    Suspended,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Consistency {
    #[default]
    Eventual,
    Quorum,
    Strong,
    All,
}

impl fmt::Display for Consistency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Consistency::Eventual => write!(f, "eventual"),
            Consistency::Quorum => write!(f, "quorum"),
            Consistency::Strong => write!(f, "strong"),
            Consistency::All => write!(f, "all"),
        }
    }
}

impl FromStr for Consistency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eventual" => Ok(Consistency::Eventual),
            "quorum" => Ok(Consistency::Quorum),
            "strong" => Ok(Consistency::Strong),
            "all" => Ok(Consistency::All),
            _ => Err(format!("unknown consistency level: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistency_from_str_eventual() {
        let c: Consistency = "eventual".parse().unwrap();
        assert!(matches!(c, Consistency::Eventual));
    }

    #[test]
    fn consistency_from_str_quorum() {
        let c: Consistency = "quorum".parse().unwrap();
        assert!(matches!(c, Consistency::Quorum));
    }

    #[test]
    fn consistency_from_str_strong() {
        let c: Consistency = "strong".parse().unwrap();
        assert!(matches!(c, Consistency::Strong));
    }

    #[test]
    fn consistency_from_str_all() {
        let c: Consistency = "all".parse().unwrap();
        assert!(matches!(c, Consistency::All));
    }

    #[test]
    fn consistency_from_str_case_insensitive() {
        let c: Consistency = "EVENTUAL".parse().unwrap();
        assert!(matches!(c, Consistency::Eventual));
    }

    #[test]
    fn consistency_from_str_invalid() {
        let result = "invalid".parse::<Consistency>();
        assert!(result.is_err());
    }
}

#[derive(Debug, Clone)]
pub struct BucketMeta {
    pub name: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
    pub region: String,
}

#[derive(Debug, Clone)]
pub struct ObjectMeta {
    pub bucket: String,
    pub key: String,
    pub version_id: Uuid,
    pub etag: String,
    pub size: u64,
    pub content_type: String,
    pub user_metadata: HashMap<String, String>,
    pub chunk_count: u32,
    pub delete_marker: bool,
    pub created_at: DateTime<Utc>,
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
    pub if_none_match: bool,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct PutObjectResult {
    pub etag: String,
    pub version_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct GetObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<Uuid>,
    pub range: Option<ByteRange>,
    pub if_match: Option<String>,
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<DateTime<Utc>>,
    pub if_unmodified_since: Option<DateTime<Utc>>,
    pub consistency: Consistency,
}

pub struct GetObjectResult {
    pub meta: ObjectMeta,
    pub body: ByteStream,
    pub content_range: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HeadObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct DeleteObjectParams {
    pub bucket: String,
    pub key: String,
    pub version_id: Option<Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct DeleteObjectResult {
    pub version_id: Uuid,
    pub delete_marker: bool,
}

#[derive(Debug, Clone)]
pub struct CopyObjectParams {
    pub source_bucket: String,
    pub source_key: String,
    pub source_version_id: Option<Uuid>,
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
    pub last_modified: DateTime<Utc>,
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
    pub last_modified: DateTime<Utc>,
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
    pub version_id_marker: Option<Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListVersionsResult {
    pub versions: Vec<VersionEntry>,
    pub delete_markers: Vec<DeleteMarkerEntry>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_key_marker: Option<String>,
    pub next_version_id_marker: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub key: String,
    pub version_id: Uuid,
    pub is_latest: bool,
    pub last_modified: DateTime<Utc>,
    pub etag: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct DeleteMarkerEntry {
    pub key: String,
    pub version_id: Uuid,
    pub is_latest: bool,
    pub last_modified: DateTime<Utc>,
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
    pub upload_id: Uuid,
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
    pub upload_id: Uuid,
    pub parts: Vec<PartInfo>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct CompleteMultipartResult {
    pub etag: String,
    pub version_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AbortMultipartParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: Uuid,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListMultipartUploadsParams {
    pub bucket: String,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub max_uploads: u32,
    pub key_marker: Option<String>,
    pub upload_id_marker: Option<Uuid>,
    pub consistency: Consistency,
}

#[derive(Debug, Clone)]
pub struct ListMultipartUploadsResult {
    pub uploads: Vec<MultipartUploadEntry>,
    pub is_truncated: bool,
    pub next_key_marker: Option<String>,
    pub next_upload_id_marker: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct MultipartUploadEntry {
    pub key: String,
    pub upload_id: Uuid,
    pub initiated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ListPartsParams {
    pub bucket: String,
    pub key: String,
    pub upload_id: Uuid,
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
    pub last_modified: DateTime<Utc>,
}
