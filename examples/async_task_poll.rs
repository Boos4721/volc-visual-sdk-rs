//! Asynchronous task: submit (`CVSubmitTask`) then poll (`CVGetResult`).
//!
//! Many heavier Visual actions are asynchronous: you submit a task, receive a
//! `task_id`, then poll for the result until the status is done. This example
//! shows that submit + poll loop.
//!
//! Run with:
//! ```bash
//! VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example async_task_poll
//! ```

use std::thread::sleep;
use std::time::Duration;

use serde_json::json;
use volc_visual_sdk::VisualClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = VisualClient::from_env()?;

    // Submit the task. Replace req_key/params with the action you need.
    let submit = client.cv_submit_task(
        "CVSubmitTask",
        "2022-08-31",
        json!({
            "req_key": "high_aes_general_v21_L",
            "prompt": "a serene mountain lake reflecting the night sky"
        }),
    )?;

    if let Some(err) = submit.error() {
        eprintln!("submit failed: {err}");
        return Ok(());
    }

    let task_id = submit
        .result
        .get("task_id")
        .and_then(|v| v.as_str())
        .ok_or("no task_id in submit response")?
        .to_string();
    println!("submitted task_id = {task_id}");

    // Poll for the result. Real code should bound the number of attempts.
    for attempt in 1..=20 {
        sleep(Duration::from_secs(3));

        let result = client.cv_get_result(
            "CVGetResult",
            "2022-08-31",
            json!({
                "req_key": "high_aes_general_v21_L",
                "task_id": task_id,
            }),
        )?;

        if let Some(err) = result.error() {
            eprintln!("poll failed: {err}");
            return Ok(());
        }

        let status = result
            .result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("attempt {attempt}: status = {status}");

        if status == "done" || status == "success" {
            println!("final result: {}", result.result);
            return Ok(());
        }
    }

    println!("task did not finish within the polling window");
    Ok(())
}
