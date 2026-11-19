mod health;
mod process;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::models::VoiceSetupStatus;

use super::PROTOCOL_VERSION;

const START_TIMEOUT: Duration = Duration::from_secs(120);
const SETUP_MESSAGE: &str =
    "Setting up voice for the first time. Downloading the speech models can take several minutes.";

/// Serializes the one-time setup so concurrent voice commands wait for it.
static SETUP_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static SETUP_RUNNING: AtomicBool = AtomicBool::new(false);
static SETUP_ERROR: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

/// Warms up the local voice service in the background.
///
/// The one-time dependency/model setup is deliberately deferred until the
/// user first asks for voice, so a fresh install does not download gigabytes
/// of models without being asked.
pub fn preload() {
    std::thread::spawn(|| {
        let Some(directory) = process::voice_dir() else {
            return;
        };

        if process::is_provisioned(&directory) {
            let _ = ensure_server();
        }
    });
}

/// Reports the state of the one-time voice setup.
pub fn setup_status() -> VoiceSetupStatus {
    let ready = process::voice_dir().is_some_and(|directory| process::is_provisioned(&directory));

    if ready {
        return VoiceSetupStatus {
            state: "ready",
            message: String::new(),
        };
    }

    if SETUP_RUNNING.load(Ordering::SeqCst) {
        return VoiceSetupStatus {
            state: "preparing",
            message: SETUP_MESSAGE.to_string(),
        };
    }

    if let Some(error) = SETUP_ERROR.lock().ok().and_then(|error| error.clone()) {
        return VoiceSetupStatus {
            state: "failed",
            message: error,
        };
    }

    VoiceSetupStatus {
        state: "idle",
        message: String::new(),
    }
}

/// Kills the child process we started, if any.
pub fn shutdown() {
    if let Some(mut child) = process::take() {
        health::request_shutdown();

        for _ in 0..25 {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let _ = child.kill();
        let _ = child.wait();
    }
}

pub(super) fn is_running() -> bool {
    health::server_ready()
}

pub(super) fn ensure_server() -> Result<(), String> {
    if health::server_ready() {
        if health::server_version() == Some(PROTOCOL_VERSION) {
            return Ok(());
        }

        // Replace a service left over from an older build.
        health::request_shutdown();
        for _ in 0..50 {
            if !health::server_ready() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        if health::server_ready() {
            return Err("the old voice service did not stop".to_string());
        }
    }

    let directory =
        process::voice_dir().ok_or_else(|| "voice service files not found".to_string())?;
    provision(&directory)?;
    let mut child = process::spawn(&directory)?;
    let deadline = Instant::now() + START_TIMEOUT;

    while Instant::now() < deadline {
        if health::server_ready() {
            process::store(child);
            return Ok(());
        }

        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "voice service exited ({status}). Log: {}/voice.log",
                directory.display()
            ));
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(format!(
        "voice service did not start in time. Log: {}/voice.log",
        directory.display()
    ))
}

/// Runs the bundled `setup.sh` the first time voice is used.
fn provision(directory: &std::path::Path) -> Result<(), String> {
    if process::is_provisioned(directory) {
        return Ok(());
    }

    // Another command may already be running the setup; wait for it.
    let _guard = SETUP_LOCK
        .lock()
        .map_err(|_| "voice setup lock is poisoned".to_string())?;

    if process::is_provisioned(directory) {
        return Ok(());
    }

    if auto_setup_disabled() {
        return Err(format!(
            "voice is not set up. Run `{}` once, then try again. Log: {}/voice.log",
            directory.join("setup.sh").display(),
            directory.display()
        ));
    }

    if let Ok(mut error) = SETUP_ERROR.lock() {
        *error = None;
    }
    SETUP_RUNNING.store(true, Ordering::SeqCst);

    let result = process::run_setup(directory);

    SETUP_RUNNING.store(false, Ordering::SeqCst);

    if let Err(error) = &result
        && let Ok(mut slot) = SETUP_ERROR.lock()
    {
        *slot = Some(error.clone());
    }

    result
}

fn auto_setup_disabled() -> bool {
    std::env::var("KLEEP_VOICE_AUTO_SETUP").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}
