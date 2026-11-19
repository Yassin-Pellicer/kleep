use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WebSearchArgs {
    pub query: String,
}
