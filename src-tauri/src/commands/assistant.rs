use crate::models::{AssistantError, AssistantReply, ChatMessage};
use crate::services::assistant;

#[tauri::command]
pub async fn assistant_chat(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, AssistantError> {
    assistant::chat(messages, voice).await
}
