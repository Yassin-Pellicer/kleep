use serde_json::{Value, json};

use crate::models::ServiceError;
use crate::services::support::text::truncate_chars;

use super::model::ModelConfig;
use super::types::{AssistantMessage, ChatCompletion};

pub fn request_body(
    config: &ModelConfig,
    messages: &[Value],
    temperature: f64,
    max_tokens: u32,
    tools: &[Value],
    allow_tools: bool,
) -> Value {
    let mut body = json!({
        "model": config.model,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "messages": messages,
        "thinking": { "type": "disabled" },
    });
    if allow_tools {
        body["tools"] = json!(tools);
        body["tool_choice"] = json!("auto");
    }
    body
}

pub async fn read(response: reqwest::Response) -> Result<AssistantMessage, ServiceError> {
    let body = response
        .text()
        .await
        .map_err(|error| ServiceError::BadResponse(format!("could not read response: {error}")))?;

    let parsed: ChatCompletion = serde_json::from_str(&body).map_err(|error| {
        ServiceError::BadResponse(format!(
            "invalid response JSON: {error}; body: {}",
            truncate_chars(&body, 300)
        ))
    })?;

    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ServiceError::BadResponse("empty model response".to_string()))?;

    if let Some(reason) = &choice.finish_reason
        && reason == "length"
    {
        eprintln!("assistant model output hit the token/context limit (finish_reason=length)");
    }

    if choice
        .message
        .content
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && choice.message.tool_calls.is_empty()
    {
        return Err(ServiceError::BadResponse(format!(
            "model returned no content or tool calls (finish_reason: {})",
            choice.finish_reason.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(choice.message)
}
