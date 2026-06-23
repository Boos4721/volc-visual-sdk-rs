//! Error types for the Volcengine Visual SDK.

use std::fmt;

/// The result type used throughout this crate.
pub type Result<T> = std::result::Result<T, VisualError>;

/// Errors that can occur when calling the Visual service.
#[derive(Debug, thiserror::Error)]
pub enum VisualError {
    /// Access key or secret key was empty or could not be resolved.
    #[error("missing credentials: {0}")]
    MissingCredentials(String),

    /// Failed while building the canonical request or signing key.
    #[error("signing error: {0}")]
    Signing(String),

    /// Failed to build a valid request URL.
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// The transport (network) layer failed.
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The response body could not be parsed as JSON.
    #[error("failed to decode response: {0}")]
    Decode(String),

    /// The service returned a non-success HTTP status with a parsed body.
    #[error("api error (http {status}): {code}: {message}")]
    Api {
        /// HTTP status code returned by the gateway.
        status: u16,
        /// Business error code from `ResponseMetadata.Error.Code`.
        code: String,
        /// Human readable error message.
        message: String,
        /// The volcengine request id, useful when filing a ticket.
        request_id: String,
    },
}

/// A structured business error extracted from `ResponseMetadata.Error`.
#[derive(Debug, Clone)]
pub struct ApiErrorInfo {
    /// Business error code, e.g. `InvalidParameter`.
    pub code: String,
    /// Human readable message.
    pub message: String,
    /// Volcengine request id.
    pub request_id: String,
}

impl fmt::Display for ApiErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (code={}, request_id={})",
            self.message, self.code, self.request_id
        )
    }
}
