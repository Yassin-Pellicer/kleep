use tauri::AppHandle;

use crate::app::window::{hide_with_fade, main_window};

#[tauri::command]
pub fn hide_palette(app: AppHandle) {
    if let Some(window) = main_window(&app) {
        hide_with_fade(window);
    }
}
