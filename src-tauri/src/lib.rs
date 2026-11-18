use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod apps;
mod assistant;
mod dictionary;
mod structs;
mod voice;

const MAIN_WINDOW: &str = "main";
const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT_RATIO: f64 = 0.5;
const FOCUS_GRACE: Duration = Duration::from_millis(400);
const FADE: Duration = Duration::from_millis(140);

/// When the palette was last shown. Focus-loss events that fire while the
/// window is still being mapped must not hide it again.
static LAST_SHOWN: Mutex<Option<Instant>> = Mutex::new(None);

/// Invalidates a pending fade-out when the palette is shown again.
static HIDE_GENERATION: AtomicU64 = AtomicU64::new(0);

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
}

fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
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
    HIDE_GENERATION.fetch_add(1, Ordering::SeqCst);

    if let Ok(mut last) = LAST_SHOWN.lock() {
        *last = Some(Instant::now());
    }

    let _ = window.show();
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Loads DEEPSEEK_API_KEY from a .env file during development. The key is
    // only ever read by the Rust side and never exposed to the webview.
    let _ = dotenvy::dotenv();

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
            voice::voice_start_recording,
            voice::voice_recording_status,
            voice::voice_stop_recording,
            voice::voice_cancel_recording,
            voice::voice_speak,
            voice::voice_stop_speaking
        ])
        .setup(|app| {
            std::thread::spawn(apps::preload);
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
                voice::shutdown();
            }
        });
}
