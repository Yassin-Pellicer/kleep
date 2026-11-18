use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_name: Option<String>,
    pub exec: Vec<String>,
    pub terminal: bool,
    pub search_fields: Vec<String>,
}

impl DesktopApp {
    pub fn spawn(&self) -> std::io::Result<()> {
        if self.terminal
            && let Some(terminal) = terminal_command()
        {
            let mut command = Command::new(&terminal[0]);
            command.args(&terminal[1..]).arg("-e").args(&self.exec);

            if spawn_detached(command).is_ok() {
                return Ok(());
            }
        }

        let mut command = Command::new(&self.exec[0]);
        command.args(&self.exec[1..]);
        spawn_detached(command)
    }
}

fn spawn_detached(mut command: Command) -> std::io::Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn()?;

    // Reap the child without blocking the caller.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

fn terminal_command() -> Option<Vec<String>> {
    if let Some(value) = env::var_os("TERMINAL") {
        let value = value.to_string_lossy();
        let parts: Vec<String> = value.split_whitespace().map(str::to_string).collect();
        if !parts.is_empty() {
            return Some(parts);
        }
    }

    const CANDIDATES: &[&str] = &[
        "x-terminal-emulator",
        "konsole",
        "alacritty",
        "foot",
        "xterm",
        "gnome-terminal",
    ];

    CANDIDATES
        .iter()
        .find(|candidate| command_exists(candidate))
        .map(|candidate| vec![candidate.to_string()])
}

pub fn load_apps() -> Vec<DesktopApp> {
    let mut by_id: HashMap<String, DesktopApp> = HashMap::new();

    for directory in application_dirs() {
        if directory.is_dir() {
            walk(&directory, &directory, 0, &mut by_id);
        }
    }

    let mut apps: Vec<DesktopApp> = by_id.into_values().collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

fn walk(base: &Path, directory: &Path, depth: usize, apps: &mut HashMap<String, DesktopApp>) {
    if depth > 3 {
        return;
    }

    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            walk(base, &path, depth + 1, apps);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("desktop") {
            continue;
        }

        let Ok(relative) = path.strip_prefix(base) else {
            continue;
        };
        let id = relative.to_string_lossy().replace('/', "-");

        // Earlier application directories take precedence.
        if apps.contains_key(&id) {
            continue;
        }

        if let Some(app) = parse_desktop_file(&path, id) {
            apps.insert(app.id.clone(), app);
        }
    }
}

fn parse_desktop_file(path: &Path, id: String) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;

    let mut in_desktop_entry = false;
    let mut entries: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    if entries.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }

    if is_true(entries.get("NoDisplay")) || is_true(entries.get("Hidden")) {
        return None;
    }

    if let Some(try_exec) = entries.get("TryExec")
        && !command_exists(try_exec)
    {
        return None;
    }

    let name = entries.get("Name")?.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let exec = parse_exec(entries.get("Exec")?);
    if exec.is_empty() {
        return None;
    }

    let description = entries
        .get("GenericName")
        .or_else(|| entries.get("Comment"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let icon_name = entries
        .get("Icon")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut search_fields = vec![name.to_lowercase()];
    if let Some(description) = &description {
        search_fields.push(description.to_lowercase());
    }
    if let Some(keywords) = entries.get("Keywords") {
        for keyword in keywords.split(';') {
            let keyword = keyword.trim();
            if !keyword.is_empty() {
                search_fields.push(keyword.to_lowercase());
            }
        }
    }
    if let Some(program) = exec.first()
        && let Some(stem) = Path::new(program)
            .file_stem()
            .and_then(|stem| stem.to_str())
    {
        search_fields.push(stem.to_lowercase());
    }

    Some(DesktopApp {
        id,
        name,
        description,
        icon_name,
        exec,
        terminal: is_true(entries.get("Terminal")),
        search_fields,
    })
}

fn is_true(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn data_dirs() -> Vec<PathBuf> {
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

fn application_dirs() -> Vec<PathBuf> {
    let data_home = data_home();

    let mut directories = vec![data_home.join("applications")];

    for directory in data_dirs() {
        directories.push(directory.join("applications"));
    }

    directories.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    directories.push(data_home.join("flatpak/exports/share/applications"));
    directories.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    let mut seen = HashSet::new();
    directories.retain(|directory| directory.is_dir() && seen.insert(directory.clone()));
    directories
}

fn command_exists(command: &str) -> bool {
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

fn parse_exec(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = raw.chars();

    while let Some(character) = chars.next() {
        match character {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args.into_iter()
        .filter_map(|argument| {
            let (cleaned, removed_field_code) = strip_field_codes(&argument);
            if cleaned.is_empty() && removed_field_code {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect()
}

/// Removes `%f`, `%U`, `%i`, etc. from a desktop entry argument.
fn strip_field_codes(argument: &str) -> (String, bool) {
    let mut result = String::new();
    let mut removed = false;
    let mut chars = argument.chars();

    while let Some(character) = chars.next() {
        if character != '%' {
            result.push(character);
            continue;
        }

        match chars.next() {
            Some('%') => result.push('%'),
            Some(_) => removed = true,
            None => result.push('%'),
        }
    }

    (result, removed)
}

#[cfg(test)]
mod tests {
    use super::parse_exec;

    #[test]
    fn strips_field_codes() {
        assert_eq!(parse_exec("firefox %U"), vec!["firefox"]);
        assert_eq!(
            parse_exec("app --name \"hello world\" %f"),
            vec!["app", "--name", "hello world"]
        );
        assert_eq!(
            parse_exec("sh -c \"echo 100%%\""),
            vec!["sh", "-c", "echo 100%"]
        );
    }
}
