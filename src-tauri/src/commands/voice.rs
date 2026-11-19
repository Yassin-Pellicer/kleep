use crate::models::{VoiceRecordingStatus, VoiceTranscript};
use crate::services::voice;

#[tauri::command]
pub async fn voice_start_recording() -> Result<(), String> {
    voice::start_recording().await
}

#[tauri::command]
pub async fn voice_recording_status() -> Result<VoiceRecordingStatus, String> {
    voice::recording_status().await
}

#[tauri::command]
pub async fn voice_stop_recording() -> Result<VoiceTranscript, String> {
    voice::stop_recording().await
}

#[tauri::command]
pub async fn voice_cancel_recording() -> Result<(), String> {
    voice::cancel_recording().await
}

#[tauri::command]
pub async fn voice_speak(text: String, language: String) -> Result<(), String> {
    voice::speak(text, language).await
}

#[tauri::command]
pub async fn voice_stop_speaking() -> Result<(), String> {
    voice::stop_speaking().await
}
