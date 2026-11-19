mod request;
mod response;

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};

use crate::models::{DictionaryEntry, ServiceError};
use crate::services::support::http::{RequestFailure, classify_request_error, client};
use crate::services::support::text::truncate_chars;

const API_URL: &str = "https://api.deepseek.com/beta/chat/completions";

static CLIENT: LazyLock<Client> =
    LazyLock::new(|| client(Duration::from_secs(20), Duration::from_secs(5)));

pub async fn lookup(api_key: &str, word: &str) -> Result<DictionaryEntry, ServiceError> {
    // The provider's strict mode is normally reliable, but a malformed tool
    // call can still slip through. One retry is cheap insurance.
    match request_once(api_key, word).await {
        Err(ServiceError::BadResponse(_)) => request_once(api_key, word).await,
        result => result,
    }
}

async fn request_once(api_key: &str, word: &str) -> Result<DictionaryEntry, ServiceError> {
    let response = CLIENT
        .post(API_URL)
        .bearer_auth(api_key)
        .json(&request::request_body(word))
        .send()
        .await
        .map_err(map_request_error)?;

    let status = response.status();

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(ServiceError::Unauthorized(format!(
            "DeepSeek rejected the API key ({status})"
        )));
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(ServiceError::RateLimited(
            "DeepSeek rate limit reached".to_string(),
        ));
    }

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ServiceError::Server(format!(
            "DeepSeek returned {status}: {}",
            truncate_chars(&body, 200)
        )));
    }

    let parsed: response::ChatResponse = response
        .json()
        .await
        .map_err(|error| ServiceError::BadResponse(error.to_string()))?;

    response::parse_entry(parsed, request::TOOL_NAME)
}

fn map_request_error(error: reqwest::Error) -> ServiceError {
    match classify_request_error(error) {
        RequestFailure::Timeout(message) => ServiceError::Timeout(message),
        RequestFailure::Network(message) => ServiceError::Network(message),
    }
}
