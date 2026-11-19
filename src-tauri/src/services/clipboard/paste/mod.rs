mod device;
mod events;

use std::fs::File;
use std::sync::{LazyLock, Mutex};

static VIRTUAL_KEYBOARD: LazyLock<Mutex<Option<File>>> = LazyLock::new(|| Mutex::new(None));

/// Creates the virtual keyboard ahead of the first paste.
pub fn prepare_paste() -> Result<(), String> {
    let mut keyboard = VIRTUAL_KEYBOARD
        .lock()
        .map_err(|_| "virtual keyboard unavailable".to_string())?;

    if keyboard.is_some() {
        return Ok(());
    }

    *keyboard = Some(device::create()?);
    Ok(())
}

pub fn send_paste_shortcut() -> Result<(), String> {
    let mut keyboard = VIRTUAL_KEYBOARD
        .lock()
        .map_err(|_| "virtual keyboard unavailable".to_string())?;
    let file = keyboard
        .as_mut()
        .ok_or_else(|| "virtual keyboard is not ready".to_string())?;

    events::send_paste_shortcut(file)
}
