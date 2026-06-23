//! # volc-visual-sdk
//!
//! A pure-Rust SDK for the Volcengine Intelligent Visual (CV) service, ported
//! from the official Java/Python SDKs. It implements the Volcengine Signature
//! V4 (HMAC-SHA256) algorithm natively (no OpenSSL — TLS is provided by
//! rustls) and exposes the three standard call shapes used by the Visual APIs:
//!
//! * **synchronous** — [`VisualClient::cv_process`] (`Action=CVProcess`)
//! * **async submit** — [`VisualClient::cv_submit_task`] (`Action=CVSubmitTask`)
//! * **async query** — [`VisualClient::cv_get_result`] (`Action=CVGetResult`)
//!
//! All three are thin wrappers over the generic [`VisualClient::request`].
//!
//! ## Quick start
//!
//! ```no_run
//! use volc_visual_sdk::VisualClient;
//! use serde_json::json;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = VisualClient::from_env()?; // VOLC_ACCESSKEY / VOLC_SECRETKEY
//! let resp = client.cv_process(
//!     "CVProcess",
//!     "2022-08-31",
//!     json!({
//!         "req_key": "high_aes_general_v21_L",
//!         "prompt": "a fluffy cat sitting on a windowsill"
//!     }),
//! )?;
//! if let Some(err) = resp.error() {
//!     eprintln!("api error: {err}");
//! } else {
//!     println!("result: {}", resp.result);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Signature V4
//!
//! See the [`sign`] module for the full algorithm. The signing core is
//! deterministic ([`sign::sign_with_date`]) and validated against fixed vectors
//! that match the official Python and Go SDKs.

#![warn(missing_docs)]

pub mod client;
pub mod credentials;
pub mod error;
pub mod response;
pub mod sign;

pub use client::{PreparedRequest, VisualClient};
pub use credentials::{Credentials, DEFAULT_HOST, DEFAULT_REGION, DEFAULT_SERVICE};
pub use error::{ApiErrorInfo, Result, VisualError};
pub use response::{ResponseError, ResponseMetadata, VisualResponse};
