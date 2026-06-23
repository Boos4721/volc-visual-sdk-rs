# volc-visual-sdk

[English](README.md) | [中文](README.zh-CN.md)

A pure-Rust SDK for the **Volcengine Intelligent Visual (CV)** service, ported
from the official Java/Python SDKs. It implements the Volcengine **Signature V4
(HMAC-SHA256)** algorithm natively and ships with TLS provided by **rustls** —
no OpenSSL required.

## Features

- Native Signature V4, validated byte-for-byte against fixed vectors that match
  the official Python and Go SDKs.
- The three standard call shapes used across the Visual APIs:
  - **synchronous** — `cv_process` (`Action=CVProcess`)
  - **async submit** — `cv_submit_task` (`Action=CVSubmitTask`)
  - **async query** — `cv_get_result` (`Action=CVGetResult`)
  - plus a generic `request(action, version, body)` that all three wrap.
- Strongly typed response envelope (`ResponseMetadata` + `Result`) with a
  `serde_json::Value` fallback for action-specific payloads.
- Configurable region, host, STS session token and timeout.
- Credentials from explicit keys or from `VOLC_ACCESSKEY` / `VOLC_SECRETKEY`.

## Installation

```toml
[dependencies]
# Not yet published to crates.io — depend on the git repo:
volc-visual-sdk = { git = "https://github.com/Boos4721/volc-visual-sdk-rs" }
```

The default `blocking` feature uses `reqwest`'s blocking client over rustls.

## Quick start

```rust
use volc_visual_sdk::VisualClient;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads VOLC_ACCESSKEY / VOLC_SECRETKEY from the environment.
    let client = VisualClient::from_env()?
        .with_region("cn-north-1");

    let resp = client.cv_process(
        "CVProcess",
        "2022-08-31",
        json!({
            "req_key": "high_aes_general_v21_L",
            "prompt": "a fluffy cat sitting on a windowsill"
        }),
    )?;

    if let Some(err) = resp.error() {
        eprintln!("api error: {err}");
    } else {
        println!("result: {}", resp.result);
    }
    Ok(())
}
```

### Configuration

```rust
use std::time::Duration;
use volc_visual_sdk::VisualClient;

let client = VisualClient::new("AK...", "SK...")
    .with_region("cn-north-1")
    .with_host("visual.volcengineapi.com")
    .with_security_token("STS2...")     // optional STS token
    .with_timeout(Duration::from_secs(60));
```

## The three interface types

| Type | Method | Action | Use |
| --- | --- | --- | --- |
| Synchronous | `cv_process` | `CVProcess` | Returns the result inline. |
| Async submit | `cv_submit_task` | `CVSubmitTask` | Returns a `task_id`. |
| Async query | `cv_get_result` | `CVGetResult` | Polls for a submitted task. |

All three forward to `request(action, version, body)`, so you can call any
Visual action — including sync-to-async ones such as `CVSync2AsyncSubmitTask` /
`CVSync2AsyncGetResult` — through the generic method.

## Signature V4

Every request is signed with Volcengine Signature V4:

1. **Canonical request** = `method \n norm_uri \n norm_query \n canonical_headers \n signed_headers \n hex_sha256(body)`.
   Headers signed are `content-type`, `host`, `x-content-sha256`, `x-date`, and
   `x-security-token` when a session token is present — sorted and lower-cased.
2. **String to sign** = `HMAC-SHA256 \n X-Date \n date/region/service/request \n hex_sha256(canonical_request)`.
3. **Signing key** = four chained HMAC-SHA256 rounds: `date → region → service → "request"`.
4. **Signature** = `hex(HMAC-SHA256(signing_key, string_to_sign))`, placed in the
   `Authorization` header alongside the credential scope and signed-header list.

Defaults: `service = cv`, `region = cn-north-1`, `host = visual.volcengineapi.com`,
`X-Date` in ISO8601 basic (`YYYYMMDDTHHMMSSZ`).

The signing core (`sign::sign_with_date`) is deterministic and covered by fixed
vectors in `src/sign.rs` — run `cargo test` to verify the canonical request
hash, signing key and final signature all match the reference implementation.

## Verification

The fixed signature vectors are not self-referential: they were cross-checked
against both official SDKs for the same request, and all three implementations
(this crate, the Python SDK, the Go SDK) produce byte-for-byte identical output.

```bash
# 1) Rust unit tests (canonical hash, signing key, full signature)
cargo test

# 2) Cross-validate against the official Volcengine Go SDK.
#    Clones volc-sdk-golang, drops a test into its `base` package to reach the
#    unexported signer, feeds the same inputs as src/sign.rs, and asserts the
#    Go signer matches. Requires a Go toolchain and network access.
./scripts/verify_go_crossbuild.sh
```

| Source | Method | Result |
| --- | --- | --- |
| This crate | `cargo test` (fixed vectors) | matches |
| Official Python SDK | `SignerV4.py` over the same inputs | matches |
| Official Go SDK | `scripts/verify_go_crossbuild.sh` | matches |

A notable correctness detail: the secret key feeds the first HMAC round as its
**raw UTF-8 bytes** — it is *not* base64-decoded, even though it may look like
base64. Both official SDKs do the same, and so does this crate.

## Examples

```bash
# Synchronous text-to-image (通用2.1-文生图)
VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example text_to_image

# Async submit + poll
VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example async_task_poll
```

## License

MIT
