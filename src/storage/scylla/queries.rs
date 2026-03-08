use scylla::client::session::Session;
use scylla::statement::prepared::PreparedStatement;

use crate::storage::error::StorageError;

pub struct PreparedQueries {
    pub create_bucket: PreparedStatement,
    pub create_bucket_by_owner: PreparedStatement,
    pub delete_bucket: PreparedStatement,
    pub delete_bucket_by_owner: PreparedStatement,
    pub get_bucket: PreparedStatement,
    pub list_buckets: PreparedStatement,
    pub update_bucket_versioning: PreparedStatement,
    pub update_bucket_policy: PreparedStatement,
    pub update_bucket_lifecycle: PreparedStatement,
    pub put_object_meta: PreparedStatement,
    pub get_object_latest: PreparedStatement,
    pub get_object_version: PreparedStatement,
    pub delete_object_meta: PreparedStatement,
    pub put_chunk: PreparedStatement,
    pub get_chunk: PreparedStatement,
    pub delete_chunk: PreparedStatement,
    pub put_listing: PreparedStatement,
    pub delete_listing: PreparedStatement,
    pub list_by_prefix: PreparedStatement,
    pub create_upload: PreparedStatement,
    pub create_active_upload: PreparedStatement,
    pub get_upload: PreparedStatement,
    pub update_upload_status: PreparedStatement,
    pub put_part: PreparedStatement,
    pub get_parts: PreparedStatement,
    pub delete_parts: PreparedStatement,
    pub delete_active_upload: PreparedStatement,
    pub list_uploads: PreparedStatement,
}

impl PreparedQueries {
    pub async fn prepare(session: &Session) -> Result<Self, StorageError> {
        Ok(Self {
            // LWT: caller must check [applied] column. If false, return BucketAlreadyExists.
            create_bucket: session
                .prepare(
                    "INSERT INTO rux.buckets (name, owner, region, versioning, created_at) \
                     VALUES (?, ?, ?, ?, ?) IF NOT EXISTS",
                )
                .await?,
            create_bucket_by_owner: session
                .prepare(
                    "INSERT INTO rux.buckets_by_owner (owner, name, region, created_at) \
                     VALUES (?, ?, ?, ?)",
                )
                .await?,
            delete_bucket: session
                .prepare("DELETE FROM rux.buckets WHERE name = ?")
                .await?,
            delete_bucket_by_owner: session
                .prepare("DELETE FROM rux.buckets_by_owner WHERE owner = ? AND name = ?")
                .await?,
            get_bucket: session
                .prepare("SELECT * FROM rux.buckets WHERE name = ?")
                .await?,
            list_buckets: session
                .prepare("SELECT * FROM rux.buckets_by_owner WHERE owner = ? LIMIT 10000")
                .await?,
            update_bucket_versioning: session
                .prepare("UPDATE rux.buckets SET versioning = ? WHERE name = ?")
                .await?,
            update_bucket_policy: session
                .prepare("UPDATE rux.buckets SET policy = ? WHERE name = ?")
                .await?,
            update_bucket_lifecycle: session
                .prepare("UPDATE rux.buckets SET lifecycle = ? WHERE name = ?")
                .await?,
            put_object_meta: session
                .prepare(
                    "INSERT INTO rux.objects (bucket, key, version_id, etag, size, content_type, \
                     user_metadata, chunk_count, delete_marker, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .await?,
            get_object_latest: session
                .prepare("SELECT * FROM rux.objects WHERE bucket = ? AND key = ? LIMIT 1")
                .await?,
            get_object_version: session
                .prepare(
                    "SELECT * FROM rux.objects WHERE bucket = ? AND key = ? AND version_id = ?",
                )
                .await?,
            delete_object_meta: session
                .prepare("DELETE FROM rux.objects WHERE bucket = ? AND key = ? AND version_id = ?")
                .await?,
            put_chunk: session
                .prepare(
                    "INSERT INTO rux.chunks (bucket, key, version_id, chunk_index, data, checksum) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .await?,
            get_chunk: session
                .prepare(
                    "SELECT data FROM rux.chunks \
                     WHERE bucket = ? AND key = ? AND version_id = ? AND chunk_index = ?",
                )
                .await?,
            delete_chunk: session
                .prepare(
                    "DELETE FROM rux.chunks \
                     WHERE bucket = ? AND key = ? AND version_id = ? AND chunk_index = ?",
                )
                .await?,
            put_listing: session
                .prepare(
                    "INSERT INTO rux.listings (bucket, prefix, name, is_common_prefix, \
                     version_id, size, etag, last_modified) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .await?,
            delete_listing: session
                .prepare("DELETE FROM rux.listings WHERE bucket = ? AND prefix = ? AND name = ?")
                .await?,
            list_by_prefix: session
                .prepare(
                    "SELECT name, is_common_prefix, version_id, size, etag, last_modified \
                     FROM rux.listings \
                     WHERE bucket = ? AND prefix = ? AND name > ? \
                     ORDER BY name ASC LIMIT ?",
                )
                .await?,
            create_upload: session
                .prepare(
                    "INSERT INTO rux.multipart_uploads \
                     (bucket, key, upload_id, content_type, user_metadata, status, initiated) \
                     VALUES (?, ?, ?, ?, ?, 'in_progress', ?)",
                )
                .await?,
            create_active_upload: session
                .prepare(
                    "INSERT INTO rux.active_multipart_uploads \
                     (bucket, key, upload_id, initiated) \
                     VALUES (?, ?, ?, ?)",
                )
                .await?,
            get_upload: session
                .prepare(
                    "SELECT * FROM rux.multipart_uploads \
                     WHERE bucket = ? AND key = ? AND upload_id = ?",
                )
                .await?,
            update_upload_status: session
                .prepare(
                    "UPDATE rux.multipart_uploads SET status = ? \
                     WHERE bucket = ? AND key = ? AND upload_id = ?",
                )
                .await?,
            put_part: session
                .prepare(
                    "INSERT INTO rux.multipart_parts \
                     (bucket, upload_id, part_number, data, size, etag, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .await?,
            get_parts: session
                .prepare(
                    "SELECT * FROM rux.multipart_parts \
                     WHERE bucket = ? AND upload_id = ? ORDER BY part_number ASC",
                )
                .await?,
            delete_parts: session
                .prepare("DELETE FROM rux.multipart_parts WHERE bucket = ? AND upload_id = ?")
                .await?,
            delete_active_upload: session
                .prepare(
                    "DELETE FROM rux.active_multipart_uploads \
                     WHERE bucket = ? AND key = ? AND upload_id = ?",
                )
                .await?,
            list_uploads: session
                .prepare(
                    "SELECT key, upload_id, initiated FROM rux.active_multipart_uploads \
                     WHERE bucket = ? AND (key, upload_id) > (?, ?) \
                     LIMIT ?",
                )
                .await?,
        })
    }
}
