use crate::models::AppEntry;

use super::catalog;
use super::desktop::DesktopApp;
use super::icons;

const MAX_RESULTS: usize = 16;

pub fn search(query: &str) -> Vec<AppEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<(i32, DesktopApp)> = match catalog::with_apps(|apps| {
        apps.iter()
            .filter_map(|app| best_score(app, &query).map(|score| (score, app.clone())))
            .collect()
    }) {
        Some(matches) => matches,
        None => return Vec::new(),
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

pub(super) fn best_score(app: &DesktopApp, query: &str) -> Option<i32> {
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
