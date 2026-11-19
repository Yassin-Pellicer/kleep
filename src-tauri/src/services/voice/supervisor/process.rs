use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use super::super::PORT;
use super::super::embedded;

static SERVER: LazyLock<Mutex<Option<Child>>> = LazyLock::new(|| Mutex::new(None));

/// Upper bound for the one-time dependency/model download.
const SETUP_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Resolves the writable voice directory, materializing the embedded service
/// files for installed builds.
pub(super) fn voice_dir() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("KLEEP_VOICE_DIR").map(PathBuf::from)
        && directory.join("server.py").is_file()
    {
        return Some(directory);
    }

    // Developers keep using the checked-out service with its existing venv.
    #[cfg(debug_assertions)]
    {
        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../voice");
        if development.join("server.py").is_file() {
            return Some(development);
        }
    }

    if let Some(directory) = embedded::ensure_install() {
        return Some(directory);
    }

    let candidates = [
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

/// True once the one-time setup finished successfully.
///
/// `setup.sh` writes `.setup-complete` as its very last step, so a partial
/// install (or a setup still in progress) always triggers a retry.
pub(super) fn is_provisioned(directory: &Path) -> bool {
    directory.join(".venv/bin/python").is_file() && directory.join(".setup-complete").is_file()
}

/// Runs the bundled `setup.sh`, appending its output to `voice.log`.
pub(super) fn run_setup(directory: &Path) -> Result<(), String> {
    let log_path = directory.join("voice.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let mut command = Command::new("bash");
    command
        .arg("setup.sh")
        .current_dir(directory)
        .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
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

    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not run {}: {error}",
            directory.join("setup.sh").display()
        )
    })?;

    let deadline = Instant::now() + SETUP_TIMEOUT;

    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "voice setup failed ({status}). See {}",
                    log_path.display()
                ));
            }
            Ok(None) => {}
            Err(error) => return Err(format!("could not wait for voice setup: {error}")),
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("voice setup timed out. See {}", log_path.display()));
        }

        std::thread::sleep(Duration::from_millis(250));
    }
}

pub(super) fn spawn(directory: &Path) -> Result<Child, String> {
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
        .current_dir(directory)
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
