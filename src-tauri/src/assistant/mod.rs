mod deepseek;

use std::collections::HashSet;

use chrono::Local;
use serde::Deserialize;
use serde_json::json;

use crate::structs::{
    AssistantError, AssistantReply, ChatMessage, ChatSource, SearchResponse, SearchResult,
};

const MAX_TOOL_ROUNDS: usize = 3;
const SEARCH_RESULTS: u8 = 5;
const MAX_SOURCES: usize = 8;

#[derive(Debug, Deserialize)]
struct WebSearchArgs {
    query: String,
}

fn system_prompt(voice: bool) -> String {
    let now = Local::now();

    let style = if voice {
        "The user is speaking and will hear your answer read aloud. Answer in the same \
         language they used. Write exactly as it should be spoken: one or two short \
         sentences of plain prose. Never use parentheses, brackets, bullet points, lists, \
         Markdown, emoji, code, URLs, links, or source citations. Say the answer directly \
         instead of pointing to a page. Be concise, precise, and friendly, with a very \
         slight touch of humor when it fits naturally."
    } else {
        "Keep answers concise and on point: at most 3 short sentences, or a short bullet list \
         when listing facts. Use Markdown when it improves readability: short paragraphs, \
         bullet lists, bold for key terms, and inline links. When you use web_search, include \
         the source links inline as Markdown links. Do not add filler or repeat the question."
    };

    format!(
        "You are Kleep, a concise desktop assistant.\n\
         Today is {date}, local time {time} (UTC offset {offset}).\n\n\
         Use the web_search tool when the user asks about current events, news, prices, \
         recent releases, schedules, or anything time-sensitive or uncertain. Do not use it \
         for ordinary conversation, general knowledge, coding help, or personal actions.\n\
         When you use web_search, base your answer on the returned summaries. Never invent \
         search results.\n\
         If no tool is needed, answer directly.\n\n\
         {style}\n\
         If the user asks for an action you cannot perform with the available tools, say so \
         briefly.",
        date = now.format("%Y-%m-%d (%A)"),
        time = now.format("%H:%M"),
        offset = now.format("%:z"),
    )
}

#[tauri::command]
pub async fn assistant_chat(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, AssistantError> {
    let voice = voice.unwrap_or(false);
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(AssistantError::MissingApiKey)?;

    // Keep the context bounded: recent turns only, and cap each message.
    let history: Vec<ChatMessage> = messages
        .into_iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut conversation = vec![json!({
        "role": "system",
        "content": system_prompt(voice),
    })];

    for message in history {
        let role = match message.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };

        let content: String = message.content.chars().take(8_000).collect();
        if content.trim().is_empty() {
            continue;
        }

        conversation.push(json!({
            "role": role,
            "content": content,
        }));
    }

    if conversation.len() <= 1 {
        return Err(AssistantError::InvalidInput(
            "no user message provided".to_string(),
        ));
    }

    let mut sources: Vec<ChatSource> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut tool_round = 0;

    loop {
        let message = deepseek::complete(&api_key, &conversation, voice).await?;

        if message.tool_calls.is_empty() {
            let content = message.content.unwrap_or_default().trim().to_string();
            if content.is_empty() {
                return Err(AssistantError::BadResponse(
                    "the model returned an empty answer".to_string(),
                ));
            }

            return Ok(AssistantReply { content, sources });
        }

        if tool_round >= MAX_TOOL_ROUNDS {
            return Err(AssistantError::Server(
                "the assistant used too many tool calls".to_string(),
            ));
        }
        tool_round += 1;

        conversation.push(json!({
            "role": "assistant",
            "content": message.content,
            "tool_calls": message.tool_calls.clone(),
        }));

        for call in message.tool_calls {
            let result = if call.function.name == "web_search" {
                run_web_search(
                    &api_key,
                    &call.function.arguments,
                    &mut sources,
                    &mut seen_urls,
                )
                .await
            } else {
                format!("Unknown tool `{}`.", call.function.name)
            };

            conversation.push(json!({
                "role": "tool",
                "tool_call_id": call.id,
                "content": result,
            }));
        }
    }
}

async fn run_web_search(
    api_key: &str,
    arguments: &str,
    sources: &mut Vec<ChatSource>,
    seen_urls: &mut HashSet<String>,
) -> String {
    let args: WebSearchArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return "Invalid arguments for web_search.".to_string(),
    };

    match deepseek::search(api_key, &args.query, SEARCH_RESULTS).await {
        Ok(response) => {
            collect_sources(&response, sources, seen_urls);
            format_results(&args.query, &response)
        }
        Err(error) => format!(
            "Web search failed: {}. Answer from your own knowledge, and say you could not \
             search if the question needs current information.",
            friendly_error(&error)
        ),
    }
}

fn collect_sources(
    response: &SearchResponse,
    sources: &mut Vec<ChatSource>,
    seen_urls: &mut HashSet<String>,
) {
    for result in &response.results {
        if sources.len() >= MAX_SOURCES {
            break;
        }
        if result.url.is_empty() || !seen_urls.insert(result.url.clone()) {
            continue;
        }

        sources.push(ChatSource {
            title: if result.title.is_empty() {
                result.url.clone()
            } else {
                result.title.clone()
            },
            url: result.url.clone(),
            snippet: result.snippet.clone(),
            date: result.date.clone(),
        });
    }
}

fn format_results(query: &str, response: &SearchResponse) -> String {
    if response.results.is_empty() {
        return format!("No web results found for \"{query}\".");
    }

    let mut text = format!("Web results for \"{query}\":");

    for (index, result) in response
        .results
        .iter()
        .take(SEARCH_RESULTS as usize)
        .enumerate()
    {
        text.push_str(&format!(
            "\n\n{}. {}\nURL: {}\n",
            index + 1,
            display_title(result),
            result.url
        ));

        let summary = if result.summary.is_empty() {
            &result.snippet
        } else {
            &result.summary
        };
        text.push_str(summary);

        if let Some(date) = &result.date
            && !date.is_empty()
        {
            text.push_str(&format!("\nPublished: {date}"));
        }
    }

    text.push_str(
        "\n\nUse these summaries to answer. Cite source URLs in plain text when relevant.",
    );

    text
}

fn display_title(result: &SearchResult) -> &str {
    if result.title.is_empty() {
        &result.url
    } else {
        &result.title
    }
}

fn friendly_error(error: &AssistantError) -> &'static str {
    match error {
        AssistantError::MissingApiKey => "the API key is missing",
        AssistantError::Unauthorized(_) => "the search service rejected the API key",
        AssistantError::RateLimited(_) => "the search rate limit was reached",
        AssistantError::Timeout(_) => "the search timed out",
        AssistantError::Network(_) => "there is no connection to the search service",
        AssistantError::BadResponse(_) | AssistantError::Server(_) => {
            "the search service returned an error"
        }
        AssistantError::InvalidInput(_) => "the search query was invalid",
    }
}
