use crate::models::{DictionaryEntry, LookupError};
use crate::services::dictionary;

#[tauri::command]
pub async fn lookup_word(word: String) -> Result<DictionaryEntry, LookupError> {
    dictionary::lookup(word).await
}
