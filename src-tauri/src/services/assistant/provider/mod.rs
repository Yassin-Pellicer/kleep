mod model;
mod native;
mod openai;
mod types;

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::models::ServiceError;
use crate::services::support::http::{RequestFailure, classify_request_error, client};
use crate::services::support::text::truncate_chars;

pub use model::{ModelConfig, config, preload};
pub use types::AssistantMessage;

pub(super) static CLIENT: LazyLock<Client> =
    LazyLock::new(|| client(Duration::from_secs(30), Duration::from_secs(5)));

async fn post_chat(config: &ModelConfig, body: &Value) -> Result<reqwest::Response, ServiceError> {
    let mut request = CLIENT.post(&config.url).timeout(config.timeout).json(body);
    if let Some(api_key) = &config.api_key {
        request = request.bearer_auth(api_key);
    }
    request.send().await.map_err(map_request_error)
}

pub async fn complete(
    config: &ModelConfig,
    messages: &[Value],
    tools: &[Value],
    voice: bool,
    allow_tools: bool,
) -> Result<AssistantMessage, ServiceError> {
    let temperature = if voice { 0.4 } else { 0.3 };
    let max_tokens: u32 = if config.local {
        1_200
    } else if voice {
        220
    } else {
        600
    };

    let body = if config.local {
        native::request_body(
            config,
            messages,
            temperature,
            max_tokens,
            tools,
            allow_tools,
        )
    } else {
        openai::request_body(
            config,
            messages,
            temperature,
            max_tokens,
            tools,
            allow_tools,
        )
    };

    let response = post_chat(config, &body).await?;
    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(map_status(status, error_body));
    }

    if config.local {
        native::read(response).await
    } else {
        openai::read(response).await
    }
}

fn map_request_error(error: reqwest::Error) -> ServiceError {
    match classify_request_error(error) {
        RequestFailure::Timeout(message) => ServiceError::Timeout(message),
        RequestFailure::Network(message) => ServiceError::Network(message),
    }
}

fn map_status(status: StatusCode, body: String) -> ServiceError {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        ServiceError::Unauthorized("the model service rejected the API key".to_string())
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        ServiceError::RateLimited("the model service rate limit was reached".to_string())
    } else {
        ServiceError::Server(format!(
            "the model service returned {status}: {}",
            truncate_chars(&body, 300)
        ))
    }
}
