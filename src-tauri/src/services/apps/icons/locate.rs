use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::services::support::xdg;

pub(super) fn resolve(icon: &str) -> Option<PathBuf> {
    let direct = Path::new(icon);
    if direct.is_absolute() {
        return direct.is_file().then(|| direct.to_path_buf());
    }

    let file_name = direct
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(icon);
    let stem = strip_icon_extension(file_name);

    for root in icon_roots() {
        if root.file_name().and_then(|name| name.to_str()) == Some("pixmaps") {
            if let Some(path) = best_in_directory(&root, stem, 1) {
                return Some(path);
            }
            continue;
        }

        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };

        let mut themes: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();

        themes.sort_by_key(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            (theme_rank(name), name.to_string())
        });

        for theme in themes {
            if let Some(path) = best_in_directory(&theme, stem, 3) {
                return Some(path);
            }
        }
    }

    None
}

fn best_in_directory(directory: &Path, stem: &str, max_depth: usize) -> Option<PathBuf> {
    let mut best: Option<(i32, PathBuf)> = None;
    find_icon(directory, stem, 0, max_depth, &mut best);
    best.map(|(_, path)| path)
}

fn find_icon(
    directory: &Path,
    stem: &str,
    depth: usize,
    max_depth: usize,
    best: &mut Option<(i32, PathBuf)>,
) {
    if depth > max_depth {
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
            find_icon(&path, stem, depth + 1, max_depth, best);
            continue;
        }

        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };

        if !matches!(extension, "png" | "svg" | "xpm") {
            continue;
        }

        if path.file_stem().and_then(|name| name.to_str()) != Some(stem) {
            continue;
        }

        let score = icon_score(&path, extension);
        if best.as_ref().is_none_or(|(current, _)| score > *current) {
            *best = Some((score, path));
        }
    }
}

fn icon_score(path: &Path, extension: &str) -> i32 {
    let mut score = match extension {
        "png" => 40,
        "svg" => 35,
        "xpm" => 5,
        _ => 0,
    };

    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();

        if name == "apps" {
            score += 20;
        }
        if name.starts_with("scalable") {
            score += 10;
        }
        if name.contains("symbolic") {
            score -= 30;
        }
        if let Some(size) = parse_size(&name) {
            score += match size {
                48..=96 => 9,
                32..=47 => 6,
                24..=31 => 4,
                128.. => 8,
                _ => 2,
            };
        }
    }

    score
}

fn parse_size(name: &str) -> Option<u32> {
    let end = name.find('x')?;
    name[..end].parse().ok()
}

fn theme_rank(name: &str) -> u8 {
    match name {
        "hicolor" => 0,
        "Adwaita" => 1,
        "breeze" => 2,
        "Papirus" => 3,
        _ => 10,
    }
}

/// Strips only known icon suffixes. `Path::file_stem` would turn
/// `org.gnome.Calculator` into `org.gnome`.
fn strip_icon_extension(name: &str) -> &str {
    for extension in [".png", ".svg", ".xpm"] {
        if let Some(stem) = name.strip_suffix(extension) {
            return stem;
        }
    }

    name
}

fn icon_roots() -> Vec<PathBuf> {
    let home = xdg::home_dir();
    let data_home = xdg::data_home();

    let mut roots = vec![data_home.join("icons"), home.join(".icons")];

    for directory in xdg::data_dirs() {
        roots.push(directory.join("icons"));
        roots.push(directory.join("pixmaps"));
    }

    roots.push(PathBuf::from("/var/lib/flatpak/exports/share/icons"));
    roots.push(data_home.join("flatpak/exports/share/icons"));
    roots.push(PathBuf::from("/var/lib/snapd/desktop/icons"));

    let mut seen = HashSet::new();
    roots.retain(|root| root.is_dir() && seen.insert(root.clone()));
    roots
}

#[cfg(test)]
mod tests {
    use super::strip_icon_extension;

    #[test]
    fn strips_only_known_icon_extensions() {
        assert_eq!(
            strip_icon_extension("org.gnome.Calculator"),
            "org.gnome.Calculator"
        );
        assert_eq!(strip_icon_extension("firefox.png"), "firefox");
        assert_eq!(strip_icon_extension("firefox.svg"), "firefox");
    }
}
