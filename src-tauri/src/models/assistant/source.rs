use serde::Serialize;

/// A web result surfaced alongside an assistant answer.
#[derive(Debug, Clone, Serialize)]
pub struct ChatSource {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}
