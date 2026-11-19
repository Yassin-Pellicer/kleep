use super::catalog;
use super::search;

pub fn launch(id: &str) -> Result<(), String> {
    let app = catalog::with_apps(|apps| apps.iter().find(|app| app.id == id).cloned())
        .ok_or_else(|| "application list unavailable".to_string())?
        .ok_or_else(|| "application not found".to_string())?;

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))
}

/// Resolves an application name and launches the best match.
///
/// `request` is the user's original message. The resolved app must share a
/// word or alias with it, so the model cannot open an unrelated app just
/// because it appears in the installed-app context.
pub fn launch_by_name(query: &str, request: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("No application name was provided.".to_string());
    }

    let app = search::resolve(query).ok_or_else(|| no_match_message(query))?;

    if !search::matches_request(&app, request) {
        return Err(format!(
            "No installed application matches \"{query}\" for this request. Tell the user it is \
             not installed instead of opening a different app."
        ));
    }

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))?;

    Ok(app.name)
}

fn no_match_message(query: &str) -> String {
    let first_word = query.split_whitespace().next().unwrap_or("");

    let suggestions = catalog::with_apps(|apps| {
        apps.iter()
            .filter(|app| app.name.to_lowercase().contains(first_word))
            .take(3)
            .map(|app| app.name.clone())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    if suggestions.is_empty() {
        format!("No desktop application matches \"{query}\".")
    } else {
        format!(
            "No desktop application matches \"{query}\". Similar apps: {}. Retry open_app with one of these names if it is the right app.",
            suggestions.join(", ")
        )
    }
}
