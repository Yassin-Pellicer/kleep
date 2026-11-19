use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;

use crate::models::ServiceError;
use crate::services::support::http::{RequestFailure, classify_request_error, client};
use crate::services::support::text::truncate_chars;

const SEARCH_URL: &str = "https://api.deepseek.com/v1/tools/search";

static CLIENT: LazyLock<Client> =
    LazyLock::new(|| client(Duration::from_secs(30), Duration::from_secs(5)));

/// Response shape from `POST /v1/tools/search`.
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub date: Option<String>,
}

pub async fn search(api_key: &str, query: &str, count: u8) -> Result<SearchResponse, ServiceError> {
    let query: String = query.trim().chars().take(128).collect();
    if query.is_empty() {
        return Err(ServiceError::InvalidInput(
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
        .map_err(|error| ServiceError::BadResponse(error.to_string()))
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
