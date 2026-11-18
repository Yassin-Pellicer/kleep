use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod apps;
mod assistant;
mod clipboard;
mod dictionary;
mod structs;
mod voice;

const MAIN_WINDOW: &str = "main";
const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT_RATIO: f64 = 0.5;
const FOCUS_GRACE: Duration = Duration::from_millis(400);
const FADE: Duration = Duration::from_millis(140);
/// Small pause after hiding so the compositor can move focus back to the
/// window that was focused before the palette opened.
const PASTE_SETTLE: Duration = Duration::from_millis(20);

/// When the palette was last shown. Focus-loss events that fire while the
/// window is still being mapped must not hide it again.
static LAST_SHOWN: Mutex<Option<Instant>> = Mutex::new(None);

/// Invalidates a pending fade-out when the palette is shown again.
static HIDE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Set when Enter requests a clipboard paste; consumed once the palette hides.
static PENDING_PASTE: AtomicBool = AtomicBool::new(false);

/// Original GNOME overlay key value, saved while the palette is visible.
static OVERLAY_KEY: Mutex<Option<String>> = Mutex::new(None);

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
}

fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
}

fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|desktop| desktop.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
}

fn overview_is_active() -> bool {
    if !is_gnome() {
        return false;
    }

    let Ok(output) = Command::new("gdbus")
        .args([
            "call",
            "-t",
            "1",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.gnome.Shell",
            "OverviewActive",
        ])
        .output()
    else {
        return false;
    };

    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("true")
}

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
fn recover_overlay_key() {
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
fn disable_overlay_key() {
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

fn restore_overlay_key() {
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

/// Size the window to half the monitor height. A compositor that centers new
/// windows will then place its top edge at 25% of the screen; the bar itself is
/// drawn at the top of the window with CSS.
fn fit_to_screen(window: &WebviewWindow) {
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    else {
        return;
    };

    let logical_height = monitor.size().height as f64 / monitor.scale_factor();
    let size = LogicalSize::new(WINDOW_WIDTH, logical_height * WINDOW_HEIGHT_RATIO);

    let _ = window.set_resizable(true);
    let _ = window.set_size(size);
    let _ = window.set_min_size(Some(size));
    let _ = window.set_max_size(Some(size));
    let _ = window.center();
}

fn show_palette(window: &WebviewWindow) {
    // Do not pop Kleep over the GNOME Overview.
    if overview_is_active() {
        return;
    }

    HIDE_GENERATION.fetch_add(1, Ordering::SeqCst);
    // A show cancels any paste that was waiting for the previous hide.
    PENDING_PASTE.store(false, Ordering::SeqCst);

    if let Ok(mut last) = LAST_SHOWN.lock() {
        *last = Some(Instant::now());
    }

    // Keep Super from opening the Overview while the palette is visible.
    disable_overlay_key();

    if window.show().is_err() {
        restore_overlay_key();
        return;
    }
    let _ = window.set_focus();

    // The webview can be suspended while the window is hidden, so give it a
    // moment to resume before asking the frontend to play the fade-in.
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        let _ = window.emit("palette-shown", ());
    });
}

fn hide_with_fade(window: WebviewWindow) {
    let generation = HIDE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = window.emit("palette-hide", ());

    std::thread::spawn(move || {
        std::thread::sleep(FADE);
        if HIDE_GENERATION.load(Ordering::SeqCst) == generation {
            let _ = window.hide();
            restore_overlay_key();

            // Paste as soon as the palette is gone rather than after a fixed
            // delay. Focus moves back to the previous window as it hides.
            if PENDING_PASTE.swap(false, Ordering::SeqCst) {
                std::thread::sleep(PASTE_SETTLE);
                if !window.is_visible().unwrap_or(false) {
                    let _ = clipboard::send_paste_shortcut();
                }
            }
        }
    });
}

fn toggle_palette(app: &AppHandle) {
    let Some(window) = main_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide_with_fade(window);
    } else {
        show_palette(&window);
    }
}

fn register_shortcuts(app: &AppHandle) {
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

#[tauri::command]
fn hide_palette(app: AppHandle) {
    if let Some(window) = main_window(&app) {
        hide_with_fade(window);
    }
}

#[tauri::command]
fn clipboard_paste(id: String, app: AppHandle) -> Result<(), String> {
    let window = main_window(&app).ok_or_else(|| "palette window not found".to_string())?;

    // Fails before hiding the palette if /dev/uinput is unavailable.
    clipboard::prepare_paste()?;
    clipboard::copy_to_clipboard(&id)?;

    PENDING_PASTE.store(true, Ordering::SeqCst);
    hide_with_fade(window);

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Loads DEEPSEEK_API_KEY from a .env file during development. The key is
    // only ever read by the Rust side and never exposed to the webview.
    let _ = dotenvy::dotenv();

    // Restore the GNOME overlay key if a previous run was killed while open.
    recover_overlay_key();

    tauri::Builder::default()
        // Must be registered first: a second launch forwards its arguments here.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == "--toggle") {
                toggle_palette(app);
            } else if let Some(window) = main_window(app) {
                show_palette(&window);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_palette(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            hide_palette,
            dictionary::lookup_word,
            apps::search_apps,
            apps::launch_app,
            assistant::assistant_chat,
            clipboard::clipboard_list,
            clipboard::clipboard_copy,
            clipboard::clipboard_delete,
            clipboard::clipboard_clear,
            clipboard_paste,
            voice::voice_start_recording,
            voice::voice_recording_status,
            voice::voice_stop_recording,
            voice::voice_cancel_recording,
            voice::voice_speak,
            voice::voice_stop_speaking
        ])
        .setup(|app| {
            std::thread::spawn(apps::preload);
            clipboard::start();
            // Create the paste device ahead of time so the first Enter press
            // doesn't wait for the compositor to discover it.
            std::thread::spawn(|| {
                let _ = clipboard::prepare_paste();
            });
            voice::preload();

            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                fit_to_screen(&window);
            }

            if is_wayland() {
                eprintln!(
                    "kleep: Wayland detected; bind a desktop shortcut to `kleep --toggle` \
                     (the built-in shortcut only works on X11)."
                );
            } else {
                register_shortcuts(app.handle());
            }

            if std::env::args().skip(1).any(|arg| arg == "--toggle") {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                    show_palette(&window);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                let grace_over = LAST_SHOWN
                    .lock()
                    .ok()
                    .and_then(|last| *last)
                    .is_none_or(|last| last.elapsed() >= FOCUS_GRACE);
                if grace_over {
                    if let Some(webview) = window.app_handle().get_webview_window(MAIN_WINDOW) {
                        hide_with_fade(webview);
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                restore_overlay_key();
                voice::shutdown();
            }
        });
}
