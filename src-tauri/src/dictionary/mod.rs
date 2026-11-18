mod deepseek;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::structs::{DictionaryEntry, LookupError};

static CACHE: LazyLock<Mutex<HashMap<String, DictionaryEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[tauri::command]
pub async fn lookup_word(word: String) -> Result<DictionaryEntry, LookupError> {
    let word = word.trim().to_string();

    if word.is_empty() || word.split_whitespace().count() != 1 {
        return Err(LookupError::InvalidInput(
            "expected exactly one word".to_string(),
        ));
    }

    let key = word.to_lowercase();

    if let Some(entry) = CACHE.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
        return Ok(entry);
    }

    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(LookupError::MissingApiKey)?;

    let entry = deepseek::lookup(&api_key, &word).await?;

    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(key, entry.clone());
    }

    Ok(entry)
}
