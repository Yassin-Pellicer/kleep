use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, WindowClass,
};

use crate::structs::ClipboardEntry;

const MAX_ENTRIES: usize = 200;
const MAX_TEXT_BYTES: usize = 100_000;
const POLL_INTERVAL: Duration = Duration::from_millis(500);

const EV_KEY: u16 = 1;
const EV_SYN: u16 = 0;
const SYN_REPORT: u16 = 0;
const KEY_LEFTCTRL: u16 = 29;
const KEY_V: u16 = 47;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_INSERT: u16 = 110;
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x0000_5501;

static VIRTUAL_KEYBOARD: LazyLock<Mutex<Option<File>>> = LazyLock::new(|| Mutex::new(None));

#[repr(C)]
struct UinputInputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

// Matches `struct uinput_setup` from <linux/uinput.h>.
#[repr(C)]
struct UinputSetup {
    id: UinputInputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

struct ClipboardStore {
    entries: Vec<ClipboardEntry>,
    last_seen: Option<String>,
}

static STORE: LazyLock<Mutex<ClipboardStore>> = LazyLock::new(|| {
    Mutex::new(ClipboardStore {
        entries: load_history(),
        last_seen: None,
    })
});

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn history_path() -> PathBuf {
    data_home().join("kleep").join("clipboard.json")
}

fn load_history() -> Vec<ClipboardEntry> {
    let Ok(bytes) = fs::read(history_path()) else {
        return Vec::new();
    };

    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save_history(entries: &[ClipboardEntry]) {
    let path = history_path();
    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let Ok(data) = serde_json::to_vec(entries) else {
        return;
    };

    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, data).is_err() {
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
    }

    let _ = fs::rename(temporary, path);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn read_clipboard_x11() -> Option<String> {
    let (connection, screen_num) = x11rb::connect(None).ok()?;
    let screen = &connection.setup().roots[screen_num];
    let window = connection.generate_id().ok()?;

    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new()
                .event_mask(EventMask::PROPERTY_CHANGE)
                .override_redirect(1u32),
        )
        .ok()?;
    connection.map_window(window).ok()?;
    connection.flush().ok()?;

    let clipboard = connection
        .intern_atom(false, b"CLIPBOARD")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let utf8 = connection
        .intern_atom(false, b"UTF8_STRING")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let property = connection
        .intern_atom(false, b"KLEEP_CLIPBOARD")
        .ok()?
        .reply()
        .ok()?
        .atom;

    connection
        .convert_selection(window, clipboard, utf8, property, x11rb::CURRENT_TIME)
        .ok()?;
    connection.flush().ok()?;

    let deadline = std::time::Instant::now() + Duration::from_millis(250);
    let mut result = None;

    while std::time::Instant::now() < deadline {
        match connection.poll_for_event().ok()? {
            Some(Event::SelectionNotify(event)) => {
                if event.property != x11rb::NONE {
                    let reply = connection
                        .get_property(
                            true,
                            window,
                            property,
                            AtomEnum::ANY,
                            0,
                            MAX_TEXT_BYTES as u32 / 4,
                        )
                        .ok()?
                        .reply()
                        .ok()?;

                    if !reply.value.is_empty() {
                        result = Some(String::from_utf8_lossy(&reply.value).to_string());
                    }
                }
                break;
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }

    let _ = connection.destroy_window(window);
    result.filter(|text| !text.is_empty())
}

fn read_clipboard() -> Option<String> {
    // Use XWayland for polling. wl-paste's Wayland fallback has to briefly
    // focus a surface to read the clipboard, which causes visible flicker.
    read_clipboard_x11()
}

fn write_clipboard(text: &str) -> Result<(), String> {
    let mut command = Command::new("wl-copy");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command.spawn().or_else(|_| {
        let mut fallback = Command::new("xclip");
        fallback
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        fallback.spawn()
    });

    let mut child = child.map_err(|error| format!("could not copy to clipboard: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| error.to_string())?;
    }

    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("clipboard command failed".to_string());
    }

    Ok(())
}

pub fn copy_to_clipboard(id: &str) -> Result<(), String> {
    let entry = STORE
        .lock()
        .ok()
        .and_then(|store| store.entries.iter().find(|entry| entry.id == id).cloned())
        .ok_or_else(|| "clipboard entry not found".to_string())?;

    write_clipboard(&entry.text)
}

/// Creates a virtual keyboard for pasting into the previously focused app.
pub fn prepare_paste() -> Result<(), String> {
    let mut keyboard = VIRTUAL_KEYBOARD
        .lock()
        .map_err(|_| "virtual keyboard unavailable".to_string())?;

    if keyboard.is_some() {
        return Ok(());
    }

    let file = OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .map_err(|error| format!("direct paste unavailable: {error}"))?;

    let fd = file.as_raw_fd();

    unsafe {
        if libc::ioctl(fd, UI_SET_EVBIT, EV_KEY as libc::c_int) < 0 {
            return Err("could not configure virtual keyboard".to_string());
        }

        for key in [KEY_LEFTCTRL, KEY_V, KEY_LEFTSHIFT, KEY_INSERT] {
            if libc::ioctl(fd, UI_SET_KEYBIT, key as libc::c_int) < 0 {
                return Err("could not configure virtual keyboard keys".to_string());
            }
        }

        // The kernel rejects UI_DEV_CREATE with EINVAL unless the device
        // has been given an identity first.
        let mut setup = UinputSetup {
            id: UinputInputId {
                bustype: 0x03,
                vendor: 0x1234,
                product: 0x5678,
                version: 1,
            },
            name: [0; 80],
            ff_effects_max: 0,
        };
        let name = b"kleep-paste";
        setup.name[..name.len()].copy_from_slice(name);

        if libc::ioctl(fd, UI_DEV_SETUP, &setup as *const UinputSetup) < 0 {
            return Err("could not name virtual keyboard".to_string());
        }

        if libc::ioctl(fd, UI_DEV_CREATE) < 0 {
            return Err("could not create virtual keyboard".to_string());
        }
    }

    // Give the compositor a moment to notice the new input device.
    std::thread::sleep(Duration::from_millis(150));
    *keyboard = Some(file);
    Ok(())
}

fn write_event(file: &mut File, event_type: u16, code: u16, value: i32) -> Result<(), String> {
    let event = libc::input_event {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        type_: event_type,
        code,
        value,
    };

    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&event as *const libc::input_event).cast::<u8>(),
            std::mem::size_of::<libc::input_event>(),
        )
    };

    file.write_all(bytes).map_err(|error| error.to_string())
}

pub fn send_paste_shortcut() -> Result<(), String> {
    let mut keyboard = VIRTUAL_KEYBOARD
        .lock()
        .map_err(|_| "virtual keyboard unavailable".to_string())?;
    let file = keyboard
        .as_mut()
        .ok_or_else(|| "virtual keyboard is not ready".to_string())?;

    // Each key change needs its own SYN_REPORT, otherwise the compositor
    // receives Ctrl and V pressed and released in a single report.
    let press = |file: &mut File, code: u16, value: i32| -> Result<(), String> {
        write_event(file, EV_KEY, code, value)?;
        write_event(file, EV_SYN, SYN_REPORT, 0)?;
        file.flush().map_err(|error| error.to_string())
    };

    press(file, KEY_LEFTCTRL, 1)?;
    press(file, KEY_V, 1)?;
    press(file, KEY_V, 0)?;
    press(file, KEY_LEFTCTRL, 0)?;
    Ok(())
}

/// Starts the background clipboard watcher.
pub fn start() {
    // Load the saved history before the first command arrives.
    let _ = &*STORE;

    std::thread::spawn(|| {
        loop {
            std::thread::sleep(POLL_INTERVAL);

            let Some(text) = read_clipboard() else {
                continue;
            };

            if text.len() > MAX_TEXT_BYTES {
                continue;
            }

            let Ok(mut store) = STORE.lock() else {
                break;
            };

            if store.last_seen.as_deref() == Some(text.as_str()) {
                continue;
            }
            store.last_seen = Some(text.clone());

            // Re-copying an old entry moves it to the top instead of
            // creating a duplicate row.
            store.entries.retain(|entry| entry.text != text);
            store.entries.insert(
                0,
                ClipboardEntry {
                    id: hash_text(&text),
                    text,
                    timestamp: now_ms(),
                },
            );
            store.entries.truncate(MAX_ENTRIES);

            let snapshot = store.entries.clone();
            drop(store);
            save_history(&snapshot);
        }
    });
}

#[tauri::command]
pub fn clipboard_list() -> Vec<ClipboardEntry> {
    STORE
        .lock()
        .map(|store| store.entries.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn clipboard_copy(id: String) -> Result<(), String> {
    copy_to_clipboard(&id)
}

#[tauri::command]
pub fn clipboard_delete(id: String) {
    let Ok(mut store) = STORE.lock() else {
        return;
    };

    let before = store.entries.len();
    store.entries.retain(|entry| entry.id != id);
    if store.entries.len() == before {
        return;
    }

    let snapshot = store.entries.clone();
    drop(store);
    save_history(&snapshot);
}

#[tauri::command]
pub fn clipboard_clear() {
    let Ok(mut store) = STORE.lock() else {
        return;
    };

    store.entries.clear();
    let snapshot = store.entries.clone();
    drop(store);
    save_history(&snapshot);
}
