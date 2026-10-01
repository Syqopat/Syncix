//! Split out of layout.rs.

#[allow(unused_imports)]
use super::*;

/// <sync_dir>/../.syncix/trash
pub(crate) fn trash_root(sync_dir: &str) -> PathBuf {
    let s = Path::new(sync_dir);
    let upper = s.parent().unwrap_or(s);
    upper.join(".syncix").join("trash")
}

/// Keeps the newest TRASH_KEEP_DEFAULT folders; older ones are deleted completely.
pub(crate) const TRASH_KEEP_DEFAULT: usize = 10;

/// The run name is generated once and kept, so every deletion in the same core session
/// lands in a single folder.
pub(crate) fn run_label() -> String {
    static TRASH_RUN: OnceLock<String> = OnceLock::new();
    TRASH_RUN.get_or_init(|| chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string())
        .clone()
}

/// Moves a file to the trash instead of deleting it. The path relative to the sync folder
/// is kept, so restoring is a plain copy.
pub(crate) fn move_to_trash(path: &Path, sync_dir: &str) -> std::io::Result<()> {
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

/// Keeps a copy of the file in the trash and leaves the original where it is.
///
/// Used before Studio's version replaces a file that changed on disk while the core was
/// not running: that edit is nobody's to throw away, and until now it was overwritten
/// without a trace.
pub(crate) fn copy_to_trash(path: &Path, sync_dir: &str) -> std::io::Result<()> {
    if !trash_config().0 {
        // With the trash off there is nowhere to keep it. The caller reports the
        // overwrite either way.
        return Ok(());
    }
    let rel_path = path.strip_prefix(sync_dir).unwrap_or(path);
    let dest = trash_root(sync_dir).join(run_label()).join(rel_path);
    if let Some(upper) = dest.parent() {
        fs::create_dir_all(upper)?;
    }
    fs::copy(path, &dest)?;
    prune_trash(sync_dir);
    Ok(())
}

/// The trash must not grow without bound: the newest TRASH_KEEP_DEFAULT folders stay.
pub(crate) fn prune_trash(sync_dir: &str) {
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

/// One file in the trash.
#[derive(Debug, Clone, PartialEq)]
pub struct TrashEntry {
    /// Run folder name: a UTC timestamp, "%Y%m%d-%H%M%S".
    pub run: String,
    /// Path relative to the sync folder, '/'-separated.
    pub rel: String,
}

/// Picks single files out of the trash instead of a whole run, CoreProtect-style:
/// by name, by place in the tree, by class and by time. Unset fields match everything.
#[derive(Debug, Default, Clone)]
pub struct TrashFilter {
    /// Instance or folder name anywhere on the path, case-insensitive.
    pub name: Option<String>,
    /// Only files under this path of the tree, e.g. "Workspace/Zones".
    pub scope: Option<String>,
    /// Class as file names carry it: "part", "model", "script", "localscript", ...
    pub class_name: Option<String>,
    /// Only runs at or after this run-name timestamp (they sort as text).
    pub since: Option<String>,
}

/// Every file in the trash, newest run first.
pub fn trash_entries(sync_dir: &str) -> Vec<TrashEntry> {
    let root_dir = trash_root(sync_dir);
    let mut out = Vec::new();
    for (run, _) in trash_runs(sync_dir) {
        let origin = root_dir.join(&run);
        let mut file_list = Vec::new();
        collect_files(&origin, &mut file_list);
        file_list.sort();
        for f in file_list {
            if let Ok(rel_path) = f.strip_prefix(&origin) {
                let rel = rel_path
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(TrashEntry { run: run.clone(), rel });
            }
        }
    }
    out
}

/// A path component without its extensions: "Box_1a2b3c4d.part.json" -> "Box_1a2b3c4d".
pub(crate) fn base_of(component: &str) -> &str {
    component.split('.').next().unwrap_or(component)
}

/// Is this component exactly the one named? "Workspace/RampA" picks RampA.part.json,
/// not the duplicate RampA_149b6fa2.part.json.
pub(crate) fn component_at(component: &str, wanted: &str) -> bool {
    component.eq_ignore_ascii_case(wanted) || base_of(component).eq_ignore_ascii_case(wanted)
}

/// Does this component carry the instance name, duplicates included?
pub(crate) fn component_named(component: &str, name: &str) -> bool {
    component_at(component, name) || instance_name_of(component).eq_ignore_ascii_case(name)
}

/// The distinct instances a name picks, as paths `--in` accepts:
/// ["Workspace/RampA", "Workspace/RampA_149b6fa2"]. More than one means the name alone
/// is ambiguous.
pub fn instances_named(entries: &[TrashEntry], name: &str) -> Vec<String> {
    let mut out: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            let parts: Vec<&str> = e.rel.split('/').collect();
            let i = parts.iter().position(|p| component_named(p, name))?;
            let mut key: Vec<&str> = parts[..i].to_vec();
            key.push(base_of(parts[i]));
            Some(key.join("/"))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every instance name the trash holds, for "Did you mean" when a search finds nothing.
pub fn trash_names(entries: &[TrashEntry]) -> Vec<String> {
    let mut out: Vec<String> = entries
        .iter()
        .flat_map(|e| e.rel.split('/').map(instance_name_of))
        .filter(|name| !name.is_empty() && *name != "init")
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every class the trash's file names carry ("part", "script", ...).
pub fn trash_classes(entries: &[TrashEntry]) -> Vec<String> {
    let mut out: Vec<String> = entries
        .iter()
        .filter_map(|e| class_of_file(e.rel.rsplit('/').next().unwrap_or("")))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The instance name a path component stands for: "Box_1a2b3c4d.part.json" -> "Box".
pub(crate) fn instance_name_of(component: &str) -> &str {
    let base = base_of(component);
    match base.rsplit_once('_') {
        Some((name, suffix)) if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit()) => name,
        _ => base,
    }
}

/// The class a file name stands for, lower case: "Box.part.json" -> "part",
/// "Main.server.lua" -> "script". None for a file that names no class of its own
/// (a .meta.json, or a legacy plain ".json").
pub(crate) fn class_of_file(file_name: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".server.lua") {
        return Some("script".into());
    }
    if lower.ends_with(".client.lua") {
        return Some("localscript".into());
    }
    if lower.ends_with(".lua") || lower.ends_with(".luau") {
        return Some("modulescript".into());
    }
    if lower.ends_with(".txt") {
        return Some("stringvalue".into());
    }
    if lower.ends_with(".csv") {
        return Some("localizationtable".into());
    }
    if lower.ends_with(".meta.json") {
        return None;
    }
    let stem = lower.strip_suffix(".json")?;
    stem.rsplit_once('.').map(|(_, class)| class.to_string())
}

/// Applies a filter and keeps the newest copy of every path. A .meta.json follows its
/// instance: it is picked whenever the data file beside it is.
pub fn select_entries(entries: &[TrashEntry], filter: &TrashFilter) -> Vec<TrashEntry> {
    let matches = |e: &TrashEntry| -> bool {
        let parts: Vec<&str> = e.rel.split('/').collect();
        let file_name = parts.last().copied().unwrap_or("");
        if let Some(since) = &filter.since {
            if e.run.as_str() < since.as_str() {
                return false;
            }
        }
        if let Some(scope) = &filter.scope {
            let wanted: Vec<&str> = scope.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
            if wanted.len() > parts.len() || !wanted.iter().zip(&parts).all(|(w, p)| component_at(p, w)) {
                return false;
            }
        }
        if let Some(name) = &filter.name {
            if !parts.iter().any(|p| component_named(p, name)) {
                return false;
            }
        }
        if let Some(class) = &filter.class_name {
            if class_of_file(file_name).as_deref() != Some(class.to_ascii_lowercase().as_str()) {
                return false;
            }
        }
        true
    };

    let mut seen = std::collections::HashSet::new();
    let mut picked: Vec<TrashEntry> = Vec::new();
    for e in entries.iter().filter(|e| !e.rel.ends_with(".meta.json")) {
        if matches(e) && seen.insert(e.rel.clone()) {
            picked.push(e.clone());
        }
    }

    // Companion metadata: same run, same folder, same instance name.
    let key = |rel: &str| -> (String, String) {
        let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
        (dir.to_string(), instance_name_of(file).to_string())
    };
    let wanted_meta: std::collections::HashSet<(String, String, String)> = picked
        .iter()
        .map(|e| {
            let (dir, name) = key(&e.rel);
            (e.run.clone(), dir, name)
        })
        .collect();
    for e in entries.iter().filter(|e| e.rel.ends_with(".meta.json")) {
        let (dir, name) = key(&e.rel);
        if wanted_meta.contains(&(e.run.clone(), dir, name)) && seen.insert(e.rel.clone()) {
            picked.push(e.clone());
        }
    }
    picked
}

/// Restores the given files. Like a whole-run restore it never overwrites a file.
/// Returns: (restored, skipped because a file already exists there).
pub fn restore_entries(sync_dir: &str, entries: &[TrashEntry]) -> (usize, usize) {
    let root_dir = trash_root(sync_dir);
    let (mut restored_count, mut skipped) = (0usize, 0usize);
    for e in entries {
        let origin = root_dir.join(&e.run).join(&e.rel);
        let dest = Path::new(sync_dir).join(&e.rel);
        if dest.exists() {
            skipped += 1;
            continue;
        }
        if let Some(upper) = dest.parent() {
            let _ = fs::create_dir_all(upper);
        }
        if fs::copy(&origin, &dest).is_ok() {
            restored_count += 1;
        }
    }
    (restored_count, skipped)
}
