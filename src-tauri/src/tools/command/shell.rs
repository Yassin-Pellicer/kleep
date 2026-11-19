use std::process::Stdio;
use std::time::Duration;

use super::output;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) async fn run(command: &str) -> String {
    let mut process = tokio::process::Command::new("sh");
    process
        .arg("-lc")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    match tokio::time::timeout(COMMAND_TIMEOUT, process.output()).await {
        Err(_) => format!(
            "Command timed out after {} seconds. It may still be running in the background.",
            COMMAND_TIMEOUT.as_secs()
        ),
        Ok(Err(error)) => format!("Could not run the command: {error}"),
        Ok(Ok(output)) => output::describe(&output),
    }
}
