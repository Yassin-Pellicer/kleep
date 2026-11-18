use serde::{Deserialize, Serialize};

/// The entry shape consumed by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub word: String,
    pub pronunciation: String,
    pub part_of_speech: String,
    pub definition: String,
    pub example: String,
    pub origin: String,
    pub synonyms: Vec<String>,
}

/// Serializable error returned to the frontend by `lookup_word`.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum LookupError {
    MissingApiKey,
    Unauthorized(String),
    RateLimited(String),
    Timeout(String),
    Network(String),
    BadResponse(String),
    Server(String),
    InvalidInput(String),
}

/// Chat Completions response envelope.
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
