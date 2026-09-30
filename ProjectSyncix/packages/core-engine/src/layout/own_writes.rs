//! Split out of layout.rs.

#[allow(unused_imports)]
use super::*;

pub(crate) fn write_log() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static WRITE_HASHES: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn content_hash(file_content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    file_content.hash(&mut h);
    h.finish()
}

pub(crate) fn record_write(path: &Path, file_content: &str) {
    if let Ok(mut record) = write_log().lock() {
        record.insert(normalize(path), content_hash(file_content));
    }
}

pub(crate) fn forget_write(path: &Path) {
    if let Ok(mut record) = write_log().lock() {
        record.remove(&normalize(path));
    }
}

/// Is this the content we last wrote? If so the watcher must not process it.
pub fn is_own_write(path: &Path, file_content: &str) -> bool {
    write_log()
        .lock()
        .map(|k| k.get(&normalize(path)) == Some(&content_hash(file_content)))
        .unwrap_or(false)
}

pub(crate) fn delete_log() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    static WRITE_HASHES: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

pub(crate) fn record_delete(path: &Path) {
    if let Ok(mut k) = delete_log().lock() {
        k.insert(normalize(path));
    }
}

/// Is this deletion ours? The record is SINGLE-USE: the path asked about is dropped from the list,
/// so if the same path is later deleted by the user it is handled as a real
/// deletion.
pub fn is_own_delete(path: &Path) -> bool {
    delete_log()
        .lock()
        .map(|mut k| k.remove(&normalize(path)))
        .unwrap_or(false)
}
