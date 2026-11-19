use serde_json::json;

const DEFAULT_MODEL: &str = "deepseek-flash";
pub(super) const TOOL_NAME: &str = "return_dictionary_entry";
const NATIVE_LANGUAGE: &str = "English";

fn model() -> String {
    std::env::var("KLEEP_DICTIONARY_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

fn system_prompt() -> String {
    format!(
        "You are a multilingual dictionary. The user supplies a word or short phrase in an \
         unknown language. Detect the language of the input and call the `{TOOL_NAME}` tool \
         exactly once.\n\n\
         Keep `word` in the original script and spelling exactly as the user typed it. Provide \
         the pronunciation as IPA.\n\n\
         Write `partOfSpeech`, `definition`, `example`, and `origin` in {NATIVE_LANGUAGE}. If the \
         input is not {NATIVE_LANGUAGE}, translate the example sentence into {NATIVE_LANGUAGE}. A \
         definition must be at most two sentences and an example at most one sentence. Provide \
         exactly three English synonyms or translations as a JSON array of strings, for example \
         [\"hello\", \"hi\", \"greetings\"]. Always return a definition; do not refuse or ask \
         questions."
    )
}

pub(super) fn request_body(word: &str) -> serde_json::Value {
    json!({
        "model": model(),
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
                "description": "Return exactly one dictionary entry for the user's word or phrase.",
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
                            "description": "The word or phrase in its original script and spelling."
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
