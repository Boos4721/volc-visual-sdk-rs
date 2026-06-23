//! Strongly typed wrappers for the common Visual API response envelope.
//!
//! Every Visual API responds with a `ResponseMetadata` block plus an optional
//! `Result` payload. Because the `Result` shape differs per action, it is kept
//! as a [`serde_json::Value`] so callers can deserialize into their own structs
//! or inspect it dynamically.

use serde::Deserialize;

use crate::error::ApiErrorInfo;

/// The shared response envelope returned by every Visual action.
#[derive(Debug, Clone, Deserialize)]
pub struct VisualResponse {
    /// Gateway metadata: request id, action, version and any error block.
    #[serde(rename = "ResponseMetadata", default)]
    pub response_metadata: ResponseMetadata,

    /// The action-specific payload, left untyped for flexibility.
    #[serde(rename = "Result", default)]
    pub result: serde_json::Value,
}

impl VisualResponse {
    /// Returns the business error block if the gateway reported one.
    pub fn error(&self) -> Option<ApiErrorInfo> {
        self.response_metadata.error.as_ref().map(|e| ApiErrorInfo {
            code: e.code.clone(),
            message: e.message.clone(),
            request_id: self.response_metadata.request_id.clone(),
        })
    }

    /// `true` when the response carries no error block.
    pub fn is_success(&self) -> bool {
        self.response_metadata.error.is_none()
    }

    /// Deserialize the `Result` payload into a concrete type.
    pub fn result_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        T::deserialize(&self.result)
    }
}

/// Gateway metadata block (`ResponseMetadata`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResponseMetadata {
    /// Unique request id; quote this when contacting support.
    #[serde(rename = "RequestId", default)]
    pub request_id: String,
    /// The action that was invoked.
    #[serde(rename = "Action", default)]
    pub action: String,
    /// The API version.
    #[serde(rename = "Version", default)]
    pub version: String,
    /// The service name (`cv`).
    #[serde(rename = "Service", default)]
    pub service: String,
    /// The region that served the request.
    #[serde(rename = "Region", default)]
    pub region: String,
    /// Present only when the request failed.
    #[serde(rename = "Error")]
    pub error: Option<ResponseError>,
}

/// The error block nested inside [`ResponseMetadata`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResponseError {
    /// Numeric internal code, when provided.
    #[serde(rename = "CodeN", default)]
    pub code_n: i64,
    /// Business error code, e.g. `InvalidParameter`.
    #[serde(rename = "Code", default)]
    pub code: String,
    /// Human readable message.
    #[serde(rename = "Message", default)]
    pub message: String,
}
