use serde::Serialize;

use super::source::ChatSource;

/// Final answer returned by the assistant chat command.
#[derive(Debug, Serialize)]
pub struct AssistantReply {
    pub content: String,
    pub sources: Vec<ChatSource>,
    /// True when the assistant completed a user-facing action such as opening
    /// an app, so the palette can close instead of showing a confirmation.
    pub opened: bool,
}
