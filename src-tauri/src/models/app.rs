use serde::Serialize;

/// One launchable application entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A `data:` URI so the webview can display the icon without the asset
    /// protocol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
