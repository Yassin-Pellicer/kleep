mod command;
mod context;
mod kind;
mod open_app;
mod web_search;

use serde_json::Value;

pub use context::{ToolContext, ToolOutcome};
pub use kind::ToolKind;

/// Tool definitions offered to the model for this turn.
pub fn definitions(web_search: bool) -> Vec<Value> {
    let mut kinds = vec![ToolKind::OpenApp, ToolKind::RunCommand];
    if web_search {
        kinds.push(ToolKind::WebSearch);
    }
    kinds.into_iter().map(ToolKind::definition).collect()
}

/// Central dispatch for every assistant tool call.
pub async fn execute(name: &str, arguments: &str, context: &mut ToolContext<'_>) -> ToolOutcome {
    match ToolKind::from_name(name) {
        Some(tool) => tool.execute(arguments, context).await,
        None => ToolOutcome::text(format!("Unknown tool `{name}`.")),
    }
}
