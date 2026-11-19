use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::services::support::xdg;

pub fn application_dirs() -> Vec<PathBuf> {
    let data_home = xdg::data_home();

    let mut directories = vec![data_home.join("applications")];

    for directory in xdg::data_dirs() {
        directories.push(directory.join("applications"));
    }

    directories.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    directories.push(data_home.join("flatpak/exports/share/applications"));
    directories.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    let mut seen = HashSet::new();
    directories.retain(|directory| directory.is_dir() && seen.insert(directory.clone()));
    directories
}

pub fn command_exists(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty() {
        return false;
    }

    let first = command.split_whitespace().next().unwrap_or(command);
    let path = Path::new(first);

    if path.is_absolute() {
        return is_executable(path);
    }

    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|directory| is_executable(&directory.join(first)))
    })
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
