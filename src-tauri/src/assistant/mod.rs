mod deepseek;

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::process::Stdio;
use std::time::Duration;

use chrono::Local;
use serde::Deserialize;
use serde_json::json;

use crate::structs::{
    AssistantError, AssistantReply, ChatMessage, ChatSource, SearchResponse, SearchResult,
};

const MAX_TOOL_ROUNDS: usize = 3;
const MAX_TOOL_ROUNDS_LOCAL: usize = 6;
const SEARCH_RESULTS: u8 = 5;
const SEARCH_RESULTS_LOCAL: u8 = 3;
const MAX_SOURCES: usize = 8;
const MAX_SUMMARY_CHARS: usize = 1_600;
const MAX_SUMMARY_CHARS_LOCAL: usize = 700;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_COMMAND_OUTPUT: usize = 4_000;
const MAX_RUNNING_PROGRAMS: usize = 120;

/// Warms the local model in the background when the app starts.
pub fn preload() {
    deepseek::preload();
}

#[derive(Debug, Deserialize)]
struct WebSearchArgs {
    query: String,
}

#[derive(Debug, Deserialize)]
struct RunCommandArgs {
    command: String,
}

#[derive(Debug, Deserialize)]
struct OpenAppArgs {
    #[serde(default)]
    app: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

fn system_prompt(voice: bool, web_search: bool) -> String {
    let now = Local::now();

    let search_instructions = if web_search {
        "Use the web_search tool when the user asks about current events, news, prices, \
         recent releases, schedules, or anything time-sensitive or uncertain. Do not use it \
         for ordinary conversation, general knowledge, coding help, or personal actions.\n\
         When you use web_search, base your answer on the returned summaries. Never invent \
         search results.\n"
    } else {
        ""
    };

    let style = if voice {
        "The user is speaking and will hear your answer read aloud. Answer in the same \
         language they used. Write exactly as it should be spoken: one or two short \
         sentences of plain prose. Never answer with a question or end with one. If you \
         would normally ask a follow-up or offer a choice, reformulate it as a statement \
         instead: instead of '¿Quieres que lo abra?' say 'Puedo abrirlo, si quieres.' Only \
         ask a question when the user explicitly asks you to. Never use parentheses, \
         brackets, bullet points, lists, Markdown, emoji, code, URLs, links, or source \
         citations. Say the answer directly instead of pointing to a page. Be concise, \
         precise, and friendly, with a very slight touch of humor when it fits naturally."
    } else {
        "Keep answers concise and on point: at most 3 short sentences, or a short bullet list \
         when listing facts. Use Markdown when it improves readability: short paragraphs, \
         bullet lists, bold for key terms, and inline links. When you use web_search, include \
         the source links inline as Markdown links. Do not add filler or repeat the question."
    };

    format!(
        "You are Kleep, a concise desktop assistant.\n\
         Today is {date}, local time {time} (UTC offset {offset}).\n\n\
         {search_instructions}\
         Use the open_app tool whenever the user asks to open, launch, or start something. \
         For installed desktop applications, put the app name in the app field. For \
         websites, online services, and links such as YouTube, Gmail, GitHub, or any URL, \
         put a full https URL in the url field instead, for example opening YouTube means \
         open_app with url https://youtube.com. When something could be either a website or \
         an installed app, prefer the url field for online services. This opens the user's \
         default browser. Only say something opened when the tool reports success.\n\
         Use the run_command tool when the user asks you to perform a CLI or system action \
         that open_app cannot handle, such as changing a setting, running a command, powering \
         off, rebooting, suspending, or closing the session. Use `systemctl poweroff`, \
         `systemctl reboot`, `systemctl suspend`, and \
         `loginctl terminate-session \"$XDG_SESSION_ID\"` for those actions, adding \
         `--no-block` to the systemctl commands so they return right away; if the session id \
         is empty, use `loginctl terminate-user \"$USER\"` instead. Use it to inspect \
         the system when the user asks about running processes, GPU usage, memory, disk, \
         services, or logs: run commands such as `ps aux`, `nvidia-smi`, `free -h`, `df -h`, \
         `systemctl --failed`, or `journalctl -n 50 --no-pager`. For per-process GPU memory \
         on NVIDIA, run \
         `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader`. \
         The command output is returned to you, so answer from it instead of guessing. For \
         GUI programs, launch them in the background and redirect output so the command \
         returns immediately, for example `nohup google-chrome-stable >/dev/null 2>&1 &`. \
         Only say an action succeeded when the tool reports success.\n\
         These execution rules override any hesitation:\n\
         - If the user asks for an action or for the real state of this machine, call the tool \
         in this same turn. Do not ask for permission first, do not answer with an offer such \
         as '¿Quieres que lo ejecute?', and do not say you will do it later. Read-only \
         inspection commands are always safe to run without asking.\n\
         - Never claim that a command or tool cannot do something unless you actually ran it \
         (or its `--help`) and saw it fail. Your assumptions about a program's version or \
         features are not evidence. Never invent command output, process names, memory \
         figures, versions, or limitations.\n\
         - Report numbers, names, and statuses exactly as the tool output gives them. If the \
         output answers the question, answer from it; never replace it with generic advice or \
         point the user at another program such as btop or htop.\n\
         - Interactive monitors hang. If a command times out, retry with a bounded form: \
         `nvidia-smi pmon -c 1` instead of `nvidia-smi pmon`, `top -b -n 1` instead of `top`, \
         or a non-interactive alternative instead of `watch` or `btop`.\n\
         If no tool is needed, answer directly.\n\n\
         {style}\n\
         If you answer in Spanish, use Peninsular Spanish from Spain (castellano): Spain \
         vocabulary and grammar, such as ordenador, móvil, conducir, and vosotros when \
         addressing a group. Avoid Latin American forms.\n\
         If the user asks for an action you cannot perform with the available tools, say so \
         briefly.",
        date = now.format("%Y-%m-%d (%A)"),
        time = now.format("%H:%M"),
        offset = now.format("%:z"),
        search_instructions = search_instructions,
    )
}

#[tauri::command]
pub async fn assistant_chat(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, AssistantError> {
    let result = assistant_chat_inner(messages, voice).await;
    if let Err(error) = &result {
        eprintln!("assistant_chat failed: {error:?}");
    }
    result
}

async fn assistant_chat_inner(
    messages: Vec<ChatMessage>,
    voice: Option<bool>,
) -> Result<AssistantReply, AssistantError> {
    let voice = voice.unwrap_or(false);
    let search_key = std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let config = deepseek::config(search_key.as_deref())?;
    let web_search = search_key.is_some();

    // Keep the context bounded: recent turns only, and cap each message.
    // Local models often run with a much smaller context window, so keep
    // proportionally less history for them.
    let max_messages = if config.local { 6 } else { 12 };
    let max_message_chars = if config.local { 3_000 } else { 8_000 };
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
        "content": system_prompt(voice, web_search),
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
        return Err(AssistantError::InvalidInput(
            "no user message provided".to_string(),
        ));
    }

    let mut sources: Vec<ChatSource> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut seen_calls: HashMap<String, String> = HashMap::new();
    let max_tool_rounds = if config.local {
        MAX_TOOL_ROUNDS_LOCAL
    } else {
        MAX_TOOL_ROUNDS
    };
    let mut tool_round = 0;
    let mut opened_app = false;

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
            deepseek::complete(&config, &conversation, voice, web_search, allow_tools).await?;

        if message.tool_calls.is_empty() {
            let content = message.content.unwrap_or_default().trim().to_string();
            if content.is_empty() {
                return Err(AssistantError::BadResponse(
                    "the model returned an empty answer".to_string(),
                ));
            }

            return Ok(AssistantReply { content, sources });
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
            return Ok(AssistantReply { content, sources });
        }

        tool_round += 1;

        conversation.push(json!({
            "role": "assistant",
            "content": message.content,
            "tool_calls": message.tool_calls.clone(),
        }));

        for call in message.tool_calls {
            let cache_key =
                match serde_json::from_str::<serde_json::Value>(&call.function.arguments) {
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

            let result = match call.function.name.as_str() {
                "web_search" => match search_key.as_deref() {
                    Some(api_key) => {
                        run_web_search(
                            api_key,
                            &call.function.arguments,
                            &mut sources,
                            &mut seen_urls,
                            config.local,
                        )
                        .await
                    }
                    None => "Web search is unavailable.".to_string(),
                },
                "run_command" => run_command(&call.function.arguments).await,
                "open_app" => {
                    let (result, opened) = run_open_app(&call.function.arguments).await;
                    opened_app |= opened;
                    result
                }
                other => format!("Unknown tool `{other}`."),
            };

            seen_calls.insert(cache_key, result.clone());

            conversation.push(json!({
                "role": "tool",
                "tool_call_id": call.id,
                "content": result,
            }));
        }

        // Opening an app or link is a complete action on its own; don't ask
        // the model for a spoken/visible confirmation.
        if opened_app {
            return Ok(AssistantReply {
                content: String::new(),
                sources,
            });
        }
    }
}

async fn run_open_app(arguments: &str) -> (String, bool) {
    let args: OpenAppArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ("Invalid arguments for open_app.".to_string(), false),
    };

    if let Some(url) = args
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        return open_url(url).await;
    }

    let app = match args.app.as_deref().map(str::trim) {
        Some(app) if !app.is_empty() => app.to_string(),
        _ => return ("Provide either an app name or a URL.".to_string(), false),
    };

    match tauri::async_runtime::spawn_blocking(move || crate::apps::launch_by_name(&app)).await {
        Ok(Ok(name)) => (format!("Opened {name}."), true),
        Ok(Err(error)) => (error, false),
        Err(error) => (format!("Could not open the app: {error}"), false),
    }
}

async fn open_url(url: &str) -> (String, bool) {
    let url = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return (
            "Only http and https links can be opened.".to_string(),
            false,
        );
    }

    let target = url.clone();
    match tauri::async_runtime::spawn_blocking(move || open_in_browser(&target)).await {
        Ok(Ok(())) => (format!("Opened {url} in the default browser."), true),
        Ok(Err(error)) => (error, false),
        Err(error) => (format!("Could not open the link: {error}"), false),
    }
}

fn open_in_browser(url: &str) -> Result<(), String> {
    let candidates: [(&str, &[&str]); 2] = [("xdg-open", &[]), ("gio", &["open"])];
    let mut last_error = String::new();

    for (program, args) in candidates {
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(error) => last_error = format!("{program}: {error}"),
        }
    }

    Err(format!("couldn't open the link ({last_error})"))
}

async fn run_command(arguments: &str) -> String {
    let args: RunCommandArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return "Invalid arguments for run_command.".to_string(),
    };

    let command = args.command.trim();
    if command.is_empty() {
        return "The command was empty.".to_string();
    }

    let mut process = tokio::process::Command::new("sh");
    process
        .arg("-lc")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let result = match tokio::time::timeout(COMMAND_TIMEOUT, process.output()).await {
        Err(_) => format!(
            "Command timed out after {} seconds. It may still be running in the background.",
            COMMAND_TIMEOUT.as_secs()
        ),
        Ok(Err(error)) => format!("Could not run the command: {error}"),
        Ok(Ok(output)) => describe_command_output(&output),
    };

    match running_programs_context() {
        Some(context) => format!("{result}\n\n{context}"),
        None => result,
    }
}

/// Lists the programs currently running for the current user. This is passed
/// back with every command result so the model knows what is open without
/// spending another tool round on `ps`.
fn running_programs_context() -> Option<String> {
    let uid = unsafe { libc::getuid() };
    let entries = std::fs::read_dir("/proc").ok()?;
    let mut names: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == 0 {
            continue;
        }

        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.uid() != uid {
            continue;
        }

        let Ok(comm) = std::fs::read_to_string(path.join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        if !comm.is_empty() {
            names.push(comm.to_string());
        }
    }

    if names.is_empty() {
        return None;
    }

    names.sort_unstable();
    names.dedup();
    let total = names.len();
    names.truncate(MAX_RUNNING_PROGRAMS);
    let mut list = names.join(", ");
    if total > names.len() {
        list.push_str(&format!(" … (+{} more)", total - names.len()));
    }

    Some(format!("programs currently running for this user:\n{list}"))
}

fn describe_command_output(output: &std::process::Output) -> String {
    let mut parts = vec![match output.status.code() {
        Some(code) if code == 0 => "exit status: success".to_string(),
        Some(code) => format!("exit status: {code}"),
        None => "exit status: terminated by a signal".to_string(),
    }];

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        parts.push(format!("stdout:\n{}", truncate_output(stdout)));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        parts.push(format!("stderr:\n{}", truncate_output(stderr)));
    }

    parts.join("\n")
}

fn truncate_output(text: &str) -> String {
    if text.chars().count() <= MAX_COMMAND_OUTPUT {
        return text.to_string();
    }

    let truncated: String = text.chars().take(MAX_COMMAND_OUTPUT).collect();
    format!("{truncated}… [output truncated]")
}

async fn run_web_search(
    api_key: &str,
    arguments: &str,
    sources: &mut Vec<ChatSource>,
    seen_urls: &mut HashSet<String>,
    local: bool,
) -> String {
    let args: WebSearchArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return "Invalid arguments for web_search.".to_string(),
    };

    let count = if local {
        SEARCH_RESULTS_LOCAL
    } else {
        SEARCH_RESULTS
    };
    let max_summary = if local {
        MAX_SUMMARY_CHARS_LOCAL
    } else {
        MAX_SUMMARY_CHARS
    };

    match deepseek::search(api_key, &args.query, count).await {
        Ok(response) => {
            collect_sources(&response, sources, seen_urls);
            format_results(&args.query, &response, max_summary)
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

fn format_results(query: &str, response: &SearchResponse, max_summary: usize) -> String {
    if response.results.is_empty() {
        return format!("No web results found for \"{query}\".");
    }

    let mut text = format!("Web results for \"{query}\":");

    for (index, result) in response.results.iter().enumerate() {
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
        text.push_str(&compact_text(summary, max_summary));

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

fn compact_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    let truncated: String = normalized.chars().take(max_chars).collect();
    format!("{truncated}…")
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
