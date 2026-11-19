use std::io::Write;
use std::process::{Command, Stdio};

pub(super) fn write_clipboard(text: &str) -> Result<(), String> {
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
