/// Keeps at most `max_chars` characters.
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Collapses all whitespace runs to single spaces.
pub fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
