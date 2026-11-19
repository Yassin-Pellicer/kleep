use std::env;
use std::process::{Command, Stdio};

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
