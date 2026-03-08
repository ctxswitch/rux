use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, head, post, put};
use tower_http::timeout::TimeoutLayer;

use crate::config::Config;
use crate::storage::Storage;

use super::error::S3Error;

/// Shared application state threaded through all handlers.
pub struct AppState<S: Storage> {
    pub storage: Arc<S>,
    pub default_consistency: crate::storage::types::Consistency,
}

// Manual Clone: #[derive(Clone)] would add an unnecessary S: Clone bound.
// Arc<S> is always Clone regardless of S.
impl<S: Storage> Clone for AppState<S> {
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            default_consistency: self.default_consistency,
        }
    }
}

/// Build the axum router with all S3 API routes.
///
/// All handlers currently return 501 Not Implemented. They will be replaced
/// with real implementations incrementally.
pub fn build<S: Storage>(storage: S, config: &Config) -> Router {
    let state = AppState {
        storage: Arc::new(storage),
        default_consistency: config.default_consistency,
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(list_buckets::<S>))
        .route("/{bucket}", put(create_bucket::<S>))
        .route("/{bucket}", delete(delete_bucket::<S>))
        .route("/{bucket}", head(head_bucket::<S>))
        .route("/{bucket}", get(get_bucket::<S>))
        .route("/{bucket}/{key:.*}", put(put_object::<S>))
        .route("/{bucket}/{key:.*}", get(get_object::<S>))
        .route("/{bucket}/{key:.*}", head(head_object::<S>))
        .route("/{bucket}/{key:.*}", delete(delete_object::<S>))
        .route("/{bucket}/{key:.*}", post(post_object::<S>))
        .with_state(state)
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(300),
        ))
}

/// `GET /healthz` -- Health check endpoint for Kubernetes probes.
async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// Return a 501 Not Implemented S3 error.
fn not_implemented() -> S3Error {
    S3Error {
        code: "NotImplemented",
        message: "This operation is not yet implemented".into(),
        status: StatusCode::NOT_IMPLEMENTED,
        resource: None,
    }
}

/// `GET /` -- ListBuckets
async fn list_buckets<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `PUT /{bucket}` -- CreateBucket
async fn create_bucket<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `DELETE /{bucket}` -- DeleteBucket
async fn delete_bucket<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `HEAD /{bucket}` -- HeadBucket
async fn head_bucket<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `GET /{bucket}` -- ListObjectsV2, GetBucketLocation, GetBucketVersioning,
/// GetBucketPolicy, GetBucketLifecycle, or ListMultipartUploads (dispatched
/// by query parameters, not yet implemented).
async fn get_bucket<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `PUT /{bucket}/{key}` -- PutObject, CopyObject, or UploadPart (dispatched
/// by headers/query parameters, not yet implemented).
async fn put_object<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `GET /{bucket}/{key}` -- GetObject or ListObjectVersions (dispatched by
/// query parameters, not yet implemented).
async fn get_object<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `HEAD /{bucket}/{key}` -- HeadObject
async fn head_object<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `DELETE /{bucket}/{key}` -- DeleteObject
async fn delete_object<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}

/// `POST /{bucket}/{key}` -- CreateMultipartUpload or CompleteMultipartUpload
/// (dispatched by query parameters, not yet implemented).
async fn post_object<S: Storage>(State(_state): State<AppState<S>>) -> S3Error {
    not_implemented()
}
