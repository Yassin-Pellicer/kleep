mod catalog;
mod desktop;
mod icons;
mod launch;
mod search;

pub use catalog::preload;
pub use desktop::open_terminal;
pub use launch::{launch, launch_by_name};
pub use search::search;

/// Names of every visible application in the catalog, for the assistant's
/// context. The model uses these exact names to call `open_app` instead of
/// guessing whether an app is installed.
pub fn installed_names() -> Vec<String> {
    let mut names = catalog::with_apps(|apps| {
        apps.iter()
            .map(|app| app.name.clone())
            .collect::<Vec<String>>()
    })
    .unwrap_or_default();

    names.sort_by_key(|name| name.to_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    names
}
