use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::app::gnome;
use crate::services::clipboard;

use super::main_window;
use super::state;

const FADE: Duration = Duration::from_millis(140);
/// Small pause after hiding so the compositor can move focus back to the
/// window that was focused before the palette opened.
const PASTE_SETTLE: Duration = Duration::from_millis(20);

fn reveal_palette(window: &WebviewWindow) {
    state::invalidate_hides();
    state::cancel_pending_paste();
    state::mark_shown();

    // Keep Super from opening the Overview while the palette is visible.
    gnome::disable_overlay_key();

    if window.show().is_err() {
        gnome::restore_overlay_key();
        return;
    }
    let _ = window.set_focus();

    // The webview can be suspended while the window is hidden, so give it a
    // moment to resume before asking the frontend to play the fade-in.
    let window = window.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(60));
        let _ = window.emit("palette-shown", ());
    });
}

pub fn show_palette(window: &WebviewWindow) {
    // Do not pop Kleep over the GNOME Overview.
    if gnome::overview_is_active() {
        return;
    }

    reveal_palette(window);
}

/// Opening Kleep from the application grid happens while the GNOME Overview
/// is still closing. Wait for it to settle instead of silently doing nothing.
pub fn show_palette_on_launch(window: &WebviewWindow) {
    if !gnome::overview_is_active() {
        reveal_palette(window);
        return;
    }

    let window = window.clone();
    thread::spawn(move || {
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(150));
            if !gnome::overview_is_active() {
                reveal_palette(&window);
                return;
            }
        }

        reveal_palette(&window);
    });
}

pub fn hide_with_fade(window: WebviewWindow) {
    let generation = state::begin_hide();
    let _ = window.emit("palette-hide", ());

    thread::spawn(move || {
        thread::sleep(FADE);
        if !state::hide_is_current(generation) {
            return;
        }

        let _ = window.hide();
        gnome::restore_overlay_key();

        // Paste as soon as the palette is gone rather than after a fixed
        // delay. Focus moves back to the previous window as it hides.
        if state::take_pending_paste() {
            thread::sleep(PASTE_SETTLE);
            if !window.is_visible().unwrap_or(false) {
                let _ = clipboard::send_paste_shortcut();
            }
        }
    });
}

pub fn toggle_palette(app: &AppHandle) {
    let Some(window) = main_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide_with_fade(window);
    } else {
        show_palette(&window);
    }
}
