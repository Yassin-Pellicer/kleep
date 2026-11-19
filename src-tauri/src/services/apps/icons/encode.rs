use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

pub(super) fn data_uri(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;

    let mime = match path.extension().and_then(|extension| extension.to_str()) {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("xpm") => "image/x-xpixmap",
        _ => return None,
    };

    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}
