use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
}

pub fn register_shortcuts(app: &AppHandle) {
    let shortcuts = app.global_shortcut();
    let primary = Shortcut::new(Some(Modifiers::ALT), Code::Space);

    if let Err(error) = shortcuts.register(primary) {
        eprintln!("kleep: Alt+Space unavailable ({error}); trying Alt+Shift+Space");
        let fallback = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Space);
        if let Err(error) = shortcuts.register(fallback) {
            eprintln!("kleep: no global shortcut registered ({error})");
        }
    }
}
