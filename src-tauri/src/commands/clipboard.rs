use std::time::Duration;

use tauri::AppHandle;

use crate::app::window::{hide_with_fade, main_window, request_paste};
use crate::models::ClipboardEntry;
use crate::services::clipboard;

/// A clipboard write briefly borrows focus; keep the palette open this long.
const CLIPBOARD_FOCUS_GUARD: Duration = Duration::from_millis(1_500);

#[tauri::command]
pub fn clipboard_list() -> Vec<ClipboardEntry> {
    clipboard::list()
}

#[tauri::command]
pub fn clipboard_copy(id: String, app: AppHandle) -> Result<(), String> {
    // `wl-copy` borrows keyboard focus while it claims the selection, which
    // would otherwise trigger the focus-loss handler and close the palette.
    clipboard::suppress_focus_hide(CLIPBOARD_FOCUS_GUARD);

    let result = clipboard::copy_to_clipboard(&id);

    // Give focus back to the palette so the input keeps receiving keys.
    if let Some(window) = main_window(&app) {
        let _ = window.set_focus();
    }

    result
}

#[tauri::command]
pub fn clipboard_paste(id: String, app: AppHandle) -> Result<(), String> {
    let window = main_window(&app).ok_or_else(|| "palette window not found".to_string())?;

    // Fails before hiding the palette if /dev/uinput is unavailable.
    clipboard::prepare_paste()?;
    clipboard::copy_to_clipboard(&id)?;

    request_paste();
    hide_with_fade(window);

    Ok(())
}

#[tauri::command]
pub fn clipboard_delete(id: String) {
    clipboard::delete(id);
}

#[tauri::command]
pub fn clipboard_clear() {
    clipboard::clear();
}
