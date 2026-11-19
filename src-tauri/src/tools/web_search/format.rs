use std::collections::HashSet;

use crate::models::{ChatSource, ServiceError};
use crate::services::search::{SearchResponse, SearchResult};
use crate::services::support::text::{compact, truncate_chars};

const MAX_SOURCES: usize = 8;

pub(super) fn collect_sources(
    response: &SearchResponse,
    sources: &mut Vec<ChatSource>,
    seen_urls: &mut HashSet<String>,
) {
    for result in &response.results {
        if sources.len() >= MAX_SOURCES {
            break;
        }
        if result.url.is_empty() || !seen_urls.insert(result.url.clone()) {
            continue;
        }

        sources.push(ChatSource {
            title: if result.title.is_empty() {
                result.url.clone()
            } else {
                result.title.clone()
            },
            url: result.url.clone(),
            snippet: result.snippet.clone(),
            date: result.date.clone(),
        });
    }
}

pub(super) fn format_results(query: &str, response: &SearchResponse, max_summary: usize) -> String {
    if response.results.is_empty() {
        return format!("No web results found for \"{query}\".");
    }

    let mut text = format!("Web results for \"{query}\":");

    for (index, result) in response.results.iter().enumerate() {
        text.push_str(&format!(
            "\n\n{}. {}\nURL: {}\n",
            index + 1,
            display_title(result),
            result.url
        ));

        let summary = if result.summary.is_empty() {
            &result.snippet
        } else {
            &result.summary
        };
        text.push_str(&compact_text(summary, max_summary));

        if let Some(date) = &result.date
            && !date.is_empty()
        {
            text.push_str(&format!("\nPublished: {date}"));
        }
    }

    text.push_str(
        "\n\nUse these summaries to answer. Cite source URLs in plain text when relevant.",
    );

    text
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let normalized = compact(text);
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!("{}…", truncate_chars(&normalized, max_chars))
}

fn display_title(result: &SearchResult) -> &str {
    if result.title.is_empty() {
        &result.url
    } else {
        &result.title
    }
}

pub(super) fn friendly_error(error: &ServiceError) -> &'static str {
    match error {
        ServiceError::MissingApiKey => "the API key is missing",
        ServiceError::Unauthorized(_) => "the search service rejected the API key",
        ServiceError::RateLimited(_) => "the search rate limit was reached",
        ServiceError::Timeout(_) => "the search timed out",
        ServiceError::Network(_) => "there is no connection to the search service",
        ServiceError::BadResponse(_) | ServiceError::Server(_) => {
            "the search service returned an error"
        }
        ServiceError::InvalidInput(_) => "the search query was invalid",
    }
}
