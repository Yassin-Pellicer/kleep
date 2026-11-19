/// Reads `DEEPSEEK_API_KEY`, treating blank values as absent.
pub fn deepseek_api_key() -> Option<String> {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
