use std::env;
use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `XDG_DATA_HOME`, ignoring relative values.
pub fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

/// `XDG_DATA_DIRS`, with the XDG defaults when unset or empty.
pub fn data_dirs() -> Vec<PathBuf> {
    let default_dirs = || {
        vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ]
    };

    match env::var_os("XDG_DATA_DIRS") {
        Some(value) => {
            let dirs: Vec<PathBuf> = env::split_paths(&value).collect();
            if dirs.is_empty() {
                default_dirs()
            } else {
                dirs
            }
        }
        None => default_dirs(),
    }
}
