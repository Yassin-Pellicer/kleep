mod desktop;
mod icons;

use std::sync::{LazyLock, Mutex};

use crate::structs::AppEntry;

use self::desktop::DesktopApp;

const MAX_RESULTS: usize = 16;

static APPS: LazyLock<Mutex<Vec<DesktopApp>>> = LazyLock::new(|| Mutex::new(desktop::load_apps()));

/// Scans desktop entries off the main thread so the first search is instant.
pub fn preload() {
    LazyLock::force(&APPS);
}

#[tauri::command]
pub async fn search_apps(query: String) -> Vec<AppEntry> {
    tauri::async_runtime::spawn_blocking(move || search(&query))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn launch_app(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || launch(&id))
        .await
        .map_err(|error| error.to_string())?
}

fn search(query: &str) -> Vec<AppEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<(i32, DesktopApp)> = {
        let Ok(apps) = APPS.lock() else {
            return Vec::new();
        };

        apps.iter()
            .filter_map(|app| best_score(app, &query).map(|score| (score, app.clone())))
            .collect()
    };

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.to_lowercase().cmp(&right.1.name.to_lowercase()))
    });
    matches.truncate(MAX_RESULTS);

    matches
        .into_iter()
        .map(|(_, app)| AppEntry {
            id: app.id,
            name: app.name,
            description: app.description,
            icon: app.icon_name.as_deref().and_then(icons::data_uri),
        })
        .collect()
}

fn launch(id: &str) -> Result<(), String> {
    let app = {
        let apps = APPS
            .lock()
            .map_err(|_| "application list unavailable".to_string())?;

        apps.iter()
            .find(|app| app.id == id)
            .cloned()
            .ok_or_else(|| "application not found".to_string())?
    };

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))
}

/// Resolves an application name and launches the best match.
pub(crate) fn launch_by_name(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("No application name was provided.".to_string());
    }

    let normalized = query.to_lowercase();
    let app = {
        let apps = APPS
            .lock()
            .map_err(|_| "application list unavailable".to_string())?;

        let mut matches: Vec<(i32, &DesktopApp)> = apps
            .iter()
            .filter_map(|app| best_score(app, &normalized).map(|score| (score, app)))
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
            })?
    };

    app.spawn()
        .map_err(|error| format!("couldn't launch {}: {error}", app.name))?;

    Ok(app.name)
}

fn best_score(app: &DesktopApp, query: &str) -> Option<i32> {
    app.search_fields
        .iter()
        .filter_map(|field| match_score(field, query))
        .max()
}

fn match_score(haystack: &str, needle: &str) -> Option<i32> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Some(0);
    }

    if haystack == needle {
        return Some(10_000);
    }

    // Match query words against the starts of real words in the haystack.
    // No fuzzy letter matching: "cat" must match "cat", "catalog" or
    // "categories", never "Characters".
    let query_words: Vec<&str> = split_words(needle).collect();
    if query_words.is_empty() {
        return Some(0);
    }

    let haystack_words: Vec<&str> = split_words(haystack).collect();
    let mut total = 0;

    for query_word in query_words {
        let mut best: Option<i32> = None;

        for word in &haystack_words {
            let score = if *word == query_word {
                1_000
            } else if let Some(rest) = word.strip_prefix(query_word) {
                600 - rest.chars().count() as i32
            } else {
                continue;
            };

            best = Some(best.map_or(score, |current| current.max(score)));
        }

        total += best?;
    }

    Some(total)
}

fn split_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

#[cfg(test)]
mod tests {
    use super::match_score;

    #[test]
    fn matches_words_not_letters() {
        assert!(match_score("firefox", "fire").is_some());
        assert!(match_score("firefox", "fox").is_none());
        assert!(match_score("firefox", "ffx").is_none());
        assert!(match_score("characters", "cat").is_none());
        assert!(match_score("categories", "cat").is_some());
        assert!(match_score("visual studio code", "visual code").is_some());
        assert!(match_score("cachyos hello", "hello").is_some());
        assert!(match_score("firefox", "zzz").is_none());
    }
}
