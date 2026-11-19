use serde::Deserialize;
use serde_json::{Value, json};

use super::context::{ToolContext, ToolOutcome};

#[derive(Debug, Deserialize)]
struct RunInTerminalArgs {
    command: String,
}

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "run_in_terminal",
            "description": "Open a new terminal window and run a command in it. Use this for \
                            interactive or full-screen programs that need a terminal, such as \
                            btop, htop, top, less, vim, nano, tmux, a shell, or a REPL, and \
                            whenever the user asks to run something in a terminal. Never start \
                            these programs with run_command or in the background: they would \
                            run without a visible terminal.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run in the new terminal, such as \"btop\"."
                    }
                }
            }
        }
    })
}

pub(super) async fn execute(arguments: &str, _context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: RunInTerminalArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for run_in_terminal."),
    };

    let command = args.command.trim().to_string();
    if command.is_empty() {
        return ToolOutcome::text("The command was empty.");
    }

    let launched = command.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        crate::services::apps::open_terminal(&launched)
    })
    .await
    {
        Ok(Ok(())) => ToolOutcome::opened(format!("Opened a terminal running `{command}`.")),
        Ok(Err(error)) => ToolOutcome::text(error),
        Err(error) => ToolOutcome::text(format!("Couldn't open a terminal: {error}")),
    }
}
