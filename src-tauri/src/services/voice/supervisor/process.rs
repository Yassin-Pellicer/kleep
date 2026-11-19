use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};

use super::super::PORT;

static SERVER: LazyLock<Mutex<Option<Child>>> = LazyLock::new(|| Mutex::new(None));

fn voice_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("KLEEP_VOICE_DIR").map(PathBuf::from),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../voice")),
        std::env::current_dir()
            .ok()
            .map(|directory| directory.join("voice")),
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|directory| directory.join("voice"))),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|directory| directory.join("server.py").is_file())
}

pub(super) fn spawn() -> Result<Child, String> {
    let directory = voice_dir().ok_or_else(|| "voice service files not found".to_string())?;
    let venv_python = directory.join(".venv/bin/python");
    let python = if venv_python.is_file() {
        venv_python
    } else {
        PathBuf::from("python3")
    };

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("voice.log"))
        .ok();

    let mut command = Command::new(python);
    command
        .arg("server.py")
        .current_dir(&directory)
        .env("KLEEP_VOICE_PORT", PORT.to_string())
        .stdin(Stdio::null());

    match log {
        Some(file) => {
            let stderr = file.try_clone().map_err(|error| error.to_string())?;
            command.stdout(file).stderr(stderr);
        }
        None => {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }

    command
        .spawn()
        .map_err(|error| format!("could not start voice service: {error}"))
}

pub(super) fn store(child: Child) {
    if let Ok(mut slot) = SERVER.lock() {
        *slot = Some(child);
    }
}

pub(super) fn take() -> Option<Child> {
    SERVER.lock().ok().and_then(|mut slot| slot.take())
}
