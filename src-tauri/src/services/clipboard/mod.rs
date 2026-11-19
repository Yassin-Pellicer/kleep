mod focus;
mod history;
mod paste;
mod writer;
mod x11;

use std::time::Duration;

use crate::models::ClipboardEntry;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub use focus::{focus_hide_suppressed, suppress_focus_hide};
pub use paste::{prepare_paste, send_paste_shortcut};

pub fn start() {
    history::init();

    std::thread::spawn(|| {
        loop {
            std::thread::sleep(POLL_INTERVAL);

            let Some(text) = x11::read_clipboard() else {
                continue;
            };

            if text.len() > x11::MAX_TEXT_BYTES {
                continue;
            }

            history::record(text);
        }
    });
}

pub fn list() -> Vec<ClipboardEntry> {
    history::list()
}

pub fn copy_to_clipboard(id: &str) -> Result<(), String> {
    let entry = history::find(id).ok_or_else(|| "clipboard entry not found".to_string())?;
    writer::write_clipboard(&entry.text)
}

pub fn delete(id: String) {
    history::delete(&id);
}

pub fn clear() {
    history::clear();
}
