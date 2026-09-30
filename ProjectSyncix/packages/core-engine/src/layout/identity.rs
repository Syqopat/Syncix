//! Split out of layout.rs.

#[allow(unused_imports)]
use super::*;

/// One spelling per file, so the write and delete logs match the watcher's events.
///
/// The writer records paths under the sync folder as configured ("./src/X") while the
/// watcher reports them absolute ("C:\...\Game\./src\X"). Compared as they were, no write
/// was ever recognised as ours. Harmless while the files already matched the model; a
/// first write into an empty folder was taken for the user creating, renaming and
/// deleting scripts, and all of it was sent to Studio (a Team Create place's PlayerModule
/// was scrambled that way).
pub fn normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    // Windows paths ignore case; the watcher may spell a folder as the disk has it.
    if cfg!(windows) {
        PathBuf::from(out.to_string_lossy().to_lowercase())
    } else {
        out
    }
}

/// Writes a file the core repaired itself (an identity put back into a data file) and
/// records it, so the watcher does not read the repair back as a user's change.
pub fn write_own(path: &Path, file_content: &str) -> std::io::Result<()> {
    record_write(path, file_content);
    fs::write(path, file_content)
}

/// Renames a file the core repaired itself (a damaged identity in a file name). Both the
/// file that goes and the one that appears are recorded, so neither comes back as a
/// user's deletion or a new object.
pub fn rename_own(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Ok(content) = fs::read_to_string(from) {
        record_write(to, &content);
    }
    record_delete(from);
    let result = fs::rename(from, to);
    if result.is_err() {
        forget_write(to);
    }
    result
}

/// The instance name a data file or folder name stands for: "Box_1a2b3c4d.part.json" -> "Box".
pub fn instance_name_in(component: &str) -> String {
    instance_name_of(component).to_string()
}

/// The identity a data file or folder name carries: "Box_1a2b3c4d.part.json" -> "1a2b3c4d".
/// None when the name carries none (the usual case: only duplicate names need one).
pub fn short_id_in(component: &str) -> Option<String> {
    let base = base_of(component);
    match base.rsplit_once('_') {
        Some((_, suffix)) if suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit()) => {
            Some(suffix.to_string())
        }
        _ => None,
    }
}

/// The class a data file's name stands for, as Studio spells it: "Box.part.json" -> "Part".
/// None for a name that carries no class (init.meta.json, a legacy plain .json).
pub fn class_in_file_name(file_name: &str) -> Option<&'static str> {
    crate::catalog::class_named(&class_of_file(file_name)?)
}

/// Where the tree says this instance's data file belongs.
pub fn data_file_path(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    data_file(dm, sync_dir, uuid)
}

/// The instance whose folder `dir` is: its data file is `dir/init.*` (a script with
/// children, a folder, a service). None when no instance keeps its data file there.
///
/// Looking the folder up by name picked whichever instance of that name came first, and a
/// place with two PlayerModules got scripts created and renamed under the wrong one.
pub fn uuid_for_dir(dm: &DataModel, sync_dir: &str, dir: &Path) -> Option<Uuid> {
    let wanted = normalize(dir);
    dm.get_all_instances().iter().find_map(|(uuid, _)| {
        let file = data_file(dm, sync_dir, uuid)?;
        let is_init = file.file_name()?.to_str()?.starts_with("init.");
        (is_init && normalize(file.parent()?) == wanted).then_some(*uuid)
    })
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
pub(crate) fn suffix_matches(fs_path: &Path, expected_value: &Path) -> bool {
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

pub(crate) fn sanitize(name: &str) -> String {
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

pub(crate) fn seg(dm: &DataModel, node: &InstanceNode) -> String {
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
