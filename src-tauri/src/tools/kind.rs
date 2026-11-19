use serde_json::Value;

use super::context::{ToolContext, ToolOutcome};
use super::{command, open_app, web_search};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    OpenApp,
    RunCommand,
    WebSearch,
}

impl ToolKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "open_app" => Some(ToolKind::OpenApp),
            "run_command" => Some(ToolKind::RunCommand),
            "web_search" => Some(ToolKind::WebSearch),
            _ => None,
        }
    }

    pub fn definition(self) -> Value {
        match self {
            ToolKind::OpenApp => open_app::definition(),
            ToolKind::RunCommand => command::definition(),
            ToolKind::WebSearch => web_search::definition(),
        }
    }

    pub async fn execute(self, arguments: &str, context: &mut ToolContext<'_>) -> ToolOutcome {
        match self {
            ToolKind::OpenApp => open_app::execute(arguments, context).await,
            ToolKind::RunCommand => command::execute(arguments, context).await,
            ToolKind::WebSearch => web_search::execute(arguments, context).await,
        }
    }
}
