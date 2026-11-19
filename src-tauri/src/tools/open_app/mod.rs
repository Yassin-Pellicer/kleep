mod args;
mod browser;

use serde_json::{Value, json};

use super::context::{ToolContext, ToolOutcome};

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "open_app",
            "description": "Open a desktop application by name, or open a website/link in \
                            the default browser. Use the url field for websites and online \
                            services such as YouTube, Gmail, GitHub, or any link; for \
                            example opening YouTube is { \"url\": \"https://youtube.com\" }. \
                            Use the app field only for installed desktop applications. \
                            Do not pass a website name in the app field.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": [],
                "properties": {
                    "app": {
                        "type": "string",
                        "description": "An installed desktop application, such as \"spotify\" or \"chrome\"."
                    },
                    "url": {
                        "type": "string",
                        "description": "A full http(s) URL to open in the default browser, such as \"https://youtube.com\"."
                    }
                }
            }
        }
    })
}

pub(super) async fn execute(arguments: &str, _context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: args::OpenAppArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for open_app."),
    };

    if let Some(url) = args
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        return open_url(url).await;
    }

    let app = match args.app.as_deref().map(str::trim) {
        Some(app) if !app.is_empty() => app.to_string(),
        _ => return ToolOutcome::text("Provide either an app name or a URL."),
    };

    match tauri::async_runtime::spawn_blocking(move || crate::services::apps::launch_by_name(&app))
        .await
    {
        Ok(Ok(name)) => ToolOutcome::opened(format!("Opened {name}.")),
        Ok(Err(error)) => ToolOutcome::text(error),
        Err(error) => ToolOutcome::text(format!("Could not open the app: {error}")),
    }
}

async fn open_url(url: &str) -> ToolOutcome {
    let url = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return ToolOutcome::text("Only http and https links can be opened.");
    }

    let target = url.clone();
    match tauri::async_runtime::spawn_blocking(move || browser::open_in_browser(&target)).await {
        Ok(Ok(())) => ToolOutcome::opened(format!("Opened {url} in the default browser.")),
        Ok(Err(error)) => ToolOutcome::text(error),
        Err(error) => ToolOutcome::text(format!("Could not open the link: {error}")),
    }
}
