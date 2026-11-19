use serde::Deserialize;

/// A single conversation message exchanged with the frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}
