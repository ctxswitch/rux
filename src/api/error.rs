use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::error;

use crate::storage::error::StorageError;

use super::xml;

pub struct S3Error {
    pub code: &'static str,
    pub message: String,
    pub status: StatusCode,
    pub resource: Option<String>,
}

impl From<StorageError> for S3Error {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::BucketNotFound(name) => S3Error {
                code: "NoSuchBucket",
                message: format!("The specified bucket does not exist: {name}"),
                status: StatusCode::NOT_FOUND,
                resource: Some(name),
            },
            StorageError::BucketAlreadyExists(name) => S3Error {
                code: "BucketAlreadyOwnedByYou",
                message: format!(
                    "Your previous request to create the named bucket succeeded: {name}"
                ),
                status: StatusCode::CONFLICT,
                resource: Some(name),
            },
            StorageError::BucketNotEmpty(name) => S3Error {
                code: "BucketNotEmpty",
                message: format!("The bucket you tried to delete is not empty: {name}"),
                status: StatusCode::CONFLICT,
                resource: Some(name),
            },
            StorageError::ObjectNotFound { bucket, key } => S3Error {
                code: "NoSuchKey",
                message: format!("The specified key does not exist: {key}"),
                status: StatusCode::NOT_FOUND,
                resource: Some(format!("{bucket}/{key}")),
            },
            StorageError::VersionNotFound {
                bucket,
                key,
                version_id,
            } => S3Error {
                code: "NoSuchVersion",
                message: format!("The specified version does not exist: {version_id}"),
                status: StatusCode::NOT_FOUND,
                resource: Some(format!("{bucket}/{key}")),
            },
            StorageError::PreconditionFailed => S3Error {
                code: "PreconditionFailed",
                message: "At least one of the pre-conditions you specified did not hold".into(),
                status: StatusCode::PRECONDITION_FAILED,
                resource: None,
            },
            StorageError::NotModified => S3Error {
                code: "NotModified",
                message: String::new(),
                status: StatusCode::NOT_MODIFIED,
                resource: None,
            },
            StorageError::InvalidRange => S3Error {
                code: "InvalidRange",
                message: "The requested range is not satisfiable".into(),
                status: StatusCode::RANGE_NOT_SATISFIABLE,
                resource: None,
            },
            StorageError::UploadNotFound { upload_id, .. } => S3Error {
                code: "NoSuchUpload",
                message: format!("The specified upload does not exist: {upload_id}"),
                status: StatusCode::NOT_FOUND,
                resource: None,
            },
            StorageError::InvalidPart { part_number } => S3Error {
                code: "InvalidPart",
                message: format!(
                    "One or more of the specified parts could not be found: part {part_number}"
                ),
                status: StatusCode::BAD_REQUEST,
                resource: None,
            },
            StorageError::InvalidPartOrder => S3Error {
                code: "InvalidPartOrder",
                message: "The list of parts was not in ascending order".into(),
                status: StatusCode::BAD_REQUEST,
                resource: None,
            },
            StorageError::Database(msg) => {
                error!("database error: {msg}");
                S3Error {
                    code: "InternalError",
                    message: "An internal error occurred. Please try again.".into(),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    resource: None,
                }
            }
            StorageError::Internal(msg) => {
                error!("internal error: {msg}");
                S3Error {
                    code: "InternalError",
                    message: "An internal error occurred. Please try again.".into(),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    resource: None,
                }
            }
        }
    }
}

impl IntoResponse for S3Error {
    fn into_response(self) -> Response {
        // 304 responses must not include a body
        if self.status == StatusCode::NOT_MODIFIED {
            return (self.status, [("content-type", "application/xml")]).into_response();
        }

        let request_id = xml::new_request_id();
        match xml::error_response(
            self.code,
            &self.message,
            self.resource.as_deref(),
            &request_id,
        ) {
            Ok(body) => (
                self.status,
                [
                    ("content-type", "application/xml"),
                    ("x-amz-request-id", request_id.as_str()),
                ],
                body,
            )
                .into_response(),
            Err(e) => {
                error!("failed to serialize error response: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bucket_not_found() {
        let err = S3Error::from(StorageError::BucketNotFound("test-bucket".into()));
        assert_eq!(err.code, "NoSuchBucket");
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn from_object_not_found() {
        let err = S3Error::from(StorageError::ObjectNotFound {
            bucket: "b".into(),
            key: "k".into(),
        });
        assert_eq!(err.code, "NoSuchKey");
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn from_database_error_hides_details() {
        let err = S3Error::from(StorageError::Database("secret db info".into()));
        assert_eq!(err.code, "InternalError");
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!err.message.contains("secret db info"));
    }

    #[test]
    fn from_internal_error_hides_details() {
        let err = S3Error::from(StorageError::Internal("secret internal info".into()));
        assert_eq!(err.code, "InternalError");
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!err.message.contains("secret internal info"));
    }
}
