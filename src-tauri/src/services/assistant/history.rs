use serde_json::{Value, json};

use crate::models::{ChatMessage, ServiceError};
use crate::services::apps;

use super::prompt;

/// Keep the injected catalog compact enough for a local model's window.
const MAX_APP_CONTEXT_CHARS: usize = 6_000;

/// Builds the bounded conversation sent to the provider.
///
/// Keeps recent turns only and caps each message. Local models often run
/// with a much smaller context window, so keep proportionally less history
/// for them.
pub fn build(
    messages: Vec<ChatMessage>,
    voice: bool,
    web_search: bool,
    local: bool,
) -> Result<Vec<Value>, ServiceError> {
    let max_messages = if local { 6 } else { 12 };
    let max_message_chars = if local { 3_000 } else { 8_000 };

    let history: Vec<ChatMessage> = messages
        .into_iter()
        .rev()
        .take(max_messages)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut conversation = vec![json!({
        "role": "system",
        "content": prompt::system_prompt(voice, web_search),
    })];

    if let Some(context) = installed_apps_context() {
        conversation.push(json!({
            "role": "system",
            "content": context,
        }));
    }

    let mut has_user_message = false;

    for message in history {
        let role = match message.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };

        let content: String = message.content.chars().take(max_message_chars).collect();
        if content.trim().is_empty() {
            continue;
        }

        has_user_message |= role == "user";
        conversation.push(json!({
            "role": role,
            "content": content,
        }));
    }

    if !has_user_message {
        return Err(ServiceError::InvalidInput(
            "no user message provided".to_string(),
        ));
    }

    Ok(conversation)
}

/// A system message listing the installed apps so the model stops guessing
/// whether something like Chess or Mines exists, and uses real names.
fn installed_apps_context() -> Option<String> {
    format_app_context(&apps::installed_names())
}

fn format_app_context(names: &[String]) -> Option<String> {
    let mut listed = String::new();
    for name in names {
        let needed = name.chars().count() + if listed.is_empty() { 0 } else { 2 };
        if listed.chars().count() + needed > MAX_APP_CONTEXT_CHARS {
            break;
        }
        if !listed.is_empty() {
            listed.push_str(", ");
        }
        listed.push_str(name);
    }

    if listed.is_empty() {
        return None;
    }

    Some(format!(
        "Installed applications on this machine (exact names for open_app's app field): {listed}. \
         When the user asks to open an application, choose a name from this list only when it is \
         the application they asked for, even if the user's wording differs from the installed \
         name. If the requested app is not listed, call open_app with the user's own words so its \
         keywords and aliases can be resolved. Never open a different app instead, and never use \
         run_command to search for or launch an application. If an application is not installed, \
         answer with one sentence and never ask a question: state that the app is not installed \
         and stop."
    ))
}

#[cfg(test)]
mod tests {
    use super::{MAX_APP_CONTEXT_CHARS, format_app_context};

    #[test]
    fn app_context_lists_exact_names() {
        let names = vec!["Chess".to_string(), "Mines".to_string()];
        let context = format_app_context(&names).expect("context should be built");

        assert!(context.contains("Chess, Mines"));
        assert!(context.contains("open_app"));
    }

    #[test]
    fn app_context_stays_within_the_budget() {
        let names: Vec<String> = (0..500)
            .map(|index| format!("Application {index} With A Long Name"))
            .collect();
        let context = format_app_context(&names).expect("context should be built");

        assert!(
            context.chars().count() <= MAX_APP_CONTEXT_CHARS + 800,
            "context was {} chars",
            context.chars().count()
        );
    }
}
