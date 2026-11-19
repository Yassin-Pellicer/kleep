use serde_json::Value;

use super::context::{ToolContext, ToolOutcome};
use super::{command, open_app, run_in_terminal, system_power, web_search};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    OpenApp,
    RunInTerminal,
    RunCommand,
    SystemPower,
    WebSearch,
}

impl ToolKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "open_app" => Some(ToolKind::OpenApp),
            "run_in_terminal" => Some(ToolKind::RunInTerminal),
            "run_command" => Some(ToolKind::RunCommand),
            "system_power" => Some(ToolKind::SystemPower),
            "web_search" => Some(ToolKind::WebSearch),
            _ => None,
        }
    }

    pub fn definition(self) -> Value {
        match self {
            ToolKind::OpenApp => open_app::definition(),
            ToolKind::RunInTerminal => run_in_terminal::definition(),
            ToolKind::RunCommand => command::definition(),
            ToolKind::SystemPower => system_power::definition(),
            ToolKind::WebSearch => web_search::definition(),
        }
    }

    pub async fn execute(self, arguments: &str, context: &mut ToolContext<'_>) -> ToolOutcome {
        match self {
            ToolKind::OpenApp => open_app::execute(arguments, context).await,
            ToolKind::RunInTerminal => run_in_terminal::execute(arguments, context).await,
            ToolKind::RunCommand => command::execute(arguments, context).await,
            ToolKind::SystemPower => system_power::execute(arguments, context).await,
            ToolKind::WebSearch => web_search::execute(arguments, context).await,
        }
    }
}
