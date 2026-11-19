use std::time::Duration;

use tauri::Manager;

use crate::services::clipboard;

use super::geometry::MAIN_WINDOW;
use super::state;
use super::visibility;

/// Focus-loss events that fire while the window is still being mapped, or
/// while a clipboard write borrows focus, must not hide the palette.
const FOCUS_GRACE: Duration = Duration::from_millis(400);

pub fn handle_focus_lost(window: &tauri::Window) {
    if clipboard::focus_hide_suppressed() {
        return;
    }

    if !state::focus_grace_over(FOCUS_GRACE) {
        return;
    }

    if let Some(webview) = window.app_handle().get_webview_window(MAIN_WINDOW) {
        visibility::hide_with_fade(webview);
    }
}
