use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// When the palette was last shown. Focus-loss events that fire while the
/// window is still being mapped must not hide it again.
static LAST_SHOWN: Mutex<Option<Instant>> = Mutex::new(None);

/// Invalidates a pending fade-out when the palette is shown again.
static HIDE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Set when Enter requests a clipboard paste; consumed once the palette hides.
static PENDING_PASTE: AtomicBool = AtomicBool::new(false);

pub fn mark_shown() {
    if let Ok(mut last) = LAST_SHOWN.lock() {
        *last = Some(Instant::now());
    }
}

/// A show cancels any pending fade-out.
pub fn invalidate_hides() {
    HIDE_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Starts a fade-out and returns the generation it belongs to.
pub fn begin_hide() -> u64 {
    HIDE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn hide_is_current(generation: u64) -> bool {
    HIDE_GENERATION.load(Ordering::SeqCst) == generation
}

pub fn focus_grace_over(grace: Duration) -> bool {
    LAST_SHOWN
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_none_or(|last| last.elapsed() >= grace)
}

pub fn request_paste() {
    PENDING_PASTE.store(true, Ordering::SeqCst);
}

/// A show cancels a paste that was waiting for the previous hide.
pub fn cancel_pending_paste() {
    PENDING_PASTE.store(false, Ordering::SeqCst);
}

pub fn take_pending_paste() -> bool {
    PENDING_PASTE.swap(false, Ordering::SeqCst)
}
