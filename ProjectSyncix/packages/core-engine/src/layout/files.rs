//! Split out of layout.rs.

#[allow(unused_imports)]
use super::*;

/// The class a source file's name stands for: "Spin.server.lua" -> "Script". The other
/// way round from script_ext, and the one place that mapping is written down, so the
/// watcher and `syncix import` cannot drift apart.
pub fn script_class_of_file(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".server.lua") || lower.ends_with(".server.luau") {
        Some("Script")
    } else if lower.ends_with(".client.lua") || lower.ends_with(".client.luau") {
        Some("LocalScript")
    } else if lower.ends_with(".lua") || lower.ends_with(".luau") {
        Some("ModuleScript")
    } else {
        None
    }
}

pub(crate) fn script_ext(class_name: &str) -> Option<&'static str> {
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

pub(crate) fn is_script(node: &InstanceNode) -> bool {
    matches!(
        node.class_name.as_str(),
        "Script" | "LocalScript" | "ModuleScript"
    )
}

pub(crate) fn instance_ext(class_name: &str) -> String {
    if let Some(ext) = script_ext(class_name) {
        ext.to_string()
    } else {
        format!("{}.json", class_name.to_ascii_lowercase())
    }
}

/// Is this class written to disk as RAW CONTENT (instead of its own .json)?
/// Such classes keep their properties/attributes in a .meta.json file.
pub(crate) fn with_raw_content(node: &InstanceNode) -> bool {
    script_ext(&node.class_name).is_some()
}

/// Raw text written to disk for classes like StringValue.
pub(crate) fn raw_str(node: &InstanceNode) -> String {
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

pub(crate) fn ancestry(dm: &DataModel, uuid: &Uuid) -> Vec<Uuid> {
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
pub(crate) fn meta_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
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

pub(crate) fn meta_content(node: &InstanceNode) -> String {
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

pub(crate) fn node_content(node: &InstanceNode) -> String {
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
pub(crate) fn is_managed_file(path: &Path) -> bool {
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
pub(crate) fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
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
pub(crate) fn remove_empty_dirs(dir: &Path, root: &Path) {
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
