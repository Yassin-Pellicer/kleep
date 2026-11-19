use serde::{Deserialize, Serialize};

/// The entry shape consumed by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub word: String,
    pub pronunciation: String,
    pub part_of_speech: String,
    pub definition: String,
    pub example: String,
    pub origin: String,
    pub synonyms: Vec<String>,
}
