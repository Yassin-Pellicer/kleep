use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::DesktopApp;
use super::paths::{application_dirs, command_exists};

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

    let mut search_fields = Vec::new();
    if let Some(description) = &description {
        search_fields.push(description.to_lowercase());
    }
    // Include localized keywords too: someone typing "gestor" in Spanish
    // should find Files just like someone typing "manager".
    for (key, value) in &entries {
        if key == "Keywords" || key.starts_with("Keywords[") {
            for keyword in value.split(';') {
                let keyword = keyword.trim();
                if !keyword.is_empty() {
                    search_fields.push(keyword.to_lowercase());
                }
            }
        }
    }
    if let Some(categories) = entries.get("Categories") {
        for category in categories.split(';') {
            let category = category.trim();
            if category.is_empty() {
                continue;
            }
            search_fields.push(category.to_lowercase());

            // Split CamelCase categories such as "WebBrowser" or "FileManager"
            // so "browser" and "manager" match as whole words.
            let mut current = String::new();
            for character in category.chars() {
                if character.is_uppercase() && !current.is_empty() {
                    search_fields.push(std::mem::take(&mut current).to_lowercase());
                }
                current.push(character);
            }
            if !current.is_empty() {
                search_fields.push(current.to_lowercase());
            }
        }
    }
    // Index localized names as well, so "ajedrez" finds Chess and "archivos"
    // finds Files. The visible name stays the unlocalized one.
    for (key, value) in &entries {
        if key.starts_with("Name[") || key.starts_with("GenericName") {
            let value = value.trim();
            if !value.is_empty() {
                search_fields.push(value.to_lowercase());
            }
        }
    }

    if let Some(program) = exec.first()
        && let Some(stem) = PathBuf::from(program)
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
