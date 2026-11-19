use std::fs;
use std::path::{Path, PathBuf};

const SERVER_PY: &str = include_str!("../../../../voice/server.py");
const SETUP_SH: &str = include_str!("../../../../voice/setup.sh");
const REQUIREMENTS: &str = include_str!("../../../../voice/requirements.txt");

/// Where the embedded voice service is materialized for installed builds.
pub fn install_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;

    Some(base.join("kleep").join("voice"))
}

/// Writes the bundled scripts to the per-user data directory.
pub fn ensure_install() -> Option<PathBuf> {
    let directory = install_dir()?;
    if fs::create_dir_all(&directory).is_err() {
        return None;
    }

    write_if_changed(&directory.join("server.py"), SERVER_PY, false);
    write_if_changed(&directory.join("setup.sh"), SETUP_SH, true);
    write_if_changed(&directory.join("requirements.txt"), REQUIREMENTS, false);

    Some(directory)
}

fn write_if_changed(path: &Path, content: &str, executable: bool) {
    let unchanged = fs::read_to_string(path)
        .map(|existing| existing == content)
        .unwrap_or(false);

    if !unchanged {
        let _ = fs::write(path, content);
    }

    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
}
