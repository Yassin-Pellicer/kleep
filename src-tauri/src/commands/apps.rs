use crate::models::AppEntry;
use crate::services::apps;

#[tauri::command]
pub async fn search_apps(query: String) -> Vec<AppEntry> {
    tauri::async_runtime::spawn_blocking(move || apps::search(&query))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn launch_app(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || apps::launch(&id))
        .await
        .map_err(|error| error.to_string())?
}
