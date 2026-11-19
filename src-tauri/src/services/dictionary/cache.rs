use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::models::DictionaryEntry;

static CACHE: LazyLock<Mutex<HashMap<String, DictionaryEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn get(key: &str) -> Option<DictionaryEntry> {
    CACHE.lock().ok().and_then(|cache| cache.get(key).cloned())
}

pub(super) fn insert(key: String, entry: DictionaryEntry) {
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(key, entry);
    }
}
