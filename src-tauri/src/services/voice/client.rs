use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde_json::{Value, json};

use super::PORT;

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(2))
        .build()
        .expect("failed to build voice HTTP client")
});

pub(super) async fn post(path: &str, body: Value) -> Result<Value, String> {
    let response = CLIENT
        .post(format!("http://127.0.0.1:{PORT}{path}"))
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("voice service request failed: {error}"))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or(text);
        return Err(message);
    }

    if text.is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&text).map_err(|error| error.to_string())
}
