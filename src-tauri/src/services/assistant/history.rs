use serde_json::{Value, json};

use crate::models::{ChatMessage, ServiceError};

use super::prompt;

/// Builds the bounded conversation sent to the provider.
///
/// Keeps recent turns only and caps each message. Local models often run
/// with a much smaller context window, so keep proportionally less history
/// for them.
pub fn build(
    messages: Vec<ChatMessage>,
    voice: bool,
    web_search: bool,
    local: bool,
) -> Result<Vec<Value>, ServiceError> {
    let max_messages = if local { 6 } else { 12 };
    let max_message_chars = if local { 3_000 } else { 8_000 };

    let history: Vec<ChatMessage> = messages
        .into_iter()
        .rev()
        .take(max_messages)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut conversation = vec![json!({
        "role": "system",
        "content": prompt::system_prompt(voice, web_search),
    })];

    for message in history {
        let role = match message.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };

        let content: String = message.content.chars().take(max_message_chars).collect();
        if content.trim().is_empty() {
            continue;
        }

        conversation.push(json!({
            "role": role,
            "content": content,
        }));
    }

    if conversation.len() <= 1 {
        return Err(ServiceError::InvalidInput(
            "no user message provided".to_string(),
        ));
    }

    Ok(conversation)
}
