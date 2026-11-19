use super::catalog;
use super::desktop::DesktopApp;

pub fn launch(id: &str) -> Result<(), String> {
    let app = catalog::with_apps(|apps| apps.iter().find(|app| app.id == id).cloned())
        .ok_or_else(|| "application list unavailable".to_string())?
        .ok_or_else(|| "application not found".to_string())?;

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))
}

/// Resolves an application name and launches the best match.
pub fn launch_by_name(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("No application name was provided.".to_string());
    }

    let normalized = query.to_lowercase();
    let app = catalog::with_apps(|apps| -> Result<DesktopApp, String> {
        let mut matches: Vec<(i32, &DesktopApp)> = apps
            .iter()
            .filter_map(|app| super::search::best_score(app, &normalized).map(|score| (score, app)))
            .collect();

        matches.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.name.to_lowercase().cmp(&right.1.name.to_lowercase()))
        });

        matches
            .first()
            .map(|(_, app)| (*app).clone())
            .ok_or_else(|| {
                let suggestions = apps
                    .iter()
                    .filter(|app| {
                        app.name
                            .to_lowercase()
                            .contains(normalized.split_whitespace().next().unwrap_or(""))
                    })
                    .take(3)
                    .map(|app| app.name.as_str())
                    .collect::<Vec<_>>();

                if suggestions.is_empty() {
                    format!("No desktop application matches \"{query}\".")
                } else {
                    format!(
                        "No desktop application matches \"{query}\". Similar apps: {}.",
                        suggestions.join(", ")
                    )
                }
            })
    })
    .ok_or_else(|| "application list unavailable".to_string())??;

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))?;

    Ok(app.name)
}
