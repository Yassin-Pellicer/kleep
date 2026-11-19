use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use super::overview::is_gnome;

/// Original GNOME overlay key value, saved while the palette is visible.
static OVERLAY_KEY: Mutex<Option<String>> = Mutex::new(None);

fn overlay_key_cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;

    Some(base.join("kleep").join("overlay-key"))
}

fn gsettings(args: &[&str]) -> Option<std::process::Output> {
    Command::new("gsettings")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
}

/// Restores an overlay key left behind by a crash or forced shutdown.
pub fn recover_overlay_key() {
    if !is_gnome() {
        return;
    }

    let Some(path) = overlay_key_cache_path() else {
        return;
    };
    let Ok(saved) = std::fs::read_to_string(&path) else {
        return;
    };
    let saved = saved.trim().to_string();

    if saved.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }

    if gsettings(&["set", "org.gnome.mutter", "overlay-key", saved.as_str()]).is_some() {
        let _ = std::fs::remove_file(path);
    }
}

/// Disables the Super/Activities key while the palette is open.
pub fn disable_overlay_key() {
    if !is_gnome() {
        return;
    }

    let Ok(mut saved) = OVERLAY_KEY.lock() else {
        return;
    };

    if saved.is_some() {
        return;
    }

    let Some(output) = gsettings(&["get", "org.gnome.mutter", "overlay-key"]) else {
        return;
    };

    let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if current.is_empty() || current == "''" {
        return;
    }

    // Save the original first so a crash cannot leave Super permanently disabled.
    let Some(path) = overlay_key_cache_path() else {
        return;
    };
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if std::fs::write(&path, &current).is_err() {
        return;
    }

    if gsettings(&["set", "org.gnome.mutter", "overlay-key", "''"]).is_some() {
        *saved = Some(current);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

pub fn restore_overlay_key() {
    let saved = OVERLAY_KEY.lock().ok().and_then(|mut saved| saved.take());

    let Some(saved) = saved else {
        return;
    };

    if gsettings(&["set", "org.gnome.mutter", "overlay-key", saved.as_str()]).is_some()
        && let Some(path) = overlay_key_cache_path()
    {
        let _ = std::fs::remove_file(path);
    }
}
