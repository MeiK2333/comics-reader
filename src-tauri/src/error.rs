//! Typed errors returned by Tauri commands.
//!
//! Commands return [`Result`], so a failure reaches the frontend as a
//! structured payload (`{ code, message, details }`) instead of an opaque
//! string. `code` is the stable, machine-readable part the frontend maps to a
//! localized message; `message` stays technical English for logs and for codes
//! the frontend does not know yet; `details` carries primitive context such as
//! the offending field name.
//!
//! Keep the variants and their codes in sync with `src/errors.ts`.

use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use thiserror::Error;

/// Result alias used by every command. The `From` impls below let `?` bubble
/// up the underlying errors without hand-written conversions.
pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// Stable identifier of an [`AppError`] kind, as sent to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    Network,
    HttpStatus,
    EmptyResponse,
    NotImage,
    TooLarge,
    NotFound,
    Io,
    Json,
    Tauri,
    Unexpected,
}

impl ErrorCode {
    /// Wire name of the code. Renaming it also requires updating the
    /// `ErrorCode` union and the `errors.*` catalog entries in the frontend.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalidInput",
            Self::Network => "network",
            Self::HttpStatus => "httpStatus",
            Self::EmptyResponse => "emptyResponse",
            Self::NotImage => "notImage",
            Self::TooLarge => "tooLarge",
            Self::NotFound => "notFound",
            Self::Io => "io",
            Self::Json => "json",
            Self::Tauri => "tauri",
            Self::Unexpected => "unexpected",
        }
    }
}

/// Every failure a command can report.
#[derive(Debug, Error)]
pub enum AppError {
    /// The frontend sent a value the command cannot accept.
    #[error("invalid `{field}`: {reason}")]
    InvalidInput { field: &'static str, reason: String },

    /// A remote request failed. Keep the URL in the error so the frontend can
    /// point at the host that refused it.
    #[error("request to {url} failed: {reason}")]
    Network { url: String, reason: String },

    /// The host answered with a non-success status.
    #[error("request to {url} returned HTTP {status}")]
    HttpStatus { url: String, status: u16 },

    /// The host answered successfully but sent no bytes.
    #[error("request to {url} returned an empty body")]
    EmptyResponse { url: String },

    /// The payload is not an image format the webview can display.
    #[error("response from {url} is not a supported image ({content_type})")]
    NotImage { url: String, content_type: String },

    /// The payload exceeded the cache's size limit.
    #[error("response from {url} exceeds the {limit} byte limit")]
    TooLarge { url: String, limit: u64 },

    /// A file or resource the command depends on does not exist.
    #[error("`{resource}` was not found")]
    NotFound { resource: String },

    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Errors raised by Tauri itself, e.g. while resolving an app path.
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    /// Anything without a dedicated variant yet.
    #[error("{0}")]
    Unexpected(String),
}

impl AppError {
    /// Machine-readable kind of this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidInput { .. } => ErrorCode::InvalidInput,
            Self::Network { .. } => ErrorCode::Network,
            Self::HttpStatus { .. } => ErrorCode::HttpStatus,
            Self::EmptyResponse { .. } => ErrorCode::EmptyResponse,
            Self::NotImage { .. } => ErrorCode::NotImage,
            Self::TooLarge { .. } => ErrorCode::TooLarge,
            Self::NotFound { .. } => ErrorCode::NotFound,
            Self::Io(_) => ErrorCode::Io,
            Self::Json(_) => ErrorCode::Json,
            Self::Tauri(_) => ErrorCode::Tauri,
            Self::Unexpected(_) => ErrorCode::Unexpected,
        }
    }

    /// Primitive context handed to the frontend. Values stay strings and
    /// numbers so they can be used directly as i18n parameters.
    pub fn details(&self) -> Map<String, Value> {
        let mut details = Map::new();
        match self {
            Self::InvalidInput { field, .. } => {
                details.insert("field".into(), Value::from(*field));
            }
            Self::Network { url, .. } => {
                details.insert("url".into(), Value::from(url.clone()));
            }
            Self::HttpStatus { url, status } => {
                details.insert("url".into(), Value::from(url.clone()));
                details.insert("status".into(), Value::from(*status));
            }
            Self::EmptyResponse { url } => {
                details.insert("url".into(), Value::from(url.clone()));
            }
            Self::NotImage { url, content_type } => {
                details.insert("url".into(), Value::from(url.clone()));
                details.insert("contentType".into(), Value::from(content_type.clone()));
            }
            Self::TooLarge { url, limit } => {
                details.insert("url".into(), Value::from(url.clone()));
                details.insert("limit".into(), Value::from(*limit));
            }
            Self::NotFound { resource } => {
                details.insert("resource".into(), Value::from(resource.clone()));
            }
            Self::Io(error) => {
                details.insert("kind".into(), Value::from(format!("{:?}", error.kind())));
            }
            Self::Json(error) => {
                details.insert("line".into(), Value::from(error.line()));
                details.insert("column".into(), Value::from(error.column()));
            }
            Self::Tauri(_) | Self::Unexpected(_) => {}
        }
        details
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Payload<'a> {
            code: &'a str,
            message: String,
            details: Map<String, Value>,
        }

        Payload {
            code: self.code().as_str(),
            message: self.to_string(),
            details: self.details(),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invalid_input_serializes_code_message_and_details() {
        let error = AppError::InvalidInput {
            field: "url",
            reason: "scheme must be http or https".into(),
        };

        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({
                "code": "invalidInput",
                "message": "invalid `url`: scheme must be http or https",
                "details": { "field": "url" }
            })
        );
    }

    #[test]
    fn http_status_serializes_url_and_status() {
        let error = AppError::HttpStatus {
            url: "https://example.com/a.png".into(),
            status: 403,
        };

        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({
                "code": "httpStatus",
                "message": "request to https://example.com/a.png returned HTTP 403",
                "details": {
                    "url": "https://example.com/a.png",
                    "status": 403
                }
            })
        );
    }

    #[test]
    fn io_errors_keep_their_kind() {
        let error = AppError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));

        let payload = serde_json::to_value(&error).unwrap();
        assert_eq!(payload["code"], "io");
        assert_eq!(payload["details"]["kind"], "PermissionDenied");
    }
}
