mod client;
mod supervisor;

use serde_json::json;

use crate::models::{VoiceRecordingStatus, VoiceTranscript};

const PORT: u16 = 8765;
const PROTOCOL_VERSION: u64 = 16;

pub use supervisor::{preload, shutdown};

async fn ensure_ready() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(supervisor::ensure_server)
        .await
        .map_err(|error| error.to_string())?
}

pub async fn start_recording() -> Result<(), String> {
    ensure_ready().await?;
    client::post("/start", json!({})).await?;
    Ok(())
}

pub async fn recording_status() -> Result<VoiceRecordingStatus, String> {
    if !supervisor::is_running() {
        return Ok(VoiceRecordingStatus {
            recording: false,
            auto_stop: false,
            speech: false,
            silence: 0.0,
            level: 0.0,
        });
    }

    let value = client::post("/status", json!({})).await?;
    serde_json::from_value(value)
        .map_err(|error| format!("invalid voice recording status: {error}"))
}

pub async fn stop_recording() -> Result<VoiceTranscript, String> {
    ensure_ready().await?;
    let value = client::post("/stop", json!({})).await?;
    serde_json::from_value(value).map_err(|error| format!("invalid voice transcript: {error}"))
}

pub async fn cancel_recording() -> Result<(), String> {
    if !supervisor::is_running() {
        return Ok(());
    }
    client::post("/cancel", json!({})).await?;
    Ok(())
}

pub async fn speak(text: String, language: String) -> Result<(), String> {
    ensure_ready().await?;
    client::post("/speak", json!({ "text": text, "language": language })).await?;
    Ok(())
}

pub async fn stop_speaking() -> Result<(), String> {
    if !supervisor::is_running() {
        return Ok(());
    }
    client::post("/stop_speaking", json!({})).await?;
    Ok(())
}
