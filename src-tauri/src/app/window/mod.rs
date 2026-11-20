mod focus;
mod geometry;
mod state;
mod visibility;

use tauri::{AppHandle, Manager, WebviewWindow};

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(geometry::MAIN_WINDOW)
}

pub use focus::handle_focus_lost;
pub use geometry::fit_to_screen;
pub use state::request_paste;
pub use visibility::{
    hide_with_fade, show_palette_listening_on_launch, show_palette_on_launch, toggle_palette,
};
