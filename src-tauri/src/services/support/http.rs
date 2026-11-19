use std::time::Duration;

use reqwest::Client;

/// Builds an HTTP client with the timeouts a provider needs.
pub fn client(timeout: Duration, connect_timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .build()
        .expect("failed to build HTTP client")
}

/// Transport-level failure, mapped to a domain error by each provider.
pub enum RequestFailure {
    Timeout(String),
    Network(String),
}

pub fn classify_request_error(error: reqwest::Error) -> RequestFailure {
    if error.is_timeout() {
        RequestFailure::Timeout(error.to_string())
    } else {
        RequestFailure::Network(error.to_string())
    }
}
