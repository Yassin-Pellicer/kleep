use std::fs;
use std::path::PathBuf;

use crate::models::ClipboardEntry;

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn history_path() -> PathBuf {
    data_home().join("kleep").join("clipboard.json")
}

pub(super) fn load_history() -> Vec<ClipboardEntry> {
    let Ok(bytes) = fs::read(history_path()) else {
        return Vec::new();
    };

    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub(super) fn save_history(entries: &[ClipboardEntry]) {
    let path = history_path();
    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let Ok(data) = serde_json::to_vec(entries) else {
        return;
    };

    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, data).is_err() {
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
    }

    let _ = fs::rename(temporary, path);
}
