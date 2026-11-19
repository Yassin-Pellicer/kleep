use serde::Serialize;

/// Serializable error shared by every service command.
///
/// `AssistantError` and `LookupError` are aliases of this type so the
/// frontend keeps receiving the exact `{ "kind": ..., "message": ... }`
/// shape it received before.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum ServiceError {
    MissingApiKey,
    Unauthorized(String),
    RateLimited(String),
    Timeout(String),
    Network(String),
    BadResponse(String),
    Server(String),
    InvalidInput(String),
}
