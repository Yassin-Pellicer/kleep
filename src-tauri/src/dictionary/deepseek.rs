use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::json;

use crate::structs::{ChatResponse, DictionaryEntry, LookupError};

const API_URL: &str = "https://api.deepseek.com/beta/chat/completions";
const MODEL: &str = "deepseek-flash";
const TOOL_NAME: &str = "return_dictionary_entry";
const NATIVE_LANGUAGE: &str = "English";

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client")
});

fn system_prompt() -> String {
    format!(
        "You are a multilingual dictionary. The user supplies exactly one word in an unknown \
         language. Detect the language of the word and call the `{TOOL_NAME}` tool exactly once.\n\n\
         Keep `word` in the original script and spelling exactly as the user typed it. Provide \
         the pronunciation as IPA.\n\n\
         Write `partOfSpeech`, `definition`, `example`, and `origin` in {NATIVE_LANGUAGE}. If the \
         word is not {NATIVE_LANGUAGE}, translate the example sentence into {NATIVE_LANGUAGE}. A \
         definition must be at most two sentences and an example at most one sentence. Provide \
         exactly three English synonyms or translations as a JSON array of strings, for example \
         [\"hello\", \"hi\", \"greetings\"]. Always return a definition; do not refuse or ask \
         questions."
    )
}

fn request_body(word: &str) -> serde_json::Value {
    json!({
        "model": MODEL,
        "thinking": { "type": "disabled" },
        "temperature": 0.2,
        "max_tokens": 800,
        "messages": [
            { "role": "system", "content": system_prompt() },
            { "role": "user", "content": word }
        ],
        "tools": [{
            "type": "function",
            "function": {
                "name": TOOL_NAME,
                "description": "Return exactly one dictionary entry for the user's word.",
                "strict": true,
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "word",
                        "pronunciation",
                        "partOfSpeech",
                        "definition",
                        "example",
                        "origin",
                        "synonyms"
                    ],
                    "properties": {
                        "word": {
                            "type": "string",
                            "description": "The word in its original script and spelling."
                        },
                        "pronunciation": {
                            "type": "string",
                            "description": "IPA pronunciation, e.g. /bɔ̃ʒuʁ/."
                        },
                        "partOfSpeech": {
                            "type": "string",
                            "description": "Part of speech in English, e.g. noun, verb, adjective."
                        },
                        "definition": {
                            "type": "string",
                            "description": "Definition in English, at most two sentences."
                        },
                        "example": {
                            "type": "string",
                            "description": "One example sentence in English, translated if needed."
                        },
                        "origin": {
                            "type": "string",
                            "description": "Brief etymology in English."
                        },
                        "synonyms": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Exactly three English synonyms or translations. Must be a JSON array of strings, e.g. [\"hello\", \"hi\", \"greetings\"]."
                        }
                    }
                }
            }
        }],
        "tool_choice": {
            "type": "function",
            "function": { "name": TOOL_NAME }
        }
    })
}

fn map_request_error(error: reqwest::Error) -> LookupError {
    if error.is_timeout() {
        LookupError::Timeout(error.to_string())
    } else {
        LookupError::Network(error.to_string())
    }
}

fn truncate(text: &str) -> String {
    text.chars().take(200).collect()
}

pub async fn lookup(api_key: &str, word: &str) -> Result<DictionaryEntry, LookupError> {
    // The provider's strict mode is normally reliable, but a malformed tool
    // call can still slip through. One retry is cheap insurance.
    match request_once(api_key, word).await {
        Err(LookupError::BadResponse(_)) => request_once(api_key, word).await,
        result => result,
    }
}

async fn request_once(api_key: &str, word: &str) -> Result<DictionaryEntry, LookupError> {
    let response = CLIENT
        .post(API_URL)
        .bearer_auth(api_key)
        .json(&request_body(word))
        .send()
        .await
        .map_err(map_request_error)?;

    let status = response.status();

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(LookupError::Unauthorized(format!(
            "DeepSeek rejected the API key ({status})"
        )));
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(LookupError::RateLimited(
            "DeepSeek rate limit reached".to_string(),
        ));
    }

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(LookupError::Server(format!(
            "DeepSeek returned {status}: {}",
            truncate(&body)
        )));
    }

    let parsed: ChatResponse = response
        .json()
        .await
        .map_err(|error| LookupError::BadResponse(error.to_string()))?;

    let call = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.tool_calls.into_iter().next())
        .ok_or_else(|| {
            LookupError::BadResponse("the model did not return a tool call".to_string())
        })?;

    if call.function.name != TOOL_NAME {
        return Err(LookupError::BadResponse(format!(
            "unexpected tool `{}`",
            call.function.name
        )));
    }

    let entry: DictionaryEntry = serde_json::from_str(&call.function.arguments)
        .map_err(|error| LookupError::BadResponse(format!("invalid tool arguments: {error}")))?;

    if entry.word.trim().is_empty() || entry.definition.trim().is_empty() {
        return Err(LookupError::BadResponse(
            "the model returned an empty entry".to_string(),
        ));
    }

    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::request_body;

    #[test]
    fn request_is_strict_non_thinking_tool_call() {
        let body = request_body("hello");

        assert_eq!(body["model"], "deepseek-flash");
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["tools"][0]["function"]["strict"], true);
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["additionalProperties"],
            false
        );
        assert_eq!(
            body["tool_choice"]["function"]["name"],
            "return_dictionary_entry"
        );
    }
}
