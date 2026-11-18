use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VoiceTranscript {
    pub text: String,
    pub language: String,
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
}
