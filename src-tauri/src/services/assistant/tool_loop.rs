use std::collections::HashMap;

use serde_json::{Value, json};

use crate::models::{AssistantReply, ServiceError};
use crate::services::support::text::truncate_chars;
use crate::tools::{self, ToolContext, ToolOutcome};

use super::provider::{self, AssistantMessage, ModelConfig};

const MAX_TOOL_ROUNDS: usize = 3;
const MAX_TOOL_ROUNDS_LOCAL: usize = 6;

fn log_model_message(message: &AssistantMessage) {
    if let Some(content) = message
        .content
        .as_deref()
        .map(str::trim)
        .filter(|content| !content.is_empty())
    {
        eprintln!(
            "kleep assistant: model text: {}",
            truncate_chars(content, 2000)
        );
    }

    for call in &message.tool_calls {
        eprintln!(
            "kleep assistant: model tool call {} {}",
            call.function.name,
            truncate_chars(&call.function.arguments, 500)
        );
    }
}

fn reply(content: String, opened: bool, context: ToolContext<'_>) -> AssistantReply {
    AssistantReply {
        content,
        sources: context.sources,
        opened,
    }
}

/// The latest user message, used to check that a tool action matches what the
/// user actually asked for.
fn last_user_message(conversation: &[Value]) -> String {
    conversation
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|message| message.get("content").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

pub async fn run(
    config: &ModelConfig,
    mut conversation: Vec<Value>,
    voice: bool,
    web_search: bool,
    search_key: Option<&str>,
) -> Result<AssistantReply, ServiceError> {
    let request = last_user_message(&conversation);
    let mut context = ToolContext::new(search_key, config.local, request.clone());
    eprintln!(
        "kleep assistant: request: {}",
        truncate_chars(&request, 500)
    );
    let mut seen_calls: HashMap<String, String> = HashMap::new();
    let max_tool_rounds = if config.local {
        MAX_TOOL_ROUNDS_LOCAL
    } else {
        MAX_TOOL_ROUNDS
    };
    let mut tool_round = 0;
    let mut opened_app = false;
    let tool_definitions = tools::definitions(web_search);

    loop {
        let allow_tools = tool_round < max_tool_rounds;
        if !allow_tools {
            conversation.push(json!({
                "role": "system",
                "content": "The tool call limit has been reached. Do not call any more \
                            tools. Answer now using the results already in the conversation."
            }));
        }

        let message =
            provider::complete(config, &conversation, &tool_definitions, voice, allow_tools)
                .await?;
        log_model_message(&message);

        if message.tool_calls.is_empty() {
            let content = message.content.unwrap_or_default().trim().to_string();
            if content.is_empty() {
                return Err(ServiceError::BadResponse(
                    "the model returned an empty answer".to_string(),
                ));
            }

            eprintln!("kleep assistant: reply: {content}");
            return Ok(reply(content, false, context));
        }

        if !allow_tools {
            // Tools were disabled for this request, so finish with whatever text
            // the model produced instead of failing the whole turn.
            let content = message.content.unwrap_or_default().trim().to_string();
            let content = if content.is_empty() {
                "I gathered the results but hit the tool limit before finishing.".to_string()
            } else {
                content
            };
            eprintln!("kleep assistant: reply: {content}");
            return Ok(reply(content, false, context));
        }

        tool_round += 1;

        conversation.push(json!({
            "role": "assistant",
            "content": message.content,
            "tool_calls": message.tool_calls.clone(),
        }));

        for call in message.tool_calls {
            let cache_key = match serde_json::from_str::<Value>(&call.function.arguments) {
                Ok(arguments) => format!("{}\u{0}{arguments}", call.function.name),
                Err(_) => format!("{}\u{0}{}", call.function.name, call.function.arguments),
            };
            if let Some(previous) = seen_calls.get(&cache_key) {
                eprintln!(
                    "kleep assistant: reusing earlier result for {}",
                    call.function.name
                );
                conversation.push(json!({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": format!(
                        "This exact call already ran earlier in this turn. Its result was:\n\
                         {previous}\nDo not run it again; use this result."
                    ),
                }));
                continue;
            }

            let outcome: ToolOutcome =
                tools::execute(&call.function.name, &call.function.arguments, &mut context).await;

            eprintln!(
                "kleep assistant: tool result {} ({}) : {}",
                call.function.name,
                if outcome.opened { "action" } else { "data" },
                truncate_chars(&outcome.content, 1000)
            );

            seen_calls.insert(cache_key, outcome.content.clone());
            opened_app |= outcome.opened;

            conversation.push(json!({
                "role": "tool",
                "tool_call_id": call.id,
                "content": outcome.content,
            }));
        }

        // Opening an app or link is a complete action on its own; don't ask
        // the model for a spoken/visible confirmation.
        if opened_app {
            eprintln!("kleep assistant: action completed; closing the palette");
            return Ok(reply(String::new(), true, context));
        }
    }
}
