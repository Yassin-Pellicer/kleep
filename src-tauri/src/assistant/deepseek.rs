use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::structs::{AssistantError, SearchResponse};

const CHAT_URL: &str = "https://api.deepseek.com/beta/chat/completions";
const SEARCH_URL: &str = "https://api.deepseek.com/v1/tools/search";
const MODEL: &str = "deepseek-flash";

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
        AssistantError::Unauthorized("DeepSeek rejected the API key".to_string())
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        AssistantError::RateLimited("DeepSeek rate limit reached".to_string())
    } else {
        AssistantError::Server(format!("DeepSeek returned {status}: {}", truncate(&body)))
    }
}

pub async fn complete(
    api_key: &str,
    messages: &[serde_json::Value],
    voice: bool,
) -> Result<AssistantMessage, AssistantError> {
    let body = json!({
        "model": MODEL,
        "thinking": { "type": "disabled" },
        "temperature": if voice { 0.4 } else { 0.3 },
        "max_tokens": if voice { 220 } else { 600 },
        "messages": messages,
        "tools": [{
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
        }, {
            "type": "function",
            "function": {
                "name": "open_app",
                "description": "Open a desktop application by name. Use this whenever the user \
                                asks to open, launch, or start an app. It resolves the name \
                                against the applications installed on the user's machine.",
                "strict": true,
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["app"],
                    "properties": {
                        "app": {
                            "type": "string",
                            "description": "The application name, such as \"spotify\" or \"chrome\"."
                        }
                    }
                }
            }
        }, {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a non-interactive shell command on the user's Linux machine. \
                                Use this to perform desktop actions such as opening applications \
                                or changing system state. For GUI programs, launch them in the \
                                background and redirect output (for example \
                                `nohup google-chrome-stable >/dev/null 2>&1 &`) so the command \
                                returns immediately.",
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
        }],
        "tool_choice": "auto"
    });

    let response = CLIENT
        .post(CHAT_URL)
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

    let parsed: ChatCompletion = response
        .json()
        .await
        .map_err(|error| AssistantError::BadResponse(error.to_string()))?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message)
        .ok_or_else(|| AssistantError::BadResponse("empty model response".to_string()))
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
