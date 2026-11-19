use tauri::WindowEvent;
use tauri_plugin_global_shortcut::ShortcutState;

mod app;
mod commands;
mod models;
mod services;
mod tools;

use app::shortcuts::{is_wayland, register_shortcuts};
use app::window::{fit_to_screen, main_window, show_palette, toggle_palette};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Loads DEEPSEEK_API_KEY from a .env file during development. The key is
    // only ever read by the Rust side and never exposed to the webview.
    let _ = dotenvy::dotenv();

    // Restore the GNOME overlay key if a previous run was killed while open.
    app::gnome::recover_overlay_key();

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
            commands::palette::hide_palette,
            commands::dictionary::lookup_word,
            commands::apps::search_apps,
            commands::apps::launch_app,
            commands::assistant::assistant_chat,
            commands::clipboard::clipboard_list,
            commands::clipboard::clipboard_copy,
            commands::clipboard::clipboard_delete,
            commands::clipboard::clipboard_clear,
            commands::clipboard::clipboard_paste,
            commands::voice::voice_start_recording,
            commands::voice::voice_recording_status,
            commands::voice::voice_stop_recording,
            commands::voice::voice_cancel_recording,
            commands::voice::voice_speak,
            commands::voice::voice_stop_speaking
        ])
        .setup(|app| {
            std::thread::spawn(services::apps::preload);
            services::clipboard::start();
            // Create the paste device ahead of time so the first Enter press
            // doesn't wait for the compositor to discover it.
            std::thread::spawn(|| {
                let _ = services::clipboard::prepare_paste();
            });
            services::voice::preload();
            services::assistant::preload();

            if let Some(window) = main_window(app.handle()) {
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

            if std::env::args().skip(1).any(|arg| arg == "--toggle")
                && let Some(window) = main_window(app.handle())
            {
                show_palette(&window);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                app::window::handle_focus_lost(window);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                app::gnome::restore_overlay_key();
                services::voice::shutdown();
            }
        });
}
