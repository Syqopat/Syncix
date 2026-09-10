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

fn write_log() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static WRITE_HASHES: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_hash(file_content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    file_content.hash(&mut h);
    h.finish()
}

fn record_write(path: &Path, file_content: &str) {
    if let Ok(mut record) = write_log().lock() {
        record.insert(path.to_path_buf(), content_hash(file_content));
    }
}

fn forget_write(path: &Path) {
    if let Ok(mut record) = write_log().lock() {
        record.remove(path);
    }
}

/// Is this the content we last wrote? If so the watcher must not process it.
pub fn is_own_write(path: &Path, file_content: &str) -> bool {
    write_log()
        .lock()
        .map(|k| k.get(path) == Some(&content_hash(file_content)))
        .unwrap_or(false)
}

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

fn delete_log() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    static WRITE_HASHES: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn record_delete(path: &Path) {
    if let Ok(mut k) = delete_log().lock() {
        k.insert(path.to_path_buf());
    }
}

/// Is this deletion ours? The record is SINGLE-USE: the path asked about is dropped from the list,
/// so if the same path is later deleted by the user it is handled as a real
/// deletion.
pub fn is_own_delete(path: &Path) -> bool {
    delete_log()
        .lock()
        .map(|mut k| k.remove(path))
        .unwrap_or(false)
}

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

/// <sync_dir>/../.syncix/trash
fn trash_root(sync_dir: &str) -> PathBuf {
    let s = Path::new(sync_dir);
    let upper = s.parent().unwrap_or(s);
    upper.join(".syncix").join("trash")
}

/// Keeps the newest TRASH_KEEP_DEFAULT folders; older ones are deleted completely.
const TRASH_KEEP_DEFAULT: usize = 10;

/// The run name is generated once and kept, so every deletion in the same core session
/// lands in a single folder.
fn run_label() -> String {
    static TRASH_RUN: OnceLock<String> = OnceLock::new();
    TRASH_RUN.get_or_init(|| chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string())
        .clone()
}

/// Moves a file to the trash instead of deleting it. The path relative to the sync folder
/// is kept, so restoring is a plain copy.
fn move_to_trash(path: &Path, sync_dir: &str) -> std::io::Result<()> {
    // With the trash turned off the file is deleted directly. Whoever asks for that knows
    // there is no way back; the setting's description says so too.
    if !trash_config().0 {
        return fs::remove_file(path);
    }
    let rel_path = path.strip_prefix(sync_dir).unwrap_or(path);
    let dest = trash_root(sync_dir).join(run_label()).join(rel_path);
    if let Some(upper) = dest.parent() {
        fs::create_dir_all(upper)?;
    }
    // rename is cheap on the same volume; across volumes it falls back to copy and delete.
    if fs::rename(path, &dest).is_err() {
        fs::copy(path, &dest)?;
        fs::remove_file(path)?;
    }
    prune_trash(sync_dir);
    Ok(())
}

/// The trash must not grow without bound: the newest TRASH_KEEP_DEFAULT folders stay.
fn prune_trash(sync_dir: &str) {
    let root_dir = trash_root(sync_dir);
    let Ok(input_list) = fs::read_dir(&root_dir) else {
        return;
    };
    let mut runs: Vec<PathBuf> = input_list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    let to_keep = trash_config().1;
    if runs.len() <= to_keep {
        return;
    }
    // Folder names are timestamps, so sorting by name sorts by time.
    runs.sort();
    let to_delete = runs.len() - to_keep;
    for previous_text in runs.into_iter().take(to_delete) {
        let _ = fs::remove_dir_all(previous_text);
    }
}

/// Lists trash runs, newest first: (run name, file count).
pub fn trash_runs(sync_dir: &str) -> Vec<(String, usize)> {
    let root_dir = trash_root(sync_dir);
    let Ok(input_list) = fs::read_dir(&root_dir) else {
        return Vec::new();
    };
    let mut runs: Vec<PathBuf> = input_list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    runs.sort();
    runs.reverse();
    runs
        .into_iter()
        .map(|t| {
            let mut file_list = Vec::new();
            collect_files(&t, &mut file_list);
            (
                t.file_name().unwrap_or_default().to_string_lossy().to_string(),
                file_list.len(),
            )
        })
        .collect()
}

/// Restores the files of one run into the sync folder. An existing file is never
/// overwritten — restoring must not cause data loss of its own.
/// Returns: (restored, skipped because they would have overwritten a file).
pub fn restore_from_trash(sync_dir: &str, run_name: &str) -> (usize, usize) {
    let origin = trash_root(sync_dir).join(run_name);
    let mut file_list = Vec::new();
    collect_files(&origin, &mut file_list);

    let (mut restored_count, mut skipped) = (0usize, 0usize);
    for d in file_list {
        let Ok(rel_path) = d.strip_prefix(&origin) else {
            continue;
        };
        let dest = Path::new(sync_dir).join(rel_path);
        if dest.exists() {
            skipped += 1;
            continue;
        }
        if let Some(upper) = dest.parent() {
            let _ = fs::create_dir_all(upper);
        }
        if fs::copy(&d, &dest).is_ok() {
            restored_count += 1;
        }
    }
    (restored_count, skipped)
}

/// Finds which instance a path on disk belongs to.
///
/// The reverse direction (uuid -> path) is `data_file`; to handle a deletion event we
/// have to start from the path itself, because the file can no longer be read.
/// Returns a uuid only when the DATA file matches: deleting the meta file
/// is not deleting the instance.
/// Is a path a component-wise suffix of the expected path?
///
/// Plain equality does not work: the watcher reports ABSOLUTE paths, while data_file
/// produces RELATIVE paths starting at sync_dir. Path::ends_with alone is not
/// enough either, because sync_dir often arrives as "./src_workspace" and
/// the leading "." counts as a separate component and breaks the match. So "." and ""
/// components are dropped on both sides.
///
/// Because the suffix includes sync_dir, a false match is not possible in practice.
fn suffix_matches(fs_path: &Path, expected_value: &Path) -> bool {
    use std::path::Component;
    let clean_up = |p: &Path| -> Vec<std::ffi::OsString> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_os_string())
            .collect()
    };
    let a = clean_up(fs_path);
    let b = clean_up(expected_value);
    if b.is_empty() || b.len() > a.len() {
        return false;
    }
    a[a.len() - b.len()..] == b[..]
}

pub fn uuid_for_path(dm: &DataModel, sync_dir: &str, path: &Path) -> Option<Uuid> {
    for uuid in dm.get_all_instances().keys() {
        let Some(expected_value) = data_file(dm, sync_dir, uuid) else {
            continue;
        };
        if suffix_matches(path, &expected_value) {
            return Some(*uuid);
        }
        if let Some(name_str) = expected_value.file_name().and_then(|f| f.to_str()) {
            if name_str.ends_with(".json") && !name_str.ends_with(".meta.json") {
                if let Some((base, _)) = name_str.strip_suffix(".json").and_then(|s| s.rsplit_once('.')) {
                    let legacy = expected_value.with_file_name(format!("{}.json", base));
                    if suffix_matches(path, &legacy) {
                        return Some(*uuid);
                    }
                }
            }
        }
    }
    None
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*\0".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn seg(dm: &DataModel, node: &InstanceNode) -> String {
    let clean = sanitize(&node.name);
    let has_sibling_collision = if let Some(pid) = node.parent {
        dm.get_instance(&pid).map(|p| {
            p.children.iter().any(|cid| {
                cid != &node.syncix_id && dm.get_instance(cid).map(|cn| sanitize(&cn.name) == clean).unwrap_or(false)
            })
        }).unwrap_or(false)
    } else {
        false
    };

    if has_sibling_collision {
        let short = &node.syncix_id.to_string()[0..8];
        format!("{}_{}", clean, short)
    } else {
        clean
    }
}

fn script_ext(class_name: &str) -> Option<&'static str> {
    match class_name {
        "Script" => Some("server.lua"),
        "LocalScript" => Some("client.lua"),
        "ModuleScript" => Some("lua"),
        // A StringValue's only meaningful field is Value; rather than embedding it in JSON
        // and dealing with escape characters, we write it as a plain text
        // file. That way the text can be edited directly in the editor.
        "StringValue" => Some("txt"),
        // LocalizationTable.Contents is a JSON string; we write it to disk as CSV
        // so translations can be edited in Excel/Sheets. The conversion is lossless and
        // locked by the round-trip tests in localization.rs.
        "LocalizationTable" => Some("csv"),
        _ => None,
    }
}

fn is_script(node: &InstanceNode) -> bool {
    matches!(
        node.class_name.as_str(),
        "Script" | "LocalScript" | "ModuleScript"
    )
}

fn instance_ext(class_name: &str) -> String {
    if let Some(ext) = script_ext(class_name) {
        ext.to_string()
    } else {
        format!("{}.json", class_name.to_ascii_lowercase())
    }
}

/// Is this class written to disk as RAW CONTENT (instead of its own .json)?
/// Such classes keep their properties/attributes in a .meta.json file.
fn with_raw_content(node: &InstanceNode) -> bool {
    script_ext(&node.class_name).is_some()
}

/// Raw text written to disk for classes like StringValue.
fn raw_str(node: &InstanceNode) -> String {
    if is_script(node) {
        return node.source.clone().unwrap_or_default();
    }

    if node.class_name == "LocalizationTable" {
        let contents = match node.properties.get("Contents") {
            Some(crate::model::PropertyValue::String(s)) => s.as_str(),
            _ => "[]",
        };
        return match crate::localization::json_to_csv(contents) {
            Ok(csv) => csv,
            Err(e) => {
                tracing::warn!("Could not convert LocalizationTable to CSV ({}): {}", node.name, e);
                String::new()
            }
        };
    }

    match node.properties.get("Value") {
        Some(crate::model::PropertyValue::String(s)) => s.clone(),
        Some(rest) => format!("{:?}", rest),
        None => String::new(),
    }
}

fn ancestry(dm: &DataModel, uuid: &Uuid) -> Vec<Uuid> {
    let mut chain = Vec::new();
    let mut cur = Some(*uuid);
    let mut guard = 0;
    while let Some(id) = cur {
        chain.push(id);
        cur = dm.get_instance(&id).and_then(|n| n.parent);
        guard += 1;
        if guard > 512 {
            break;
        }
    }
    chain.reverse();
    chain
}

/// The path of an object's file on disk. Sourcemap generation uses it too, so
/// path computation lives in one place.
pub fn data_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    let node = dm.get_instance(uuid)?;
    let chain = ancestry(dm, uuid);

    let mut path = PathBuf::from(sync_dir);
    if chain.len() > 1 {
        for aid in &chain[..chain.len() - 1] {
            if let Some(n) = dm.get_instance(aid) {
                path.push(seg(dm, n));
            }
        }
    }

    let ext = instance_ext(&node.class_name);
    if node.children.is_empty() {
        path.push(format!("{}.{}", seg(dm, node), ext));
    } else {
        path.push(seg(dm, node));
        path.push(format!("init.{}", ext));
    }
    Some(path)
}

/// Property/attribute file that sits next to a script file.
///
/// Why it is needed: scripts are written to disk as raw source (.lua), so
/// there is no room for their properties and attributes. Fields like Disabled and RunContext
/// and ALL attributes were lost on the disk side. A plain version of Rojo's .meta.json
/// idea: no magic `$` keys, just two fields.
///
/// Only produced when there is something to write; folders are not littered with empty meta files.
fn meta_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    if !META_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let node = dm.get_instance(uuid)?;
    if !with_raw_content(node) {
        return None; // other objects' own .json file already holds everything
    }
    // For a StringValue the Value already lives in the .txt file; repeating it in meta is pointless.
    let non_value_property = node
        .properties
        .keys()
        .any(|k| is_script(node) || (k != "Value" && k != "Contents"));
    // Tags count as something to write too: if no meta file were produced for a script
    // that only has tags, the tags would never reach disk.
    if !non_value_property && node.attributes.is_empty() && node.tags.is_empty() {
        return None;
    }
    let script_path = data_file(dm, sync_dir, uuid)?;
    let dir = script_path.parent()?;
    let item_name = if node.children.is_empty() {
        format!("{}.meta.json", seg(dm, node))
    } else {
        "init.meta.json".to_string()
    };
    Some(dir.join(item_name))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ScriptMeta {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub properties: std::collections::BTreeMap<String, crate::model::PropertyValue>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub attributes: std::collections::BTreeMap<String, crate::model::PropertyValue>,
    /// CollectionService tags. Never written to the file when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn meta_content(node: &InstanceNode) -> String {
    let mut properties = node.properties.clone();
    // A StringValue's Value is kept in the .txt file; keeping it in two places
    // risks the two drifting apart.
    if !is_script(node) {
        // Fields kept in the raw content file are NOT REPEATED in meta;
        // keeping the same data in two places risks the two drifting apart.
        properties.remove("Value");
        properties.remove("Contents");
    }
    let meta = ScriptMeta {
        properties,
        attributes: node.attributes.clone(),
        tags: node.tags.clone(),
    };
    serde_json::to_string_pretty(&meta).unwrap_or_default()
}

fn node_content(node: &InstanceNode) -> String {
    if with_raw_content(node) {
        raw_str(node)
    } else {
        serde_json::to_string_pretty(node).unwrap_or_default()
    }
}

/// Is this a file Syncix COULD HAVE PRODUCED?
///
/// The reconcile writer used to delete every file not on the expected list, so
/// any file placed in the sync folder (README, .gitkeep, personal notes)
/// silently disappeared. Measured and confirmed: a NOTES.md file was deleted on the
/// first write.
///
/// Rojo does not have this problem because Rojo never writes to disk. A one-way
/// "ignore list" is not enough for us; the REAL rule is: we only touch files in
/// formats we could produce ourselves. A file we do not recognise is never deleted.
fn is_managed_file(path: &Path) -> bool {
    let Some(item_name) = path.file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    item_name.ends_with(".meta.json")
        || item_name.ends_with(".server.lua")
        || item_name.ends_with(".client.lua")
        || item_name.ends_with(".lua")
        || item_name.ends_with(".luau")
        || item_name.ends_with(".txt")
        || item_name.ends_with(".csv")
        || item_name.ends_with(".json")
}

/// Does the path match one of the user's ignore patterns?
/// Patterns are evaluated relative to the sync folder (e.g. "notes/**", "*.md").
pub fn is_ignored(path: &Path, sync_dir: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let rel = path.strip_prefix(sync_dir).unwrap_or(path);
    let text_value = rel.to_string_lossy().replace('\\', "/");

    patterns.iter().any(|d| {
        glob::Pattern::new(d)
            .map(|p| p.matches(&text_value))
            .unwrap_or(false)
    })
}

/// Collects every file in the folder (recursively).
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

/// Removes folders that became empty (from the leaves towards the root). The root is kept.
fn remove_empty_dirs(dir: &Path, root: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                remove_empty_dirs(&p, root);
            }
        }
    }
    if dir != root {
        if let Ok(mut it) = fs::read_dir(dir) {
            if it.next().is_none() {
                let _ = fs::remove_dir(dir);
            }
        }
    }
}

/// Mirrors the model to disk — using RECONCILE.
/// The whole folder used to be deleted and rewritten; that was slow for big scenes
/// and needlessly triggered the file watcher on every write. Now only the difference
/// is applied: files whose content is unchanged are not touched at all.
/// The trash behaviour comes from configuration. It is kept global because
/// write_full_tree's call chain is long and threading one setting through every
/// link would make the code unreadable.
static TRASH_CONFIG: OnceLock<Mutex<(bool, usize)>> = OnceLock::new();

/// Whether .meta.json files are written. When off, scripts' properties and
/// attributes are never written to disk; only the source file remains.
static META_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn configure_meta(is_enabled: bool) {
    META_ENABLED.store(is_enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn configure_trash(is_enabled: bool, run_count: usize) {
    let h = TRASH_CONFIG.get_or_init(|| Mutex::new((true, TRASH_KEEP_DEFAULT)));
    if let Ok(mut a) = h.lock() {
        *a = (is_enabled, run_count.max(1));
    }
}

fn trash_config() -> (bool, usize) {
    TRASH_CONFIG
        .get()
        .and_then(|h| h.lock().ok().map(|a| *a))
        .unwrap_or((true, TRASH_KEEP_DEFAULT))
}

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
mod meta_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// NO meta file is produced for a script without properties.
    /// Otherwise an empty .meta.json would pile up next to every script.
    #[test]
    fn script_without_properties_has_no_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        assert!(meta_file(&m, "src", &s).is_none());
    }

    /// With a property or attribute, the meta file is created NEXT TO the script.
    #[test]
    fn meta_file_created_next_to_script_with_properties() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let meta = meta_file(&m, "src", &s).expect("a meta file was expected");
        let script = data_file(&m, "src", &s).unwrap();
        assert_eq!(meta.parent(), script.parent(), "must be in the same folder");
        assert!(meta.to_string_lossy().ends_with("Main.meta.json"), "{:?}", meta);
    }

    #[test]
    fn attribute_alone_produces_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .attributes
            .insert("Version".into(), PropertyValue::Number(3.0));

        assert!(meta_file(&m, "src", &s).is_some());
    }

    /// A script with children becomes a container folder; its meta name becomes init.meta.json too.
    #[test]
    fn container_script_uses_init_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        add_instance(&mut m, "ModuleScript", "Sub", Some(s));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let meta = meta_file(&m, "src", &s).unwrap();
        assert!(meta.to_string_lossy().ends_with("init.meta.json"), "{:?}", meta);
    }

    /// NO meta is produced for non-script objects: their own .json file
    /// already holds properties and attributes; a second file would be confusing.
    #[test]
    fn non_script_object_has_no_meta() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Box", Some(ws));
        m.get_mut_instance(&p)
            .unwrap()
            .properties
            .insert("Anchored".into(), PropertyValue::Boolean(true));

        assert!(meta_file(&m, "src", &p).is_none());
    }

    /// Meta content must be able to round-trip.
    #[test]
    fn meta_content_round_trip() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        {
            let n = m.get_mut_instance(&s).unwrap();
            n.properties.insert("Disabled".into(), PropertyValue::Boolean(true));
            n.properties.insert(
                "RunContext".into(),
                PropertyValue::String("Enum.RunContext.Server".into()),
            );
            n.attributes.insert("Version".into(), PropertyValue::Number(2.0));
        }

        let text_value = meta_content(m.get_instance(&s).unwrap());
        let restored_count: ScriptMeta = serde_json::from_str(&text_value).expect("could not parse");
        assert_eq!(restored_count.properties.len(), 2);
        assert_eq!(
            restored_count.properties.get("RunContext"),
            Some(&PropertyValue::String("Enum.RunContext.Server".into()))
        );
        assert_eq!(restored_count.attributes.get("Version"), Some(&PropertyValue::Number(2.0)));
    }
}

#[cfg(test)]
mod txt_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// A StringValue is written to disk as .txt, not as .json.
    #[test]
    fn stringvalue_is_written_as_txt() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("hello".into()));

        let fs_path = data_file(&m, "src", &sv).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Message.txt"), "{:?}", fs_path);
    }

    /// The file's content is the Value itself; no JSON wrapper.
    #[test]
    fn txt_content_is_the_value() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("line1\nline2".into()));

        let file_content = node_content(m.get_instance(&sv).unwrap());
        assert_eq!(file_content, "line1\nline2");
    }

    /// The Value lives in the .txt file, so it is NOT REPEATED in the meta file;
    /// keeping it in two places risks the two drifting apart.
    #[test]
    fn value_not_repeated_in_meta_file() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("hello".into()));

        // With only a Value there is no need for a meta file
        assert!(meta_file(&m, "src", &sv).is_none());

        // Adding an attribute creates a meta file, but it does not contain Value
        m.get_mut_instance(&sv)
            .unwrap()
            .attributes
            .insert("Dil".into(), PropertyValue::String("es".into()));
        assert!(meta_file(&m, "src", &sv).is_some());

        let file_content = meta_content(m.get_instance(&sv).unwrap());
        assert!(!file_content.contains("Value"), "Value must not be in meta: {}", file_content);
        assert!(file_content.contains("Dil"));
    }

    /// In scripts Source is a separate field, so properties stay in meta.
    #[test]
    fn script_properties_stay_in_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let file_content = meta_content(m.get_instance(&s).unwrap());
        assert!(file_content.contains("Disabled"));
    }

    /// ValueBase classes other than StringValue still use .json:
    /// turning a numeric value into plain text would lose its type.
    #[test]
    fn intvalue_stays_json() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let iv = add_instance(&mut m, "IntValue", "Sayac", Some(rs));

        let fs_path = data_file(&m, "src", &iv).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Sayac.intvalue.json"), "{:?}", fs_path);
    }

    #[test]
    fn object_extension_includes_class_name() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Box", Some(ws));
        let mdl = add_instance(&mut m, "Model", "House", Some(ws));
        add_instance(&mut m, "Part", "Roof", Some(mdl));

        let p_path = data_file(&m, "src", &p).unwrap();
        assert!(p_path.to_string_lossy().ends_with("Box.part.json"), "{:?}", p_path);

        let mdl_path = data_file(&m, "src", &mdl).unwrap();
        assert!(mdl_path.to_string_lossy().ends_with("init.model.json"), "{:?}", mdl_path);

        assert!(is_managed_file(Path::new("src/Box.part.json")));
        assert!(is_managed_file(Path::new("src/House/init.model.json")));
    }
}

#[cfg(test)]
mod protection_tests {
    use super::*;

    /// Files we do not recognise are NEVER touched.
    /// Regression test for README/note files placed in the sync folder being deleted.
    #[test]
    fn foreign_files_are_not_managed() {
        assert!(!is_managed_file(Path::new("src/NOTLAR.md")));
        assert!(!is_managed_file(Path::new("src/.gitkeep")));
        assert!(!is_managed_file(Path::new("src/resim.png")));
        assert!(!is_managed_file(Path::new("src/rapor.pdf")));
    }

    /// Formats we could produce ourselves are managed.
    #[test]
    fn our_formats_are_managed() {
        assert!(is_managed_file(Path::new("src/Box.json")));
        assert!(is_managed_file(Path::new("src/Main.server.lua")));
        assert!(is_managed_file(Path::new("src/Hud.client.lua")));
        assert!(is_managed_file(Path::new("src/Modul.lua")));
        assert!(is_managed_file(Path::new("src/Modul.luau")));
        assert!(is_managed_file(Path::new("src/Message.txt")));
        assert!(is_managed_file(Path::new("src/Main.meta.json")));
    }

    #[test]
    fn ignore_patterns_match() {
        let patterns = vec!["*.md".to_string(), "notlar/**".to_string()];
        assert!(is_ignored(Path::new("src/OKU.md"), "src", &patterns));
        assert!(is_ignored(Path::new("src/notlar/a/b.lua"), "src", &patterns));
        assert!(!is_ignored(Path::new("src/Box.json"), "src", &patterns));
    }

    #[test]
    fn empty_pattern_list_ignores_nothing() {
        assert!(!is_ignored(Path::new("src/OKU.md"), "src", &[]));
    }

    /// Windows backslashes must match too.
    #[test]
    fn backslashes_are_normalised() {
        let patterns = vec!["notlar/**".to_string()];
        let p = PathBuf::from("src").join("notlar").join("gizli.lua");
        assert!(is_ignored(&p, "src", &patterns));
    }

    /// A broken pattern must not cause a crash.
    #[test]
    fn broken_pattern_does_not_crash() {
        let patterns = vec!["[".to_string()];
        assert!(!is_ignored(Path::new("src/Box.json"), "src", &patterns));
    }
}

#[cfg(test)]
mod csv_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn localizationtable_is_written_to_csv_file() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));

        let fs_path = data_file(&m, "src", &lt).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Translations.csv"), "{:?}", fs_path);
    }

    #[test]
    fn csv_content_starts_with_header_row() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
        m.get_mut_instance(&lt).unwrap().properties.insert(
            "Contents".into(),
            PropertyValue::String(
                r#"[{"Key":"greeting","Source":"Hello","Context":"","Values":{"es":"Hola"}}]"#
                    .into(),
            ),
        );

        let file_content = node_content(m.get_instance(&lt).unwrap());
        let line_list: Vec<&str> = file_content.lines().collect();
        assert_eq!(line_list[0], "Key,Source,Context,Example,es");
        assert!(line_list[1].contains("Hola"), "{}", file_content);
    }

    /// Contents lives in the .csv file, so it is NOT REPEATED in meta.
    #[test]
    fn contents_not_repeated_in_meta_file() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
        {
            let n = m.get_mut_instance(&lt).unwrap();
            n.properties
                .insert("Contents".into(), PropertyValue::String("[]".into()));
        }
        // With only Contents there is no need for a meta file
        assert!(meta_file(&m, "src", &lt).is_none());

        m.get_mut_instance(&lt)
            .unwrap()
            .attributes
            .insert("Version".into(), PropertyValue::Number(1.0));
        let file_content = meta_content(m.get_instance(&lt).unwrap());
        assert!(!file_content.contains("Contents"), "Contents must not be in meta: {}", file_content);
    }

    /// Corrupt Contents must not cause a crash; an empty file should be written.
    #[test]
    fn broken_contents_does_not_crash() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
        m.get_mut_instance(&lt)
            .unwrap()
            .properties
            .insert("Contents".into(), PropertyValue::String("{broken".into()));

        let file_content = node_content(m.get_instance(&lt).unwrap());
        assert!(file_content.is_empty());
    }

    #[test]
    fn csv_is_managed_extension() {
        assert!(is_managed_file(Path::new("src/Translations.csv")));
    }
}

#[cfg(test)]
mod trash_tests {
    use super::*;
    use std::fs;

    /// Each test works in its own folder: the trash uses one run name per process,
    /// so tests sharing a directory would break each other.
    fn scratch_root(item_name: &str) -> PathBuf {
        let root_dir = std::env::temp_dir().join(format!("syncix-trash-{}", item_name));
        let _ = fs::remove_dir_all(&root_dir);
        fs::create_dir_all(root_dir.join("src")).unwrap();
        root_dir
    }

    /// From a real incident: the editor opened before Studio connected, the model was empty and
    /// the reconciler moved all 48 files in the sync folder to the trash. While the model
    /// is not authoritative, nothing may be removed from disk.
    #[test]
    fn empty_model_deletes_nothing_before_studio_syncs() {
        let root_dir = scratch_root("no-authority");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let game_file = sync.join("Workspace.json");
        let script_node = sync.join("ServerScriptService").join("Main.server.lua");
        fs::create_dir_all(script_node.parent().unwrap()).unwrap();
        fs::write(&game_file, "{}").unwrap();
        fs::write(&script_node, "print('game code')").unwrap();

        write_full_tree(&DataModel::new(), s, &[], false);

        assert!(game_file.exists(), "no file may be deleted while the model is not authoritative");
        assert!(script_node.exists(), "the script in the subfolder must stay too");
        assert!(trash_runs(s).is_empty(), "nothing may go to the trash");
    }

    /// With authority the old behaviour is unchanged: a file not in the model goes to the trash.
    #[test]
    fn authoritative_model_trashes_extras() {
        let root_dir = scratch_root("authority");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let extra = sync.join("Deleted.server.lua");
        fs::write(&extra, "-- no longer in Studio").unwrap();

        write_full_tree(&DataModel::new(), s, &[], true);

        assert!(!extra.exists(), "a file missing from the authoritative model must be removed");
        assert_eq!(trash_runs(s).len(), 1, "the removed file must be in the trash");
    }

    /// The point: when the reconciler deletes a file, its content must not be lost.
    #[test]
    fn deleted_file_stays_in_trash() {
        let root_dir = scratch_root("basic");
        let sync = root_dir.join("src");
        let file_path = sync.join("Important.server.lua");
        fs::write(&file_path, "print('must_not_be_lost')").unwrap();

        move_to_trash(&file_path, sync.to_str().unwrap()).unwrap();

        assert!(!file_path.exists(), "the file must be removed from the sync folder");
        let runs = trash_runs(sync.to_str().unwrap());
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].1, 1, "exactly one file must be in the trash");
    }

    #[test]
    fn restore_returns_content_unchanged() {
        let root_dir = scratch_root("restore");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Sub").join("Code.server.lua");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "-- original content").unwrap();

        move_to_trash(&file_path, s).unwrap();
        let run_name = trash_runs(s)[0].0.clone();
        let (restored_count, skipped) = restore_from_trash(s, &run_name);

        assert_eq!((restored_count, skipped), (1, 0));
        assert!(file_path.exists(), "the file must return to its old place, subfolder included");
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "-- original content");
    }

    /// Restoring must not cause data loss of its own: if a file exists at the same path
    /// it is not overwritten but skipped.
    #[test]
    fn restore_does_not_overwrite_existing() {
        let root_dir = scratch_root("overwrite");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Code.server.lua");

        fs::write(&file_path, "old").unwrap();
        move_to_trash(&file_path, s).unwrap();
        fs::write(&file_path, "new and valuable").unwrap();

        let run_name = trash_runs(s)[0].0.clone();
        let (restored_count, skipped) = restore_from_trash(s, &run_name);

        assert_eq!((restored_count, skipped), (0, 1));
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "new and valuable");
    }

    /// The trash must be OUTSIDE the sync folder; inside, the watcher would take it for
    /// new content and the reconciler would delete it again on the next pass.
    #[test]
    fn trash_is_outside_sync_folder() {
        let root_dir = scratch_root("location");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Code.server.lua");
        fs::write(&file_path, "x").unwrap();
        move_to_trash(&file_path, s).unwrap();

        let mut remaining_items = Vec::new();
        collect_files(&sync, &mut remaining_items);
        assert!(remaining_items.is_empty(), "no leftovers may remain in the sync folder");
        assert!(root_dir.join(".syncix").join("trash").exists());
    }
}

#[cfg(test)]
mod path_matching_tests {
    use super::*;
    use crate::model::{DataModel, InstanceNode};

    /// The watcher reports absolute paths, data_file produces relative paths.
    /// With plain equality a disk deletion never matched.
    #[test]
    fn absolute_path_matches_relative_expectation() {
        let mut dm = DataModel::new();
        let mut service_name = InstanceNode::new("ServerScriptService", "ServerScriptService");
        let service_id = service_name.syncix_id;
        service_name.parent = None;

        let mut script_node = InstanceNode::new("Script", "DiskDeleteTest");
        let script_id = script_node.syncix_id;
        script_node.parent = Some(service_id);
        service_name.children.push(script_id);

        dm.upsert_instance(service_name).unwrap();
        dm.upsert_instance(script_node).unwrap();

        let absolute = Path::new(r"C:\project\src_workspace\ServerScriptService\DiskDeleteTest.server.lua");
        assert_eq!(
            uuid_for_path(&dm, "src_workspace", absolute),
            Some(script_id)
        );
    }

    /// Deleting the meta file is not deleting the instance.
    #[test]
    fn deleting_meta_file_is_not_a_delete() {
        let mut dm = DataModel::new();
        let mut script_node = InstanceNode::new("Script", "Code");
        script_node.parent = None;
        dm.upsert_instance(script_node).unwrap();

        let meta = Path::new(r"C:\project\src_workspace\Code.meta.json");
        assert_eq!(uuid_for_path(&dm, "src_workspace", meta), None);
    }
}

#[cfg(test)]
mod suffix_tests {
    use super::*;

    /// The sync folder often arrives as "./src_workspace". The leading "."
    /// counts as a separate component, so a plain ends_with does not match and disk
    /// deletions were silently dropped.
    #[test]
    fn dot_slash_prefix_does_not_break_matching() {
        assert!(suffix_matches(
            Path::new(r"C:\project\src_workspace\SSS\Code.server.lua"),
            Path::new("./src_workspace/SSS/Code.server.lua"),
        ));
    }

    #[test]
    fn different_file_does_not_match() {
        assert!(!suffix_matches(
            Path::new(r"C:\project\src_workspace\SSS\Other.server.lua"),
            Path::new("./src_workspace/SSS/Code.server.lua"),
        ));
    }

    /// Matching only the file name is not enough; the folder path must match too.
    #[test]
    fn same_name_other_folder_does_not_match() {
        assert!(!suffix_matches(
            Path::new(r"C:\project\src_workspace\Other\Code.server.lua"),
            Path::new("./src_workspace/SSS/Code.server.lua"),
        ));
    }
}
