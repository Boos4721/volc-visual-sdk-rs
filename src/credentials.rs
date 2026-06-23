//! Credentials and the static configuration shared by signer and client.

use crate::error::{Result, VisualError};

/// Default service identifier used in the credential scope (`cv`).
pub const DEFAULT_SERVICE: &str = "cv";
/// Default region used when none is configured.
pub const DEFAULT_REGION: &str = "cn-north-1";
/// Default API host for the Visual service.
pub const DEFAULT_HOST: &str = "visual.volcengineapi.com";

/// Holds the access key, secret key and optional session token together with
/// the service/region that scope the signature.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Access key id (`VOLC_ACCESSKEY`).
    pub access_key: String,
    /// Secret access key (`VOLC_SECRETKEY`).
    pub secret_key: String,
    /// Optional STS session token, sent as `X-Security-Token` and signed.
    pub session_token: Option<String>,
    /// Service name, defaults to [`DEFAULT_SERVICE`].
    pub service: String,
    /// Region, defaults to [`DEFAULT_REGION`].
    pub region: String,
}

impl Credentials {
    /// Build credentials from an explicit access key / secret key pair.
    pub fn new(access_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            session_token: None,
            service: DEFAULT_SERVICE.to_string(),
            region: DEFAULT_REGION.to_string(),
        }
    }

    /// Read credentials from the `VOLC_ACCESSKEY` / `VOLC_SECRETKEY`
    /// environment variables. `VOLC_SESSIONTOKEN` is honored when present.
    pub fn from_env() -> Result<Self> {
        let access_key = std::env::var("VOLC_ACCESSKEY").map_err(|_| {
            VisualError::MissingCredentials("environment variable VOLC_ACCESSKEY not set".into())
        })?;
        let secret_key = std::env::var("VOLC_SECRETKEY").map_err(|_| {
            VisualError::MissingCredentials("environment variable VOLC_SECRETKEY not set".into())
        })?;
        let mut creds = Self::new(access_key, secret_key);
        if let Ok(token) = std::env::var("VOLC_SESSIONTOKEN") {
            if !token.is_empty() {
                creds.session_token = Some(token);
            }
        }
        Ok(creds)
    }

    /// Validate that both keys are non-empty.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.access_key.is_empty() {
            return Err(VisualError::MissingCredentials("access key is empty".into()));
        }
        if self.secret_key.is_empty() {
            return Err(VisualError::MissingCredentials("secret key is empty".into()));
        }
        Ok(())
    }
}
