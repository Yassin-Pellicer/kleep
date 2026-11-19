use crate::models::AppEntry;

use super::catalog;
use super::desktop::DesktopApp;
use super::icons;

const MAX_RESULTS: usize = 16;

/// A match must be at least a full word match (or a close prefix) before the
/// assistant acts on it without asking the user for clarification.
const CONFIDENT_SCORE: i32 = 550;

pub fn search(query: &str) -> Vec<AppEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = ranked(&query);
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

fn ranked(query: &str) -> Vec<(i32, DesktopApp)> {
    let mut matches: Vec<(i32, DesktopApp)> = match catalog::with_apps(|apps| {
        apps.iter()
            .filter_map(|app| best_score(app, query).map(|score| (score, app.clone())))
            .collect()
    }) {
        Some(matches) => matches,
        None => return Vec::new(),
    };

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.len().cmp(&right.1.name.len()))
            .then_with(|| left.1.name.to_lowercase().cmp(&right.1.name.to_lowercase()))
    });
    matches
}

/// Resolves an application name to the single best installed app, but only
/// when the match is strong enough to act on without asking the user.
pub(super) fn resolve(query: &str) -> Option<DesktopApp> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }

    best_match(&query)
        .filter(|(score, _)| *score >= CONFIDENT_SCORE)
        .map(|(_, app)| app)
}

/// True when the resolved app shares a meaningful word with the user's
/// request, so a suggestion from the model cannot open an unrelated app.
pub(super) fn matches_request(app: &DesktopApp, request: &str) -> bool {
    let request = request.trim().to_lowercase();
    if request.is_empty() {
        return true;
    }

    let request_words: Vec<&str> = split_words(&request).collect();
    if request_words.is_empty() {
        return true;
    }

    let lower_name = app.name.to_lowercase();
    let mut app_words: Vec<&str> = split_words(&lower_name).collect();
    app_words.extend(
        app.search_fields
            .iter()
            .flat_map(|field| split_words(field)),
    );

    request_words.iter().any(|request_word| {
        let request_word: &str = request_word;
        request_word.chars().count() >= 3
            && app_words.iter().any(|app_word| {
                let app_word: &str = app_word;
                app_word.chars().count() >= 3
                    && (app_word == request_word
                        || app_word.starts_with(request_word)
                        || request_word.starts_with(app_word)
                        // Accept concatenations such as "vscode" → "code".
                        || (app_word.chars().count() >= 4 && request_word.contains(app_word)))
            })
    })
}

fn best_match(query: &str) -> Option<(i32, DesktopApp)> {
    catalog::with_apps(|apps| {
        apps.iter()
            .filter_map(|app| best_score(app, query).map(|score| (score, app.clone())))
            .max_by(|left, right| {
                left.0.cmp(&right.0).then_with(|| {
                    // On a tie prefer the shorter, more exact app name.
                    right.1.name.len().cmp(&left.1.name.len())
                })
            })
    })
    .flatten()
}

const NAME_EXACT_SCORE: i32 = 3_000;
const NAME_PREFIX_SCORE: i32 = 1_500;
const FIELD_EXACT_SCORE: i32 = 1_000;
const FIELD_PREFIX_SCORE: i32 = 600;

pub(super) fn best_score(app: &DesktopApp, query: &str) -> Option<i32> {
    let query = query.trim();
    if query.is_empty() {
        return Some(0);
    }

    let query_words: Vec<&str> = split_words(query).collect();
    if query_words.is_empty() {
        return None;
    }

    // Words in the application's own name count for more than keywords or
    // categories, so "chrome" prefers Google Chrome over another entry that
    // merely mentions it, "code" prefers Visual Studio Code over Kate, and
    // "settings" prefers Settings over dconf Editor.
    let lower_name = app.name.to_lowercase();
    let name_words: Vec<&str> = split_words(&lower_name).collect();
    let other_words: Vec<&str> = app
        .search_fields
        .iter()
        .flat_map(|field| split_words(field))
        .collect();

    let name_score = words_score(
        &name_words,
        &query_words,
        NAME_EXACT_SCORE,
        NAME_PREFIX_SCORE,
    );
    let other_score = words_score(
        &other_words,
        &query_words,
        FIELD_EXACT_SCORE,
        FIELD_PREFIX_SCORE,
    );

    match (name_score, other_score) {
        (Some(name), Some(other)) => Some(name + other),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

/// Scores every query word against one group of words. All words must match,
/// otherwise the field group is not relevant to the query.
fn words_score(haystack: &[&str], query_words: &[&str], exact: i32, prefix: i32) -> Option<i32> {
    let mut total = 0;

    for query_word in query_words {
        let mut best: Option<i32> = None;

        for word in haystack {
            let score = if *word == *query_word {
                exact
            } else if let Some(rest) = word.strip_prefix(query_word) {
                prefix - rest.chars().count() as i32
            } else {
                continue;
            };

            best = Some(best.map_or(score, |current| current.max(score)));
        }

        total += best?;
    }

    Some(total)
}

#[cfg(test)]
fn match_score(haystack: &str, needle: &str) -> Option<i32> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Some(0);
    }

    let query_words: Vec<&str> = split_words(needle).collect();
    if query_words.is_empty() {
        return Some(0);
    }
    let haystack_words: Vec<&str> = split_words(haystack).collect();

    words_score(
        &haystack_words,
        &query_words,
        FIELD_EXACT_SCORE,
        FIELD_PREFIX_SCORE,
    )
}

fn split_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{best_score, match_score};
    use crate::services::apps::desktop::DesktopApp;

    fn app(name: &str, fields: &[&str]) -> DesktopApp {
        DesktopApp {
            id: name.to_lowercase(),
            name: name.to_string(),
            description: None,
            icon_name: None,
            exec: vec![name.to_lowercase()],
            terminal: false,
            search_fields: fields.iter().map(|field| field.to_string()).collect(),
        }
    }

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

    #[test]
    fn prefers_matches_in_the_app_name() {
        let code = app("Visual Studio Code", &["visual studio code", "code"]);
        let kate = app("Kate", &["kate", "code"]);
        assert!(best_score(&code, "code") > best_score(&kate, "code"));
    }

    #[test]
    fn combines_name_and_keyword_words() {
        let files = app("Files", &["files", "folder", "manager", "file"]);
        let score = best_score(&files, "file manager").unwrap();
        assert!(score >= 2_000);
    }

    #[test]
    fn validates_the_resolved_app_against_the_request() {
        let mines = app("Mines", &["minesweeper", "buscaminas", "game", "strategy"]);
        assert!(super::matches_request(&mines, "open minesweeper"));
        assert!(super::matches_request(&mines, "abre el buscaminas"));
        assert!(super::matches_request(&mines, "open the game of mines"));
        assert!(!super::matches_request(&mines, "open solitaire"));

        let firefox = app("Firefox", &["fast and private browser"]);
        assert!(super::matches_request(&firefox, "open my browser"));
        assert!(!super::matches_request(&firefox, "open my spreadsheet"));

        let code = app("Visual Studio Code", &["code", "text editor"]);
        assert!(super::matches_request(&code, "open vscode"));
        assert!(super::matches_request(&code, "open my code editor"));
    }
}
