use std::collections::HashMap;

use serde_json::{Value, json};

use crate::models::{AssistantReply, ServiceError};
use crate::tools::{self, ToolContext, ToolOutcome};

use super::provider::{self, ModelConfig};

const MAX_TOOL_ROUNDS: usize = 3;
const MAX_TOOL_ROUNDS_LOCAL: usize = 6;

fn reply(content: String, context: ToolContext<'_>) -> AssistantReply {
    AssistantReply {
        content,
        sources: context.sources,
    }
}

pub async fn run(
    config: &ModelConfig,
    mut conversation: Vec<Value>,
    voice: bool,
    web_search: bool,
    search_key: Option<&str>,
) -> Result<AssistantReply, ServiceError> {
    let mut context = ToolContext::new(search_key, config.local);
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

        if message.tool_calls.is_empty() {
            let content = message.content.unwrap_or_default().trim().to_string();
            if content.is_empty() {
                return Err(ServiceError::BadResponse(
                    "the model returned an empty answer".to_string(),
                ));
            }

            return Ok(reply(content, context));
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
            return Ok(reply(content, context));
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
            return Ok(reply(String::new(), context));
        }
    }
}
