mod encode;
mod locate;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn data_uri(icon: &str) -> Option<String> {
    let icon = icon.trim();
    if icon.is_empty() {
        return None;
    }

    if let Ok(cache) = CACHE.lock()
        && let Some(value) = cache.get(icon)
    {
        return value.clone();
    }

    let value = locate::resolve(icon).and_then(|path| encode::data_uri(&path));

    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(icon.to_string(), value.clone());
    }

    value
}
