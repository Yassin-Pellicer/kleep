use std::sync::{LazyLock, Mutex};

use super::desktop::{self, DesktopApp};

static APPS: LazyLock<Mutex<Vec<DesktopApp>>> = LazyLock::new(|| Mutex::new(desktop::load_apps()));

/// Scans desktop entries off the main thread so the first search is instant.
pub fn preload() {
    LazyLock::force(&APPS);
}

/// Runs `f` with the catalog while holding the lock.
pub(super) fn with_apps<T>(f: impl FnOnce(&[DesktopApp]) -> T) -> Option<T> {
    APPS.lock().ok().map(|apps| f(&apps))
}
