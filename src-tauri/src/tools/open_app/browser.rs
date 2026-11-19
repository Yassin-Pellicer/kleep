use std::process::{Command, Stdio};

pub(super) fn open_in_browser(url: &str) -> Result<(), String> {
    let candidates: [(&str, &[&str]); 2] = [("xdg-open", &[]), ("gio", &["open"])];
    let mut last_error = String::new();

    for (program, args) in candidates {
        let mut command = Command::new(program);
        command
            .args(args)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(error) => last_error = format!("{program}: {error}"),
        }
    }

    Err(format!("couldn't open the link ({last_error})"))
}
