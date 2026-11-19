use std::process::Output;

use crate::services::support::text::truncate_chars;

const MAX_COMMAND_OUTPUT: usize = 4_000;

pub(super) fn describe(output: &Output) -> String {
    let mut parts = vec![match output.status.code() {
        Some(0) => "exit status: success".to_string(),
        Some(code) => format!("exit status: {code}"),
        None => "exit status: terminated by a signal".to_string(),
    }];

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        parts.push(format!("stdout:\n{}", truncate(stdout)));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        parts.push(format!("stderr:\n{}", truncate(stderr)));
    }

    parts.join("\n")
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_COMMAND_OUTPUT {
        return text.to_string();
    }

    format!(
        "{}… [output truncated]",
        truncate_chars(text, MAX_COMMAND_OUTPUT)
    )
}
