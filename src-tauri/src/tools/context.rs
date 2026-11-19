use std::collections::HashSet;

use crate::models::ChatSource;

/// State shared by every tool invocation during one assistant turn.
pub struct ToolContext<'a> {
    pub search_api_key: Option<&'a str>,
    pub local: bool,
    pub sources: Vec<ChatSource>,
    pub seen_urls: HashSet<String>,
}

impl<'a> ToolContext<'a> {
    pub fn new(search_api_key: Option<&'a str>, local: bool) -> Self {
        Self {
            search_api_key,
            local,
            sources: Vec::new(),
            seen_urls: HashSet::new(),
        }
    }
}

pub struct ToolOutcome {
    pub content: String,
    /// True when the tool performed a complete user-facing action.
    pub opened: bool,
}

impl ToolOutcome {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            opened: false,
        }
    }

    pub fn opened(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            opened: true,
        }
    }
}
