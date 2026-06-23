//! The high level [`VisualClient`] used to call Visual actions.

use std::time::Duration;

use serde_json::Value;

use crate::credentials::{Credentials, DEFAULT_HOST};
use crate::error::{Result, VisualError};
use crate::response::VisualResponse;
use crate::sign::{self, QueryParam, SignableRequest};

/// Default request timeout applied when the caller does not set one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A client for the Volcengine Intelligent Visual (CV) service.
///
/// Construct it with an access key / secret key pair and tune the host, region,
/// session token or timeout with the `with_*` builder methods.
///
/// ```no_run
/// use volc_visual_sdk::VisualClient;
/// use serde_json::json;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = VisualClient::new("ak", "sk").with_region("cn-north-1");
/// let resp = client.cv_process(
///     "CVProcess",
///     "2022-08-31",
///     json!({ "req_key": "high_aes_general_v21_L", "prompt": "a cat" }),
/// )?;
/// println!("request id: {}", resp.response_metadata.request_id);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct VisualClient {
    credentials: Credentials,
    host: String,
    scheme: String,
    timeout: Duration,
    #[cfg(feature = "blocking")]
    http: reqwest::blocking::Client,
}

impl VisualClient {
    /// Create a client from an explicit access key and secret key.
    pub fn new(access_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self::from_credentials(Credentials::new(access_key, secret_key))
    }

    /// Create a client reading `VOLC_ACCESSKEY` / `VOLC_SECRETKEY` (and the
    /// optional `VOLC_SESSIONTOKEN`) from the environment.
    pub fn from_env() -> Result<Self> {
        Ok(Self::from_credentials(Credentials::from_env()?))
    }

    /// Create a client from a prebuilt [`Credentials`] value.
    pub fn from_credentials(credentials: Credentials) -> Self {
        Self {
            credentials,
            host: DEFAULT_HOST.to_string(),
            scheme: "https".to_string(),
            timeout: DEFAULT_TIMEOUT,
            #[cfg(feature = "blocking")]
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Override the signing region (defaults to `cn-north-1`).
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.credentials.region = region.into();
        self
    }

    /// Override the API host (defaults to `visual.volcengineapi.com`).
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set an STS session token, signed and sent as `X-Security-Token`.
    pub fn with_security_token(mut self, token: impl Into<String>) -> Self {
        self.credentials.session_token = Some(token.into());
        self
    }

    /// Override the request timeout (defaults to 30s).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        #[cfg(feature = "blocking")]
        {
            self.http = reqwest::blocking::Client::builder()
                .timeout(timeout)
                .build()
                .unwrap_or_default();
        }
        self
    }

    /// Override the URL scheme (defaults to `https`); mainly for testing.
    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = scheme.into();
        self
    }

    /// Access the configured credentials.
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    /// Build the signed request components for `action`/`version`/`body`
    /// without performing any network IO. Exposed for testing and for callers
    /// that want to drive their own HTTP stack.
    pub fn build_signed_request(
        &self,
        action: &str,
        version: &str,
        body: &Value,
    ) -> Result<PreparedRequest> {
        self.credentials.validate()?;

        let body_bytes = serde_json::to_vec(body)
            .map_err(|e| VisualError::Signing(format!("failed to serialize body: {e}")))?;

        let query = vec![
            QueryParam {
                key: "Action".to_string(),
                value: action.to_string(),
            },
            QueryParam {
                key: "Version".to_string(),
                value: version.to_string(),
            },
        ];

        let signable = SignableRequest {
            method: "POST".to_string(),
            path: "/".to_string(),
            host: self.host.clone(),
            query: query.clone(),
            body: body_bytes.clone(),
            content_type: "application/json".to_string(),
        };

        let x_date = sign::current_x_date();
        let signed = sign::sign_with_date(&signable, &self.credentials, &x_date);

        let url = format!(
            "{}://{}/?Action={}&Version={}",
            self.scheme, self.host, action, version
        );

        Ok(PreparedRequest {
            url,
            body: body_bytes,
            signed,
        })
    }

    /// Generic entry point: sign and POST `body` to `action`/`version`.
    ///
    /// `cv_process`, `cv_submit_task` and `cv_get_result` are thin wrappers over
    /// this method.
    #[cfg(feature = "blocking")]
    pub fn request(
        &self,
        action: &str,
        version: &str,
        body: Value,
    ) -> Result<VisualResponse> {
        let prepared = self.build_signed_request(action, version, &body)?;

        let mut builder = self
            .http
            .post(&prepared.url)
            .timeout(self.timeout)
            .header("Content-Type", prepared.signed.content_type.as_str())
            .header("Host", prepared.signed.host.as_str())
            .header("X-Date", prepared.signed.x_date.as_str())
            .header("X-Content-Sha256", prepared.signed.x_content_sha256.as_str())
            .header("Authorization", prepared.signed.authorization.as_str());
        if let Some(token) = &prepared.signed.x_security_token {
            builder = builder.header("X-Security-Token", token.as_str());
        }

        let resp = builder.body(prepared.body).send()?;
        let status = resp.status();
        let text = resp.text()?;

        let parsed: VisualResponse = serde_json::from_str(&text)
            .map_err(|e| VisualError::Decode(format!("{e}; raw body: {text}")))?;

        if let Some(err) = parsed.error() {
            return Err(VisualError::Api {
                status: status.as_u16(),
                code: err.code,
                message: err.message,
                request_id: err.request_id,
            });
        }

        Ok(parsed)
    }

    /// Synchronous processing (`Action=CVProcess`). Returns the result inline.
    #[cfg(feature = "blocking")]
    pub fn cv_process(
        &self,
        action: &str,
        version: &str,
        body: Value,
    ) -> Result<VisualResponse> {
        self.request(action, version, body)
    }

    /// Asynchronous submission (`Action=CVSubmitTask`). The result carries a
    /// `task_id` to poll with [`VisualClient::cv_get_result`].
    #[cfg(feature = "blocking")]
    pub fn cv_submit_task(
        &self,
        action: &str,
        version: &str,
        body: Value,
    ) -> Result<VisualResponse> {
        self.request(action, version, body)
    }

    /// Poll an asynchronous task result (`Action=CVGetResult`).
    #[cfg(feature = "blocking")]
    pub fn cv_get_result(
        &self,
        action: &str,
        version: &str,
        body: Value,
    ) -> Result<VisualResponse> {
        self.request(action, version, body)
    }
}

/// The signed, ready-to-send components of a request.
#[derive(Debug, Clone)]
pub struct PreparedRequest {
    /// Fully-qualified request URL including `Action`/`Version` query.
    pub url: String,
    /// Raw JSON body bytes that were signed.
    pub body: Vec<u8>,
    /// The signed headers to attach.
    pub signed: crate::sign::SignedHeaders,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_signed_request_populates_headers_and_url() {
        let client = VisualClient::new("AKTPexample", "c2VjcmV0").with_region("cn-north-1");
        let prepared = client
            .build_signed_request("CVProcess", "2022-08-31", &json!({"req_key": "x"}))
            .expect("signing should succeed");

        assert_eq!(
            prepared.url,
            "https://visual.volcengineapi.com/?Action=CVProcess&Version=2022-08-31"
        );
        assert!(prepared.signed.authorization.starts_with("HMAC-SHA256 Credential=AKTPexample/"));
        assert!(prepared
            .signed
            .authorization
            .contains("/cn-north-1/cv/request"));
        assert_eq!(prepared.signed.host, "visual.volcengineapi.com");
        assert!(prepared.signed.x_security_token.is_none());
    }

    #[test]
    fn empty_credentials_are_rejected() {
        let client = VisualClient::new("", "");
        let err = client
            .build_signed_request("CVProcess", "2022-08-31", &json!({}))
            .unwrap_err();
        assert!(matches!(err, VisualError::MissingCredentials(_)));
    }

    #[test]
    fn security_token_is_signed_and_attached() {
        let client = VisualClient::new("AKTPexample", "c2VjcmV0")
            .with_security_token("STS2token");
        let prepared = client
            .build_signed_request("CVProcess", "2022-08-31", &json!({}))
            .unwrap();
        assert_eq!(prepared.signed.x_security_token.as_deref(), Some("STS2token"));
        assert!(prepared
            .signed
            .authorization
            .contains("x-security-token"));
    }
}
