use std::path::PathBuf;

/// Reads `DEEPSEEK_API_KEY`, treating blank values as absent.
pub fn deepseek_api_key() -> Option<String> {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Loads `.env` files without overriding variables that are already set.
///
/// Development reads the repository `.env` first. Installed builds read
/// `~/.config/kleep/.env`, then a `.env` in the working directory or next to
/// the executable (portable installs).
pub fn load_dotenv() {
    #[cfg(debug_assertions)]
    {
        let _ = dotenvy::dotenv();
        if let Some(path) = config_env_path() {
            let _ = dotenvy::from_path(path);
        }
    }

    #[cfg(not(debug_assertions))]
    {
        if let Some(path) = config_env_path() {
            let _ = dotenvy::from_path(path);
        }

        let _ = dotenvy::dotenv();

        if let Ok(executable) = std::env::current_exe()
            && let Some(parent) = executable.parent()
        {
            let _ = dotenvy::from_path(parent.join(".env"));
        }
    }
}

fn config_env_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    Some(base.join("kleep").join(".env"))
}
