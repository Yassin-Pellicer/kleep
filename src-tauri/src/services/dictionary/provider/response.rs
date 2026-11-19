use serde::Deserialize;

use crate::models::{DictionaryEntry, ServiceError};

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

pub(super) fn parse_entry(
    parsed: ChatResponse,
    tool_name: &str,
) -> Result<DictionaryEntry, ServiceError> {
    let call = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.tool_calls.into_iter().next())
        .ok_or_else(|| {
            ServiceError::BadResponse("the model did not return a tool call".to_string())
        })?;

    if call.function.name != tool_name {
        return Err(ServiceError::BadResponse(format!(
            "unexpected tool `{}`",
            call.function.name
        )));
    }

    let entry: DictionaryEntry = serde_json::from_str(&call.function.arguments)
        .map_err(|error| ServiceError::BadResponse(format!("invalid tool arguments: {error}")))?;

    if entry.word.trim().is_empty() || entry.definition.trim().is_empty() {
        return Err(ServiceError::BadResponse(
            "the model returned an empty entry".to_string(),
        ));
    }

    Ok(entry)
}
