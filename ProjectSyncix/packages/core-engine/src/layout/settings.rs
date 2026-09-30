//! Split out of layout.rs.

#[allow(unused_imports)]
use super::*;

/// Mirrors the model to disk — using RECONCILE.
/// The whole folder used to be deleted and rewritten; that was slow for big scenes
/// and needlessly triggered the file watcher on every write. Now only the difference
/// is applied: files whose content is unchanged are not touched at all.
/// The trash behaviour comes from configuration. It is kept global because
/// write_full_tree's call chain is long and threading one setting through every
/// link would make the code unreadable.
pub(crate) static TRASH_CONFIG: OnceLock<Mutex<(bool, usize)>> = OnceLock::new();

/// Whether .meta.json files are written. When off, scripts' properties and
/// attributes are never written to disk; only the source file remains.
pub(crate) static META_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn configure_meta(is_enabled: bool) {
    META_ENABLED.store(is_enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn configure_trash(is_enabled: bool, run_count: usize) {
    let h = TRASH_CONFIG.get_or_init(|| Mutex::new((true, TRASH_KEEP_DEFAULT)));
    if let Ok(mut a) = h.lock() {
        *a = (is_enabled, run_count.max(1));
    }
}

pub(crate) fn trash_config() -> (bool, usize) {
    TRASH_CONFIG
        .get()
        .and_then(|h| h.lock().ok().map(|a| *a))
        .unwrap_or((true, TRASH_KEEP_DEFAULT))
}
