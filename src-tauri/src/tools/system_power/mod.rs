use serde::Deserialize;
use serde_json::{Value, json};

use super::context::{ToolContext, ToolOutcome};

#[derive(Debug, Deserialize)]
struct SystemPowerArgs {
    action: String,
}

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "system_power",
            "description": "Power off, reboot, suspend, or log out of the user's Linux session. \
                            Use this whenever the user asks to shut down, turn off, restart, \
                            reboot, suspend, hibernate, or log out. The action runs immediately, \
                            so call the tool in this turn instead of announcing that you will do it.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["action"],
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["poweroff", "reboot", "suspend", "logout"],
                        "description": "The power action to perform."
                    }
                }
            }
        }
    })
}

pub(super) async fn execute(arguments: &str, _context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: SystemPowerArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for system_power."),
    };

    let action = args.action.trim().to_lowercase();

    let command = match action.as_str() {
        "poweroff" => "systemctl poweroff --no-block".to_string(),
        "reboot" => "systemctl reboot --no-block".to_string(),
        "suspend" => "systemctl suspend --no-block".to_string(),
        "logout" => logout_command(),
        _ => {
            return ToolOutcome::text(format!(
                "Unknown power action `{}`. Use poweroff, reboot, suspend, or logout.",
                args.action
            ));
        }
    };

    let result = super::command::run_shell(&command).await;

    if result.starts_with("exit status: success") {
        ToolOutcome::opened(format!("System action completed: {action}."))
    } else {
        ToolOutcome::text(result)
    }
}

fn logout_command() -> String {
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_default();
    let session = session.trim();

    if !session.is_empty()
        && session.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        format!("loginctl terminate-session {session}")
    } else {
        "loginctl terminate-user \"$USER\"".to_string()
    }
}
