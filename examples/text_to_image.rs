//! Text-to-image (synchronous, `CVProcess`).
//!
//! Mirrors the "通用2.1-文生图" doc sample: submit a prompt and read the
//! generated image data back from the synchronous response.
//!
//! Run with:
//! ```bash
//! VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example text_to_image
//! ```

use serde_json::json;
use volc_visual_sdk::VisualClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads VOLC_ACCESSKEY / VOLC_SECRETKEY from the environment.
    let client = VisualClient::from_env()?.with_region("cn-north-1");

    // "通用2.1-文生图" uses req_key = high_aes_general_v21_L.
    let body = json!({
        "req_key": "high_aes_general_v21_L",
        "prompt": "a fluffy orange cat sitting on a windowsill at sunset",
        "seed": -1,
        "scale": 3.5,
        "width": 512,
        "height": 512,
        "return_url": true
    });

    let resp = client.cv_process("CVProcess", "2022-08-31", body)?;

    if let Some(err) = resp.error() {
        eprintln!("API returned an error: {err}");
        return Ok(());
    }

    println!("request id: {}", resp.response_metadata.request_id);
    // The text-to-image result typically contains `image_urls` and/or
    // base64 `binary_data_base64`. Inspect the untyped result payload:
    if let Some(urls) = resp.result.get("image_urls") {
        println!("image urls: {urls}");
    } else {
        println!("result: {}", resp.result);
    }

    Ok(())
}
