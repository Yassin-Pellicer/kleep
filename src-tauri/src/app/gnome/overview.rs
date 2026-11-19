use std::process::Command;

pub fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|desktop| desktop.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
}

pub fn overview_is_active() -> bool {
    if !is_gnome() {
        return false;
    }

    let Ok(output) = Command::new("gdbus")
        .args([
            "call",
            "-t",
            "1",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.gnome.Shell",
            "OverviewActive",
        ])
        .output()
    else {
        return false;
    };

    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("true")
}
