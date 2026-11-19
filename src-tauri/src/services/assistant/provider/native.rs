use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::models::ServiceError;
use crate::services::support::text::truncate_chars;

use super::model::{ModelConfig, OLLAMA_KEEP_ALIVE};
use super::types::{AssistantMessage, FunctionCall, ToolCall};

pub fn request_body(
    config: &ModelConfig,
    messages: &[Value],
    temperature: f64,
    max_tokens: u32,
    tools: &[Value],
    allow_tools: bool,
) -> Value {
    // Only the native endpoint accepts `num_ctx`. The OpenAI-compatible
    // endpoint always loads the model with Ollama's default window (4096
    // for qwen3.5), which the prompt plus tool output overflows, so the
    // server truncates mid-answer. Thinking is off: after a tool result
    // qwen3.5 puts the whole answer in the hidden `thinking` field and
    // leaves `content` empty.
    let mut body = json!({
        "model": config.model,
        "messages": native_messages(messages),
        "stream": false,
        "think": false,
        "keep_alive": OLLAMA_KEEP_ALIVE,
        "options": {
            "num_ctx": config.num_ctx,
            "temperature": temperature,
            "num_predict": max_tokens,
        },
    });
    if allow_tools {
        body["tools"] = json!(tools);
    }
    body
}

#[derive(Debug, Deserialize, Default)]
struct NativeChatCompletion {
    #[serde(default)]
    message: Option<NativeMessage>,
    #[serde(default)]
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct NativeMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Vec<NativeToolCall>,
}

#[derive(Debug, Deserialize)]
struct NativeToolCall {
    #[serde(default)]
    id: Option<String>,
    function: NativeFunctionCall,
}

#[derive(Debug, Deserialize)]
struct NativeFunctionCall {
    name: String,
    #[serde(default)]
    arguments: Value,
}

pub async fn read(response: reqwest::Response) -> Result<AssistantMessage, ServiceError> {
    let body = response
        .text()
        .await
        .map_err(|error| ServiceError::BadResponse(format!("could not read response: {error}")))?;

    let parsed: NativeChatCompletion = serde_json::from_str(&body).map_err(|error| {
        ServiceError::BadResponse(format!(
            "invalid response JSON: {error}; body: {}",
            truncate_chars(&body, 300)
        ))
    })?;

    if parsed.done_reason.as_deref() == Some("length") {
        eprintln!("assistant model output hit the token/context limit (finish_reason=length)");
    }

    let native = parsed.message.unwrap_or_default();
    let tool_calls: Vec<ToolCall> = native
        .tool_calls
        .into_iter()
        .enumerate()
        .map(|(index, call)| {
            let arguments = match call.function.arguments {
                Value::String(text) => text,
                other => other.to_string(),
            };
            ToolCall {
                id: call
                    .id
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| format!("call_{index}")),
                kind: "function".to_string(),
                function: FunctionCall {
                    name: call.function.name,
                    arguments,
                },
            }
        })
        .collect();

    let content = native.content.filter(|text| !text.trim().is_empty());
    if content.is_none() && tool_calls.is_empty() {
        let thinking_chars = native
            .thinking
            .as_deref()
            .unwrap_or_default()
            .chars()
            .count();
        return Err(ServiceError::BadResponse(format!(
            "model returned no content or tool calls (finish_reason: {}; thinking: {thinking_chars} chars)",
            parsed.done_reason.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(AssistantMessage {
        content,
        tool_calls,
    })
}

/// Converts the OpenAI-shaped conversation used by the rest of the module into
/// the shape Ollama's native `/api/chat` expects: tool-call arguments must be
/// JSON objects, and tool replies carry `tool_name` instead of `tool_call_id`.
fn native_messages(messages: &[Value]) -> Vec<Value> {
    let mut tool_names: HashMap<String, String> = HashMap::new();
    for message in messages {
        let Some(calls) = message.get("tool_calls").and_then(|value| value.as_array()) else {
            continue;
        };
        for call in calls {
            let Some(id) = call.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(name) = call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            tool_names.insert(id.to_string(), name.to_string());
        }
    }

    messages
        .iter()
        .map(|message| {
            let mut message = message.clone();

            if message.get("content").is_some_and(|value| value.is_null()) {
                message["content"] = json!("");
            }

            if let Some(calls) = message
                .get_mut("tool_calls")
                .and_then(|value| value.as_array_mut())
            {
                for call in calls {
                    let Some(function) = call.get_mut("function") else {
                        continue;
                    };
                    let parsed = function
                        .get("arguments")
                        .and_then(|value| value.as_str())
                        .map(|text| serde_json::from_str(text).unwrap_or_else(|_| json!({})));
                    if let Some(parsed) = parsed {
                        function["arguments"] = parsed;
                    }
                }
            }

            let tool_name = message
                .get("tool_call_id")
                .and_then(|value| value.as_str())
                .and_then(|id| tool_names.get(id))
                .cloned();
            if message.get("role").and_then(|value| value.as_str()) == Some("tool")
                && let Some(name) = tool_name
            {
                message["tool_name"] = json!(name);
            }

            message
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::native_messages;
    use serde_json::json;

    #[test]
    fn converts_tool_messages_for_the_native_api() {
        let messages = vec![
            json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "run_command",
                        "arguments": "{\"command\":\"nvidia-smi\"}"
                    }
                }]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "ok"
            }),
        ];

        let converted = native_messages(&messages);

        assert_eq!(converted[0]["content"], json!(""));
        assert_eq!(
            converted[0]["tool_calls"][0]["function"]["arguments"]["command"],
            json!("nvidia-smi")
        );
        assert_eq!(converted[1]["tool_name"], json!("run_command"));
    }
}
