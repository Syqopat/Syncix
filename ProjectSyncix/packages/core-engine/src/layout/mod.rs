mod files;
mod identity;
mod own_writes;
mod settings;
mod trash;

pub(crate) use files::*;
pub(crate) use identity::*;
pub(crate) use own_writes::*;
pub(crate) use settings::*;
pub(crate) use trash::*;

use crate::model::{DataModel, InstanceNode};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// RECORD OF OUR OWN WRITES
//
// Problem: when the disk writer wrote a file, the watcher took it for a USER
// change and applied it to the model again. Because the event queue lags,
// the watcher sometimes read the OLD version of the file and rolled the model back.
//
// Observed result: an attribute added on disk sometimes stayed and sometimes vanished
// (one disappeared while another stayed) — the behaviour depended on a race condition.
//
// Fix: we note the content of every file we write. When the watcher reads a file
// whose content equals what we last wrote, there is nothing new to learn and it is skipped.
// No time window is used; because the comparison is by content,
// late events are filtered correctly too.
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// RECORD OF OUR OWN DELETIONS
//
// The deletion-side counterpart of the write log. The reconciler deletes files
// while rewriting the tree; if the watcher took those for user deletions it would destroy
// existing objects. So file deletion events used to be ignored entirely — but then
// deleting a file in the editor did nothing, and the file reappeared a few hundred
// milliseconds later.
//
// The fix mirrors the write side: we note the paths we delete. If a deletion event
// reaching the watcher is on this list it is ours and is skipped; otherwise the user
// deleted it and the instance really is destroyed.
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// TRASH
//
// The reconciler deletes files that have no counterpart in Studio's tree. That is correct
// behaviour — but one-way. An edit made on disk while the core was off is seen
// by nobody; when the core starts and writes Studio's tree, that file
// counts as "extra" and disappears, with no way back.
//
// So no managed file is deleted directly; it is moved to the trash.
// The trash is OUTSIDE the sync folder: inside, the watcher would take it for new content
// and the reconciler would delete it on the next pass.
// ---------------------------------------------------------------------------


/// `allow_removal`: until it is certain that the model reflects Studio's real tree
/// (until a FULL_SYNC has completed in this session) NO file is removed from disk.
/// Otherwise an empty model would mean "everything on disk is extra": opening the
/// editor with Studio closed moved every file in the sync folder to the trash.
pub fn write_full_tree(dm: &DataModel, sync_dir: &str, ignore: &[String], allow_removal: bool) {
    let root = Path::new(sync_dir);
    let _ = fs::create_dir_all(root);

    // 1) Files that should exist
    let mut expected: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    for (uuid, node) in dm.get_all_instances() {
        if node.class_name == "DataModel" {
            continue;
        }
        if let Some(file) = data_file(dm, sync_dir, uuid) {
            expected.insert(file, node_content(node));
        }
        // Scripts keep their properties/attributes in a separate meta file.
        if let Some(meta) = meta_file(dm, sync_dir, uuid) {
            expected.insert(meta, meta_content(node));
        }
    }

    // 2) Files that exist on disk
    let mut existing = Vec::new();
    collect_files(root, &mut existing);

    // 3) Remove the extras — only when the model is authoritative (see allow_removal).
    let mut removed = 0usize;
    let mut kept = 0usize;
    for path in &existing {
        if expected.contains_key(path) {
            continue;
        }
        // NEVER touch files we do not recognise (README, .gitkeep, personal notes).
        if !is_managed_file(path) {
            continue;
        }
        // Paths the user chose to ignore are protected too.
        if is_ignored(path, sync_dir, ignore) {
            continue;
        }
        if !allow_removal {
            kept += 1;
            continue;
        }
        if move_to_trash(path, sync_dir).is_ok() {
            forget_write(path);
            record_delete(path);
            removed += 1;
        }
    }

    // 4) Write what is missing or changed (leave identical files alone)
    let mut written = 0usize;
    for (path, content) in &expected {
        let unchanged = fs::read_to_string(path)
            .map(|current| current == *content)
            .unwrap_or(false);
        if unchanged {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(path, content) {
            Ok(_) => {
                record_write(path, content);
                written += 1;
            }
            Err(e) => tracing::error!("layout: could not write file ({:?}): {}", path, e),
        }
    }

    // 5) Clean up folders that became empty
    remove_empty_dirs(root, root);

    if kept > 0 {
        tracing::debug!(
            "layout: {} file(s) not in the model were kept — Studio has not synced yet",
            kept
        );
    }
    if written > 0 || removed > 0 {
        tracing::debug!(
            "layout: synced ({} instances) — {} written, {} removed",
            expected.len(),
            written,
            removed
        );
    }
}

#[cfg(test)]
mod tests;
