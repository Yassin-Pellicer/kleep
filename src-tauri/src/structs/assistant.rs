use serde::{Deserialize, Serialize};

/// A single conversation message exchanged with the frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// A web result surfaced alongside an assistant answer.
#[derive(Debug, Clone, Serialize)]
pub struct ChatSource {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// Final answer returned by the assistant chat command.
#[derive(Debug, Serialize)]
pub struct AssistantReply {
    pub content: String,
    pub sources: Vec<ChatSource>,
}

/// Serializable error returned to the frontend by `assistant_chat`.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AssistantError {
    MissingApiKey,
    Unauthorized(String),
    RateLimited(String),
    Timeout(String),
    Network(String),
    BadResponse(String),
    Server(String),
    InvalidInput(String),
}

/// Response shape from `POST /v1/tools/search`.
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub date: Option<String>,
}
