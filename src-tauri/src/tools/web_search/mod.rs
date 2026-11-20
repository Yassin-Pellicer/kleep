mod args;
mod format;

use serde_json::{Value, json};

use crate::services::search;

use super::context::{ToolContext, ToolOutcome};

const SEARCH_RESULTS: u8 = 5;
const SEARCH_RESULTS_LOCAL: u8 = 3;
const MAX_SUMMARY_CHARS: usize = 1_600;
const MAX_SUMMARY_CHARS_LOCAL: usize = 700;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web for current information. Use this for news, \
                            recent events, prices, schedules, or facts that may have changed. \
                            Do not use it to play or watch videos or music, or to look up \
                            something to play; use play_video instead.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "A concise web search query, up to 128 characters."
                    }
                }
            }
        }
    })
}

pub(super) async fn execute(arguments: &str, context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: args::WebSearchArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for web_search."),
    };

    let Some(api_key) = context.search_api_key else {
        return ToolOutcome::text("Web search is unavailable.");
    };

    let count = if context.local {
        SEARCH_RESULTS_LOCAL
    } else {
        SEARCH_RESULTS
    };
    let max_summary = if context.local {
        MAX_SUMMARY_CHARS_LOCAL
    } else {
        MAX_SUMMARY_CHARS
    };

    match search::search(api_key, &args.query, count).await {
        Ok(response) => {
            format::collect_sources(&response, &mut context.sources, &mut context.seen_urls);
            ToolOutcome::text(format::format_results(&args.query, &response, max_summary))
        }
        Err(error) => ToolOutcome::text(format!(
            "Web search failed: {}. Answer from your own knowledge, and say you could not \
             search if the question needs current information.",
            format::friendly_error(&error)
        )),
    }
}
