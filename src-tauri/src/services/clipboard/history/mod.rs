mod hash;
mod persistence;

use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::ClipboardEntry;

const MAX_ENTRIES: usize = 200;

struct ClipboardStore {
    entries: Vec<ClipboardEntry>,
    last_seen: Option<String>,
}

static STORE: LazyLock<Mutex<ClipboardStore>> = LazyLock::new(|| {
    Mutex::new(ClipboardStore {
        entries: persistence::load_history(),
        last_seen: None,
    })
});

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Loads the saved history before the first command arrives.
pub fn init() {
    LazyLock::force(&STORE);
}

pub fn list() -> Vec<ClipboardEntry> {
    STORE
        .lock()
        .map(|store| store.entries.clone())
        .unwrap_or_default()
}

pub fn find(id: &str) -> Option<ClipboardEntry> {
    STORE
        .lock()
        .ok()
        .and_then(|store| store.entries.iter().find(|entry| entry.id == id).cloned())
}

/// Adds a clipboard capture, moving re-copied text to the top.
pub fn record(text: String) {
    let Ok(mut store) = STORE.lock() else {
        return;
    };

    if store.last_seen.as_deref() == Some(text.as_str()) {
        return;
    }
    store.last_seen = Some(text.clone());

    // Re-copying an old entry moves it to the top instead of
    // creating a duplicate row.
    store.entries.retain(|entry| entry.text != text);
    store.entries.insert(
        0,
        ClipboardEntry {
            id: hash::hash_text(&text),
            text,
            timestamp: now_ms(),
        },
    );
    store.entries.truncate(MAX_ENTRIES);

    let snapshot = store.entries.clone();
    drop(store);
    persistence::save_history(&snapshot);
}

pub fn delete(id: &str) {
    let Ok(mut store) = STORE.lock() else {
        return;
    };

    let before = store.entries.len();
    store.entries.retain(|entry| entry.id != id);
    if store.entries.len() == before {
        return;
    }

    let snapshot = store.entries.clone();
    drop(store);
    persistence::save_history(&snapshot);
}

pub fn clear() {
    let Ok(mut store) = STORE.lock() else {
        return;
    };

    store.entries.clear();
    let snapshot = store.entries.clone();
    drop(store);
    persistence::save_history(&snapshot);
}
