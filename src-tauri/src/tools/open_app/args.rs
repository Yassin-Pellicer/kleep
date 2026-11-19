use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OpenAppArgs {
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}
