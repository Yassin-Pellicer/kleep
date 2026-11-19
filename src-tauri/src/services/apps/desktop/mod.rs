mod parser;
mod paths;
mod terminal;

use std::process::Command;

pub use parser::load_apps;

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
            && let Some(terminal) = terminal::terminal_command()
        {
            let mut command = Command::new(&terminal[0]);
            command.args(&terminal[1..]).arg("-e").args(&self.exec);

            if terminal::spawn_detached(command).is_ok() {
                return Ok(());
            }
        }

        let mut command = Command::new(&self.exec[0]);
        command.args(&self.exec[1..]);
        terminal::spawn_detached(command)
    }
}
