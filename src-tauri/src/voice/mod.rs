use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{Value, json};

use crate::structs::{VoiceRecordingStatus, VoiceTranscript};

const PORT: u16 = 8765;
const PROTOCOL_VERSION: u64 = 2;
const START_TIMEOUT: Duration = Duration::from_secs(120);

static SERVER: LazyLock<Mutex<Option<Child>>> = LazyLock::new(|| Mutex::new(None));
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(2))
        .build()
        .expect("failed to build voice HTTP client")
});

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

fn server_ready() -> bool {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}

fn server_version() -> Option<u64> {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let body = response.split("\r\n\r\n").nth(1)?;
    let value: Value = serde_json::from_str(body).ok()?;
    value.get("version").and_then(Value::as_u64)
}

fn ensure_server() -> Result<(), String> {
    if server_ready() {
        if server_version() == Some(PROTOCOL_VERSION) {
            return Ok(());
        }

        // Replace a service left over from an older build.
        request_shutdown();
        for _ in 0..50 {
            if !server_ready() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        if server_ready() {
            return Err("the old voice service did not stop".to_string());
        }
    }

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

    let mut child = command
        .spawn()
        .map_err(|error| format!("could not start voice service: {error}"))?;
    let deadline = Instant::now() + START_TIMEOUT;

    while Instant::now() < deadline {
        if server_ready() {
            if let Ok(mut slot) = SERVER.lock() {
                *slot = Some(child);
            }
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

/// Warms up the local voice service in the background.
pub fn preload() {
    std::thread::spawn(|| {
        let _ = ensure_server();
    });
}

/// Kills the child process we started, if any.
pub fn shutdown() {
    if let Ok(mut slot) = SERVER.lock()
        && let Some(mut child) = slot.take()
    {
        request_shutdown();

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

fn request_shutdown() {
    let address: SocketAddr = ([127, 0, 0, 1], PORT).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return;
    };

    let _ = stream.write_all(
        b"POST /shutdown HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );

    let mut buffer = [0u8; 64];
    let _ = stream.read(&mut buffer);
}

async fn ensure_ready() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(ensure_server)
        .await
        .map_err(|error| error.to_string())?
}

async fn post(path: &str, body: Value) -> Result<Value, String> {
    let response = CLIENT
        .post(format!("http://127.0.0.1:{PORT}{path}"))
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("voice service request failed: {error}"))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or(text);
        return Err(message);
    }

    if text.is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn voice_start_recording() -> Result<(), String> {
    ensure_ready().await?;
    post("/start", json!({})).await?;
    Ok(())
}

#[tauri::command]
pub async fn voice_recording_status() -> Result<VoiceRecordingStatus, String> {
    if !server_ready() {
        return Ok(VoiceRecordingStatus {
            recording: false,
            auto_stop: false,
            speech: false,
            silence: 0.0,
            level: 0.0,
        });
    }

    let value = post("/status", json!({})).await?;
    serde_json::from_value(value)
        .map_err(|error| format!("invalid voice recording status: {error}"))
}

#[tauri::command]
pub async fn voice_stop_recording() -> Result<VoiceTranscript, String> {
    ensure_ready().await?;
    let value = post("/stop", json!({})).await?;
    serde_json::from_value(value).map_err(|error| format!("invalid voice transcript: {error}"))
}

#[tauri::command]
pub async fn voice_cancel_recording() -> Result<(), String> {
    if !server_ready() {
        return Ok(());
    }
    post("/cancel", json!({})).await?;
    Ok(())
}

#[tauri::command]
pub async fn voice_speak(text: String, language: String) -> Result<(), String> {
    ensure_ready().await?;
    post("/speak", json!({ "text": text, "language": language })).await?;
    Ok(())
}

#[tauri::command]
pub async fn voice_stop_speaking() -> Result<(), String> {
    if !server_ready() {
        return Ok(());
    }
    post("/stop_speaking", json!({})).await?;
    Ok(())
}
