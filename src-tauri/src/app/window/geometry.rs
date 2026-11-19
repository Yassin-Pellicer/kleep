use tauri::{LogicalSize, WebviewWindow};

pub const MAIN_WINDOW: &str = "main";

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT_RATIO: f64 = 0.5;

/// Size the window to half the monitor height. A compositor that centers new
/// windows will then place its top edge at 25% of the screen; the bar itself is
/// drawn at the top of the window with CSS.
pub fn fit_to_screen(window: &WebviewWindow) {
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
