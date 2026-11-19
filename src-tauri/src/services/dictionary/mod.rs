mod cache;
mod provider;

use crate::models::{DictionaryEntry, ServiceError};
use crate::services::support::env;

pub async fn lookup(word: String) -> Result<DictionaryEntry, ServiceError> {
    let word = word.trim().to_string();

    if word.is_empty() || word.chars().count() > 200 {
        return Err(ServiceError::InvalidInput(
            "expected a word or short phrase".to_string(),
        ));
    }

    let key = word.to_lowercase();

    if let Some(entry) = cache::get(&key) {
        return Ok(entry);
    }

    let api_key = env::deepseek_api_key().ok_or(ServiceError::MissingApiKey)?;
    let entry = provider::lookup(&api_key, &word).await?;

    cache::insert(key, entry.clone());

    Ok(entry)
}
