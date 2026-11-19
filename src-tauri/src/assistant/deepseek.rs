use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::structs::{AssistantError, SearchResponse};

const DEEPSEEK_CHAT_URL: &str = "https://api.deepseek.com/beta/chat/completions";
const SEARCH_URL: &str = "https://api.deepseek.com/v1/tools/search";
const DEEPSEEK_MODEL: &str = "deepseek-flash";
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "qwen3.5:9b";
/// Keep the local model resident so idle gaps don't trigger another cold load.
const OLLAMA_KEEP_ALIVE: &str = "30m";
/// Ollama loads qwen3.5 with a 4096-token window by default, which the Kleep
/// prompt plus tool output overflows. Ask for more room; the model supports it.
const DEFAULT_OLLAMA_NUM_CTX: u32 = 16_384;
const OLLAMA_TIMEOUT: Duration = Duration::from_secs(300);
const WARMUP_ATTEMPTS: usize = 6;
const WARMUP_RETRY_DELAY: Duration = Duration::from_secs(5);

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Deserialize)]
pub struct ChatCompletion {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: AssistantMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssistantMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

fn map_request_error(error: reqwest::Error) -> AssistantError {
    if error.is_timeout() {
        AssistantError::Timeout(error.to_string())
    } else {
        AssistantError::Network(error.to_string())
    }
}

fn truncate(text: &str) -> String {
    text.chars().take(300).collect()
}

fn map_status(status: StatusCode, body: String) -> AssistantError {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        AssistantError::Unauthorized("the model service rejected the API key".to_string())
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        AssistantError::RateLimited("the model service rate limit was reached".to_string())
    } else {
        AssistantError::Server(format!(
            "the model service returned {status}: {}",
            truncate(&body)
        ))
    }
}

pub struct ModelConfig {
    pub url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub local: bool,
    pub timeout: Duration,
    /// Context window requested from Ollama. The server defaults qwen3.5 to
    /// 4096, which is too small once the system prompt, tools and command
    /// output share the window, so ask for a larger one explicitly.
    pub num_ctx: u32,
    warmup_url: Option<String>,
}

fn ollama_config() -> Option<ModelConfig> {
    let provider = std::env::var("KLEEP_ASSISTANT_PROVIDER").unwrap_or_default();
    if !provider.eq_ignore_ascii_case("ollama") {
        return None;
    }

    let base = std::env::var("KLEEP_OLLAMA_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
    let model = std::env::var("KLEEP_OLLAMA_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
    let num_ctx = std::env::var("KLEEP_OLLAMA_NUM_CTX")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value >= 2_048)
        .unwrap_or(DEFAULT_OLLAMA_NUM_CTX);

    Some(ModelConfig {
        url: format!("{base}/api/chat"),
        warmup_url: Some(format!("{base}/api/generate")),
        model,
        api_key: None,
        local: true,
        timeout: OLLAMA_TIMEOUT,
        num_ctx,
    })
}

/// Selects the chat model. Set `KLEEP_ASSISTANT_PROVIDER=ollama` to use a
/// local Ollama model instead of DeepSeek.
pub fn config(api_key: Option<&str>) -> Result<ModelConfig, AssistantError> {
    if let Some(config) = ollama_config() {
        return Ok(config);
    }

    let api_key = api_key
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or(AssistantError::MissingApiKey)?;

    Ok(ModelConfig {
        url: DEEPSEEK_CHAT_URL.to_string(),
        model: DEEPSEEK_MODEL.to_string(),
        api_key: Some(api_key),
        local: false,
        timeout: Duration::from_secs(30),
        num_ctx: 0,
        warmup_url: None,
    })
}

/// Loads the local Ollama model into memory before the first prompt. Ollama
/// evicts idle models, and the next request then blocks while the weights are
/// read back from disk, which can take tens of seconds on a cold boot. Warm the
/// model in the background at startup (retrying while Ollama itself boots) and
/// keep it resident afterwards.
pub fn preload() {
    let Some(config) = ollama_config() else {
        return;
    };
    let Some(url) = config.warmup_url else {
        return;
    };

    tauri::async_runtime::spawn(async move {
        let body = json!({
            "model": config.model,
            "prompt": "",
            "stream": false,
            "keep_alive": OLLAMA_KEEP_ALIVE,
            "options": { "num_ctx": config.num_ctx },
        });

        for attempt in 1..=WARMUP_ATTEMPTS {
            let result = CLIENT
                .post(&url)
                .timeout(OLLAMA_TIMEOUT)
                .json(&body)
                .send()
                .await;

            match result {
                Ok(response) if response.status().is_success() => {
                    eprintln!("assistant: local model ready");
                    return;
                }
                Ok(response) => eprintln!(
                    "assistant: local model warmup failed (attempt {attempt}/{WARMUP_ATTEMPTS}): {}",
                    response.status()
                ),
                Err(error) => eprintln!(
                    "assistant: local model warmup failed (attempt {attempt}/{WARMUP_ATTEMPTS}): {error}"
                ),
            }

            if attempt < WARMUP_ATTEMPTS {
                tokio::time::sleep(WARMUP_RETRY_DELAY).await;
            }
        }
    });
}

async fn post_chat(
    config: &ModelConfig,
    body: &serde_json::Value,
) -> Result<reqwest::Response, AssistantError> {
    let mut request = CLIENT.post(&config.url).timeout(config.timeout).json(body);
    if let Some(api_key) = &config.api_key {
        request = request.bearer_auth(api_key);
    }
    request.send().await.map_err(map_request_error)
}

pub async fn complete(
    config: &ModelConfig,
    messages: &[serde_json::Value],
    voice: bool,
    web_search: bool,
    allow_tools: bool,
) -> Result<AssistantMessage, AssistantError> {
    let mut tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "open_app",
                "description": "Open a desktop application by name, or open a website/link in \
                                the default browser. Use the url field for websites and online \
                                services such as YouTube, Gmail, GitHub, or any link; for \
                                example opening YouTube is { \"url\": \"https://youtube.com\" }. \
                                Use the app field only for installed desktop applications. \
                                Do not pass a website name in the app field.",
                "strict": true,
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [],
                    "properties": {
                        "app": {
                            "type": "string",
                            "description": "An installed desktop application, such as \"spotify\" or \"chrome\"."
                        },
                        "url": {
                            "type": "string",
                            "description": "A full http(s) URL to open in the default browser, such as \"https://youtube.com\"."
                        }
                    }
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a non-interactive shell command on the user's Linux machine \
                                and return its exit status, stdout and stderr. Use it to perform \
                                system actions such as powering off (`systemctl poweroff`), \
                                rebooting (`systemctl reboot`), suspending (`systemctl suspend`), \
                                logging out (`loginctl terminate-session \
                                \"$XDG_SESSION_ID\"`), changing settings, or launching GUI \
                                programs. Also use it to inspect the system with read-only \
                                commands such as `ps aux`, `nvidia-smi`, `free -h`, `df -h`, \
                                `systemctl status`, `systemctl --failed`, `ip addr`, or \
                                `journalctl -n 50 --no-pager`; the output is returned to you, so \
                                use it to report real data. Commands run through `sh -lc` with a \
                                15 second timeout: add `--no-block` to power actions, and launch \
                                GUI programs in the background with output redirected (for \
                                example `nohup google-chrome-stable >/dev/null 2>&1 &`) so they \
                                return immediately. Interactive monitors never return on their \
                                own: use a bounded form such as `nvidia-smi pmon -c 1` or \
                                `top -b -n 1`, not bare `pmon`, `top`, `watch`, or `btop`. The \
                                result also includes the list of \
                                programs currently running for the user, so use that instead of \
                                making a separate call just to enumerate them.",
                "strict": true,
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["command"],
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to run."
                        }
                    }
                }
            }
        }),
    ];

    if web_search {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for current information. Use this for news, \
                                recent events, prices, schedules, or facts that may have changed.",
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
        }));
    }

    let temperature = if voice { 0.4 } else { 0.3 };
    let max_tokens: u32 = if config.local {
        1_200
    } else if voice {
        220
    } else {
        600
    };

    let body = if config.local {
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
    } else {
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
    };

    let response = post_chat(config, &body).await?;
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(map_status(status, error_body));
    }

    if config.local {
        read_native_message(response).await
    } else {
        read_message(response).await
    }
}

async fn read_message(response: reqwest::Response) -> Result<AssistantMessage, AssistantError> {
    let body = response.text().await.map_err(|error| {
        AssistantError::BadResponse(format!("could not read response: {error}"))
    })?;

    let parsed: ChatCompletion = serde_json::from_str(&body).map_err(|error| {
        AssistantError::BadResponse(format!(
            "invalid response JSON: {error}; body: {}",
            truncate(&body)
        ))
    })?;

    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| AssistantError::BadResponse("empty model response".to_string()))?;

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
        return Err(AssistantError::BadResponse(format!(
            "model returned no content or tool calls (finish_reason: {})",
            choice.finish_reason.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(choice.message)
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
    arguments: serde_json::Value,
}

async fn read_native_message(
    response: reqwest::Response,
) -> Result<AssistantMessage, AssistantError> {
    let body = response.text().await.map_err(|error| {
        AssistantError::BadResponse(format!("could not read response: {error}"))
    })?;

    let parsed: NativeChatCompletion = serde_json::from_str(&body).map_err(|error| {
        AssistantError::BadResponse(format!(
            "invalid response JSON: {error}; body: {}",
            truncate(&body)
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
                serde_json::Value::String(text) => text,
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
        return Err(AssistantError::BadResponse(format!(
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
fn native_messages(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
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

pub async fn search(
    api_key: &str,
    query: &str,
    count: u8,
) -> Result<SearchResponse, AssistantError> {
    let query: String = query.trim().chars().take(128).collect();
    if query.is_empty() {
        return Err(AssistantError::InvalidInput(
            "search query cannot be empty".to_string(),
        ));
    }

    let body = json!({
        "query": query,
        "count": count.clamp(1, 10),
        "need_content": false
    });

    let response = CLIENT
        .post(SEARCH_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(map_request_error)?;

    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(map_status(status, body));
    }

    response
        .json()
        .await
        .map_err(|error| AssistantError::BadResponse(error.to_string()))
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
