mod history;
mod prompt;
mod provider;
mod tool_loop;

use crate::models::{AssistantReply, ChatMessage, ServiceError};
use crate::services::support::env;

pub use provider::preload;

pub async fn chat(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, ServiceError> {
    let result = chat_inner(messages, voice).await;
    if let Err(error) = &result {
        eprintln!("assistant_chat failed: {error:?}");
    }
    result
}

async fn chat_inner(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, ServiceError> {
    let voice = voice.unwrap_or(false);
    let search_key = env::deepseek_api_key();
    let config = provider::config(search_key.as_deref())?;
    let web_search = search_key.is_some();

    let conversation = history::build(messages, voice, web_search, config.local)?;

    tool_loop::run(
        &config,
        conversation,
        voice,
        web_search,
        search_key.as_deref(),
    )
    .await
}
