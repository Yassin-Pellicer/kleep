use std::env;
use std::time::Duration;

use serde_json::json;

use crate::models::ServiceError;

use super::CLIENT;

const DEEPSEEK_CHAT_URL: &str = "https://api.deepseek.com/beta/chat/completions";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-flash";
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "qwen3.5:9b";
/// Keep the local model resident so idle gaps don't trigger another cold load.
pub(super) const OLLAMA_KEEP_ALIVE: &str = "30m";
/// Ollama loads qwen3.5 with a 4096-token window by default, which the Kleep
/// prompt plus tool output overflows. Ask for more room; the model supports it.
const DEFAULT_OLLAMA_NUM_CTX: u32 = 16_384;
const OLLAMA_TIMEOUT: Duration = Duration::from_secs(300);
const WARMUP_ATTEMPTS: usize = 6;
const WARMUP_RETRY_DELAY: Duration = Duration::from_secs(5);

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
    let provider = env::var("KLEEP_ASSISTANT_PROVIDER").unwrap_or_default();
    if !provider.eq_ignore_ascii_case("ollama") {
        return None;
    }

    let base = env::var("KLEEP_OLLAMA_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
    let model = env::var("KLEEP_OLLAMA_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
    let num_ctx = env::var("KLEEP_OLLAMA_NUM_CTX")
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
/// local Ollama model instead of DeepSeek, `KLEEP_OLLAMA_MODEL` to pick the
/// Ollama model, and `KLEEP_DEEPSEEK_MODEL` to pick the hosted one.
pub fn config(api_key: Option<&str>) -> Result<ModelConfig, ServiceError> {
    if let Some(config) = ollama_config() {
        return Ok(config);
    }

    let api_key = api_key
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ServiceError::MissingApiKey)?;
    let model = env::var("KLEEP_DEEPSEEK_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_DEEPSEEK_MODEL.to_string());

    Ok(ModelConfig {
        url: DEEPSEEK_CHAT_URL.to_string(),
        model,
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
