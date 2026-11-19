use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSetupStatus {
    /// One of `idle`, `preparing`, `ready` or `failed`.
    pub state: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRecordingStatus {
    pub recording: bool,
    #[serde(default)]
    pub auto_stop: bool,
    #[serde(default)]
    pub speech: bool,
    #[serde(default)]
    pub silence: f64,
    #[serde(default)]
    pub level: f64,
}
