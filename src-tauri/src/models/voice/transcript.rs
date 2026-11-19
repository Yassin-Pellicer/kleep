use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VoiceTranscript {
    pub text: String,
    pub language: String,
}
