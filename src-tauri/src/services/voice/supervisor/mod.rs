mod health;
mod process;

use std::time::{Duration, Instant};

use super::PROTOCOL_VERSION;

const START_TIMEOUT: Duration = Duration::from_secs(120);

/// Warms up the local voice service in the background.
pub fn preload() {
    std::thread::spawn(|| {
        let _ = ensure_server();
    });
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

    let mut child = process::spawn()?;
    let deadline = Instant::now() + START_TIMEOUT;

    while Instant::now() < deadline {
        if health::server_ready() {
            process::store(child);
            return Ok(());
        }

        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "voice service exited ({status}); see voice/voice.log"
            ));
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err("voice service did not start in time; see voice/voice.log".to_string())
}
