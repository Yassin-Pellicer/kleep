use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// While a clipboard write is in flight, the focus-loss handler must not hide
/// the palette: `wl-copy` borrows keyboard focus briefly to get the input serial
/// it needs to own the selection.
static SUPPRESS_FOCUS_HIDE_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Keeps the palette open for `duration` even if the window loses focus.
pub fn suppress_focus_hide(duration: Duration) {
    let until = now_ms().saturating_add(duration.as_millis() as u64);
    SUPPRESS_FOCUS_HIDE_UNTIL_MS.store(until, Ordering::SeqCst);
}

pub fn focus_hide_suppressed() -> bool {
    now_ms() < SUPPRESS_FOCUS_HIDE_UNTIL_MS.load(Ordering::SeqCst)
}
