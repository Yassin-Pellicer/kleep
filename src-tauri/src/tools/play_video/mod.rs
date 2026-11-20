use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::services::support::text::truncate_chars;

use super::context::{ToolContext, ToolOutcome};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Deserialize)]
struct PlayVideoArgs {
    query: String,
}

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "play_video",
            "description": "Play or watch any video on YouTube. It searches YouTube with yt-dlp \
                            and opens the first result in the default browser. Use it whenever \
                            the user asks to play, watch, put on, or listen to a video, song, \
                            music video, trailer, clip, tutorial, or any other YouTube content. \
                            Put the title or search terms in query; never include words such as \
                            play or watch, or a platform name such as YouTube or Spotify.",
            "strict": true,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The video title or search terms, for example \"Houdini Dua Lipa\" or \"Dune Part Two trailer\"."
                    }
                }
            }
        }
    })
}

fn candidate_commands() -> Vec<(String, Vec<&'static str>)> {
    let mut candidates = vec![("yt-dlp".to_string(), Vec::new())];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push((format!("{home}/.local/bin/yt-dlp"), Vec::new()));
    }
    candidates.push(("python3".to_string(), vec!["-m", "yt_dlp"]));
    candidates
}

fn is_video_id(value: &str) -> bool {
    value.len() == 11
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

async fn search_video_id(query: &str) -> Result<String, String> {
    let search = format!("ytsearch1:{query}");
    let mut last_error = "yt-dlp is not installed".to_string();

    for (program, prefix) in candidate_commands() {
        let mut command = tokio::process::Command::new(&program);
        command
            .args(prefix)
            .arg("--no-warnings")
            .arg("--get-id")
            .arg(&search)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(SEARCH_TIMEOUT, command.output()).await {
            Err(_) => return Err("the YouTube search timed out".to_string()),
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                last_error = format!("{program}: {error}");
                continue;
            }
            Ok(Err(error)) => return Err(format!("could not run yt-dlp: {error}")),
            Ok(Ok(output)) => output,
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "yt-dlp could not search YouTube: {}",
                truncate_chars(stderr.trim(), 300)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(id) = stdout.lines().map(str::trim).find(|line| is_video_id(line)) {
            return Ok(id.to_string());
        }

        return Err(format!("no YouTube result found for \"{query}\""));
    }

    Err(format!(
        "yt-dlp is not installed ({last_error}). Install it with \
         `python3 -m pip install --user yt-dlp`."
    ))
}

pub(super) async fn execute(arguments: &str, context: &mut ToolContext<'_>) -> ToolOutcome {
    let args: PlayVideoArgs = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) => return ToolOutcome::text("Invalid arguments for play_video."),
    };

    let query = args.query.trim();
    if query.is_empty() {
        return ToolOutcome::text("Provide a video title or search terms to play.");
    }

    let id = match search_video_id(query).await {
        Ok(id) => id,
        Err(error) => return ToolOutcome::text(error),
    };

    let url = format!("https://www.youtube.com/watch?v={id}");
    let arguments = json!({ "url": url }).to_string();
    super::open_app::execute(&arguments, context).await
}
