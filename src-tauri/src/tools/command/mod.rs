mod output;
mod programs;
mod shell;

use serde::Deserialize;
use serde_json::{Value, json};

use super::context::{ToolContext, ToolOutcome};

#[derive(Debug, Deserialize)]
struct RunCommandArgs {
    command: String,
}

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "run_command",
            "description": "Run a non-interactive shell command on the user's Linux machine \
                            and return its exit status, stdout and stderr. Use it to perform \
                            system actions such as powering off (`systemctl poweroff`), \
                            rebooting (`systemctl reboot`), suspending (`systemctl suspend`), \
                            logging out (`loginctl terminate-session \
                            \"$XDG_SESSION_ID\"`), changing settings, or launching GUI \
                            programs. Also use it to inspect the system with read-only \
                            commands such as `ps aux`, `nvidia-smi`, `free -h`, `df -h`, \
                            `systemctl status`, `systemctl --failed`, `ip addr`, or \
                            `journalctl -n 50 --no-pager`; the output is returned to you, so \
                            use it to report real data. Commands run through `sh -lc` with a \
                            15 second timeout: add `--no-block` to power actions, and launch \
                            GUI programs in the background with output redirected (for \
                            example `nohup google-chrome-stable >/dev/null 2>&1 &`) so they \
                            return immediately. Interactive monitors never return on their \
                            own: use a bounded form such as `nvidia-smi pmon -c 1` or \
                            `top -b -n 1`, not bare `pmon`, `top`, `watch`, or `btop`. The \
                            result also includes the list of \
                            programs currently running for the user, so use that instead of \
                            making a separate call just to enumerate them.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run."
                    }
                }
            }
        }
    })
}

pub(super) async fn execute(arguments: &str, _context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: RunCommandArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for run_command."),
    };

    let command = args.command.trim();
    if command.is_empty() {
        return ToolOutcome::text("The command was empty.");
    }

    let result = shell::run(command).await;

    match programs::context() {
        Some(context) => ToolOutcome::text(format!("{result}\n\n{context}")),
        None => ToolOutcome::text(result),
    }
}
