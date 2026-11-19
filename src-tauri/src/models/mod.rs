mod app;
mod assistant;
mod clipboard;
mod dictionary;
mod error;
mod voice;

pub use app::AppEntry;
pub use assistant::{AssistantReply, ChatMessage, ChatSource};
pub use clipboard::ClipboardEntry;
pub use dictionary::DictionaryEntry;
pub use error::ServiceError;
pub use voice::{VoiceRecordingStatus, VoiceSetupStatus, VoiceTranscript};

/// Historical name for `ServiceError`; kept so domain code reads naturally.
pub type AssistantError = ServiceError;
/// Historical name for `ServiceError`; kept so domain code reads naturally.
pub type LookupError = ServiceError;
