use std::env;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::paths::command_exists;

pub fn terminal_command() -> Option<Vec<String>> {
    if let Some(value) = env::var_os("TERMINAL") {
        let value = value.to_string_lossy();
        let parts: Vec<String> = value.split_whitespace().map(str::to_string).collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }

    const CANDIDATES: &[&str] = &[
        "x-terminal-emulator",
        "konsole",
        "alacritty",
        "foot",
        "xterm",
        "gnome-terminal",
    ];

    CANDIDATES
        .iter()
        .find(|candidate| command_exists(candidate))
        .map(|candidate| vec![candidate.to_string()])
}

pub fn spawn_detached(mut command: Command) -> std::io::Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn()?;

    // Reap the child without blocking the caller.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

/// Opens a terminal window and runs `command` inside it.
pub fn open_terminal(command: &str) -> Result<(), String> {
    let terminal =
        terminal_command().ok_or_else(|| "no terminal emulator was found".to_string())?;

    // Keep the window open after the command exits so its output stays
    // readable, then let the user close it with Enter.
    let script = format!(
        "{command}\nstatus=$?\nprintf '\\n[kleep] command exited with status %s. Press Enter to close.\\n' \"$status\"\nread _\n"
    );

    let mut process = Command::new(&terminal[0]);
    let program = terminal[0].rsplit('/').next().unwrap_or(&terminal[0]);
    process.args(&terminal[1..]);
    if program == "gnome-terminal" {
        process.arg("--");
    } else {
        process.arg("-e");
    }
    process
        .arg("sh")
        .arg("-lc")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = process
        .spawn()
        .map_err(|error| format!("couldn't open a terminal: {error}"))?;

    // A terminal that fails on startup exits before showing a window. Some
    // terminals hand the request to an existing server and exit successfully,
    // so only a failing status is treated as an error.
    std::thread::sleep(Duration::from_millis(250));
    if let Ok(Some(status)) = child.try_wait() {
        if !status.success() {
            return Err(format!("the terminal exited immediately ({status})"));
        }
        return Ok(());
    }

    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}
