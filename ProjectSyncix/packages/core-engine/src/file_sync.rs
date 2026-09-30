use crate::model::InstanceNode;
use crate::transport::{EventType, Payload, StudioOutbox};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::serializers::part::PartSerializer;
use crate::serializers::Serializer;

/// Watches changes on disk and forwards them to Studio.
/// IMPORTANT: every value sent to Studio goes through `pv_to_wire`.
/// Putting a `PropertyValue` straight into JSON produces serde's externally tagged
/// form ({"Number":0.5}); the plugin expects the plain form and rejects it with
/// "unsupported table value for property". This bug was caught in live use
/// on Part.Transparency.
///
/// IMPORTANT: this module NEVER writes to disk. Writing is the layout module's job alone;
/// otherwise the two sides would apply different naming rules and overwrite each other's files.
pub async fn start_watcher(
    tx_to_studio: Arc<StudioOutbox>,
    path: &str,
    data_model: crate::model::SharedDataModel,
    tx_to_vscode: tokio::sync::broadcast::Sender<String>,
    disk_notify: Arc<tokio::sync::Notify>,
    ignore: Vec<String>,
    settings_data: crate::project::ProjectConfig,
) {
    // If the disk -> Studio direction is off, the watcher is not started at all.
    // Silencing only the sending would not be enough: a change on disk would still be
    // applied to the model and leak to Studio on the next write.
    configure_delete_grace(settings_data.safety_settings.delete_grace_ms);

    if !settings_data.mode_value.accepts_from_disk() {
        info!(
            "Sync mode is '{}': the file watcher was not started, disk changes are ignored.",
            settings_data.mode_value.name_of()
        );
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).unwrap();

    watcher
        .watch(Path::new(path), RecursiveMode::Recursive)
        .unwrap();
    info!("File system watcher started: {}", path);

    let serializer = PartSerializer;
    let sync_dir_owner = path.to_string();

    tokio::task::spawn_blocking(move || {
        let _watcher = watcher;
        // Deletions are NOT applied immediately. Reason: moving a file to another folder
        // shows up in the operating system as "delete + create". Deleting right away would
        // turn a move into destroying the instance and creating one with a new identity;
        // its properties and subtree would be lost.
        // Instead a deletion waits for the delete grace period; if the same uuid
        // reappears within that time it was a move, and
        // the deletion is cancelled.
        let mut pending_deletes: std::collections::HashMap<Uuid, std::time::Instant> =
            std::collections::HashMap::new();

        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    handle_event(
                        event,
                        &tx_to_studio,
                        &serializer,
                        &data_model,
                        &tx_to_vscode,
                        &disk_notify,
                        &sync_dir_owner,
                        &ignore,
                        &mut pending_deletes,
                    );
                }
                Ok(Err(e)) => error!("Watch error: {:?}", e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            apply_ripe_deletes(
                &mut pending_deletes,
                &tx_to_studio,
                &data_model,
                &tx_to_vscode,
                &disk_notify,
            );
        }
        error!("File watcher loop ended unexpectedly.");
    });
}

/// Actually applies the deletions whose grace period has expired.
///
/// The wait keeps a move from being mistaken for a deletion: the operating system
/// reports a move as "delete + create". If the file comes back within this time,
/// the deletion has been cancelled.
static DELETE_GRACE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(800);

pub fn configure_delete_grace(ms: u64) {
    DELETE_GRACE_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
}

fn delete_grace_period() -> std::time::Duration {
    std::time::Duration::from_millis(DELETE_GRACE_MS.load(std::sync::atomic::Ordering::Relaxed))
}

fn apply_ripe_deletes(
    pending_item: &mut std::collections::HashMap<Uuid, std::time::Instant>,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    disk_notify: &Arc<tokio::sync::Notify>,
) {
    if pending_item.is_empty() {
        return;
    }
    let current_time = std::time::Instant::now();
    let ready_deletes: Vec<Uuid> = pending_item
        .iter()
        .filter(|(_, t)| current_time.duration_since(**t) >= delete_grace_period())
        .map(|(u, _)| *u)
        .collect();

    for uuid in ready_deletes {
        pending_item.remove(&uuid);

        let removed_node = {
            let mut dm = data_model.blocking_write();
            dm.remove_instance(&uuid)
        };
        let Some(instance) = removed_node else {
            continue;
        };

        info!(
            "The file for '{}' was deleted from disk; the instance is being removed.",
            instance.name
        );

        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "DESTROY",
                    "data": { "syncix_id": uuid }
                }]
            }),
        });

        let msg = serde_json::json!({
            "event_type": "INSTANCE_REMOVED",
            "data": {
                "id": uuid,
                "parentId": instance.parent.map(|u| u.to_string())
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());

        // The subtree left the model too; let the disk writer rewrite the tree.
        disk_notify.notify_one();
    }
}

/// Reports the current state of an instance to the VS Code Explorer.
fn notify_vscode_updated(
    dm: &crate::model::DataModel,
    uuid: &Uuid,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
) {
    if let Some(inst) = dm.get_instance(uuid) {
        let msg = serde_json::json!({
            "event_type": event_type,
            "data": {
                "id": inst.syncix_id,
                "name": inst.name,
                "className": inst.class_name,
                "parentId": inst.parent.map(|u| u.to_string()),
                "childrenIds": inst.children,
                "isExpanded": false
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());
    }
}

fn parse_script_filename(path: &Path) -> Option<(String, Option<String>, String)> {
    let fname = path.file_name()?.to_str()?;
    // Order matters: ".server.lua" also ends with ".lua", so the longer
    // suffixes must be tried first.
    let (base, ext) = if let Some(b) = fname.strip_suffix(".server.lua") {
        (b, "server.lua")
    } else if let Some(b) = fname.strip_suffix(".client.lua") {
        (b, "client.lua")
    } else if let Some(b) = fname.strip_suffix(".luau") {
        (b, "luau")
    } else {
        (fname.strip_suffix(".lua")?, "lua")
    };

    // Container script: the folder name is the object's name (init.server.lua, etc.)
    if base == "init" {
        let parent_dir = path.parent()?.file_name()?.to_str()?;
        if let Some((dir_name, dir_uuid)) = parent_dir.rsplit_once('_') {
            if is_short_uuid(dir_uuid) {
                return Some((dir_name.to_string(), Some(dir_uuid.to_string()), ext.to_string()));
            }
        }
        return Some((parent_dir.to_string(), None, ext.to_string()));
    }

    // Leaf script: only the 8-character short UUID suffix generated by layout is recognised.
    // A numeric suffix ("Health_2") is NO LONGER parsed; layout never produces such a name, and
    // parsing it broke the name of a real object called "Health_2".
    if let Some((name_part, potential_suffix)) = base.rsplit_once('_') {
        if is_short_uuid(potential_suffix) {
            return Some((name_part.to_string(), Some(potential_suffix.to_string()), ext.to_string()));
        }
    }

    Some((base.to_string(), None, ext.to_string()))
}

/// Is this the short UUID suffix generated by layout? (exactly 8 characters, all hex)
fn is_short_uuid(s: &str) -> bool {
    s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_script_class(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

/// Finds the script node for a `Name.meta.json` or `init.meta.json` file.
///
/// The rule is the same as in layout.rs:
///
/// - `init.meta.json`  -> the node is the CONTAINING FOLDER itself (container script)
/// - `Name.meta.json`  -> the node is the folder's child called `Name`
///
/// On a name clash layout appends an 8-character short UUID to the file name; that suffix
/// is parsed here too, otherwise two scripts with the same name would be mixed up.
fn meta_target(dm: &crate::model::DataModel, path: &Path) -> Option<uuid::Uuid> {
    let fname = path.file_name()?.to_str()?;
    let base = fname.strip_suffix(".meta.json")?;
    let dir_name = path.parent()?.file_name()?.to_str()?;

    let divide = |s: &str| -> (String, String) {
        match s.rsplit_once('_') {
            Some((n, k)) if is_short_uuid(k) => (n.to_string(), k.to_string()),
            _ => (s.to_string(), String::new()),
        }
    };

    let (dir_clean, dir_short) = divide(dir_name);

    let dir_node = dm.get_all_instances().iter().find_map(|(u, n)| {
        if n.name == dir_clean && (dir_short.is_empty() || u.to_string().starts_with(&dir_short)) {
            Some(*u)
        } else {
            None
        }
    });

    if base == "init" {
        // Container script: the folder itself must be a script node.
        return dir_node.filter(|u| {
            dm.get_instance(u)
                .map(|n| is_script_class(&n.class_name))
                .unwrap_or(false)
        });
    }

    let (base_clean, base_short) = divide(base);
    let parent_ref = dir_node?;
    let parent_entry = dm.get_instance(&parent_ref)?;

    parent_entry.children.iter().find_map(|cid| {
        let c = dm.get_instance(cid)?;
        if c.name == base_clean
            && is_script_class(&c.class_name)
            && (base_short.is_empty() || cid.to_string().starts_with(&base_short))
        {
            Some(*cid)
        } else {
            None
        }
    })
}

/// Applies a `Name.txt` file to the Value of the matching StringValue.
///
/// A StringValue is written to disk as plain text (see layout::script_ext), so
/// the file's content is the Value itself. That lets the text be opened and edited
/// in the editor without dealing with JSON escape characters.
fn handle_txt_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(file_content) = fs::read_to_string(path) else {
        return;
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "txt", "StringValue")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the txt file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(file_content.clone());
                if node.properties.get("Value") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Value".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Value", "value": file_content }
                }]
            }),
        });
        debug!("Updated Value from the txt file: {:?}", path);
    }
}

/// Applies a `Name.csv` file to the Contents of the matching LocalizationTable.
///
/// Translations are written to disk as CSV so they can be edited as a table; on the
/// Roblox side the counterpart is a JSON string called Contents. The conversion is lossless (localization.rs).
fn handle_csv_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(csv) = fs::read_to_string(path) else {
        return;
    };

    let json = match crate::localization::csv_to_json(&csv) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Could not parse CSV ({:?}): {}", path, e);
            return;
        }
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "csv", "LocalizationTable")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the csv file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(json.clone());
                if node.properties.get("Contents") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Contents".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Contents", "value": json }
                }]
            }),
        });
        debug!("Updated Contents from the csv file: {:?}", path);
    }
}

/// Finds the node that owns a raw-content file (such as .txt).
/// Uses the same naming rules as meta_target.
fn raw_file_target(
    dm: &crate::model::DataModel,
    path: &Path,
    file_ext: &str,
    class_str: &str,
) -> Option<uuid::Uuid> {
    let fname = path.file_name()?.to_str()?;
    let base = fname.strip_suffix(&format!(".{}", file_ext))?;
    let dir_name = path.parent()?.file_name()?.to_str()?;

    let divide = |s: &str| -> (String, String) {
        match s.rsplit_once('_') {
            Some((n, k)) if is_short_uuid(k) => (n.to_string(), k.to_string()),
            _ => (s.to_string(), String::new()),
        }
    };

    let (dir_clean, dir_short) = divide(dir_name);
    let dir_node = dm.get_all_instances().iter().find_map(|(u, n)| {
        if n.name == dir_clean && (dir_short.is_empty() || u.to_string().starts_with(&dir_short)) {
            Some(*u)
        } else {
            None
        }
    });

    if base == "init" {
        return dir_node.filter(|u| {
            dm.get_instance(u).map(|n| n.class_name == class_str).unwrap_or(false)
        });
    }

    let (base_clean, base_short) = divide(base);
    let parent_ref = dir_node?;
    let parent_entry = dm.get_instance(&parent_ref)?;

    parent_entry.children.iter().find_map(|cid| {
        let c = dm.get_instance(cid)?;
        if c.name == base_clean
            && c.class_name == class_str
            && (base_short.is_empty() || cid.to_string().starts_with(&base_short))
        {
            Some(*cid)
        } else {
            None
        }
    })
}

/// Applies a .meta.json change on disk to the model and to Studio.
fn handle_meta_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let meta: crate::layout::ScriptMeta = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Could not read meta file ({:?}): {}", path, e);
            return true; // recognised file with broken content; must not fall through to the json branch
        }
    };

    let uuid = {
        let dm = data_model.blocking_read();
        meta_target(&dm, path)
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the meta file: {:?}", path);
        return true;
    };

    // Only values that REALLY changed are sent; otherwise every disk write
    // would flood Studio with needless patches.
    let mut patches = Vec::new();
    {
        let mut dm = data_model.blocking_write();
        let Some(node) = dm.get_mut_instance(&uuid) else {
            return true;
        };

        for (item_name, raw_value) in &meta.properties {
            if node.properties.get(item_name) != Some(raw_value) {
                node.properties.insert(item_name.clone(), raw_value.clone());
                patches.push(serde_json::json!({
                    "event_type": "PROPERTY_UPDATE",
                    "data": {
                        "syncix_id": uuid,
                        "property": item_name,
                        "value": crate::pv_to_wire(raw_value)
                    }
                }));
            }
        }
        for (item_name, raw_value) in &meta.attributes {
            if node.attributes.get(item_name) != Some(raw_value) {
                node.attributes.insert(item_name.clone(), raw_value.clone());
                patches.push(serde_json::json!({
                    "event_type": "ATTRIBUTE_UPDATE",
                    "data": {
                        "syncix_id": uuid,
                        "name": item_name,
                        "value": crate::pv_to_wire(raw_value)
                    }
                }));
            }
        }

        // Tags are compared as a list, not one by one like attributes,
        // because a tag is present-or-absent information: a tag removed from the file really
        // was deleted. That is also why tags are handled separately from properties.
        {
            let mut in_file = meta.tags.clone();
            in_file.sort();
            in_file.dedup();
            if node.tags != in_file {
                node.tags = in_file.clone();
                patches.push(serde_json::json!({
                    "event_type": "TAGS_UPDATE",
                    "data": { "syncix_id": uuid, "tags": in_file }
                }));
            }
        }

        // PROPERTIES and ATTRIBUTES deliberately behave DIFFERENTLY here.
        //
        // An attribute is an extra field the user added; it can be deleted.
        // A property always has a value in Roblox: Anchored cannot be "deleted",
        // it can only be true or false. So removing a property from the meta file
        // does NOT mean "delete it in Studio"; at most it means "Syncix should stop
        // recording it". Studio keeps observing that property, so its
        // value comes back on the next sync.
        //
        // So nothing is done when a property is missing, but it is logged so the user's
        // expectation is not silently ignored.
        let missing_properties: Vec<&String> = node
            .properties
            .keys()
            .filter(|k| !meta.properties.contains_key(*k))
            .collect();
        if !missing_properties.is_empty() {
            tracing::info!(
                "Properties removed from the meta file were ignored ({:?}): {:?}. \
                 Properties cannot be deleted in Roblox; write the new value instead.",
                path.file_name().unwrap_or_default(),
                missing_properties
            );
        }

        // An attribute that is NO LONGER in the file has been deleted.
        //
        // This is only safe thanks to the write-log protection: we never read back a file
        // we wrote ourselves, so whatever is "missing" is something the user actually
        // deleted, not an old version from a delayed event.
        let delete_list: Vec<String> = node
            .attributes
            .keys()
            .filter(|k| !meta.attributes.contains_key(*k))
            .cloned()
            .collect();
        for item_name in delete_list {
            node.attributes.remove(&item_name);
            patches.push(serde_json::json!({
                "event_type": "ATTRIBUTE_UPDATE",
                "data": { "syncix_id": uuid, "name": item_name, "value": serde_json::Value::Null }
            }));
        }
    }

    if !patches.is_empty() {
        let number_value = patches.len();
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": patches }),
        });
        debug!("Sent {} change(s) from the meta file to Studio", number_value);
    }
    true
}

// Most arguments are independent channels (Studio queue, model, editor broadcast, disk
// notification) and all of them are needed to handle a single event. Bundling them
// into one struct would force every link in the call chain to carry it;
// it would not make the code easier to read.
#[allow(clippy::too_many_arguments)]
/// Applies a data file (`Box.part.json`, `init.model.json`) to the model and to Studio.
///
/// The file's NAME is the object's name, so renaming the file in the editor renames the
/// object in Studio — the editor was otherwise a read-only view of the tree and every
/// rename had to go through the CLI. The identity is the `syncix_id` INSIDE the file, so
/// a name may be changed freely; the identity in the file name (only duplicate names
/// carry one) is repaired from it, and a file written by hand gets a real identity, its
/// class from its name and its parent from its folder.
fn handle_instance_file(
    path: &Path,
    content: &str,
    old_path: Option<&Path>,
    tx_to_studio: &StudioOutbox,
    serializer: &PartSerializer,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    sync_dir: &str,
) {
    let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else {
        return;
    };
    // An init file belongs to its FOLDER, so the folder's name is the object's name.
    let named_by = if file_name.starts_with("init.") {
        path.parent().and_then(|d| d.file_name()).and_then(|f| f.to_str()).unwrap_or(file_name)
    } else {
        file_name
    };
    let wanted_name = crate::layout::instance_name_in(named_by);

    let value: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("{} could not be read as JSON, so it was left alone: {}", path.display(), e);
            return;
        }
    };
    let id_in_file = value
        .get("syncix_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .filter(|u| !u.is_nil());

    let (target, folder_owner) = {
        let dm = data_model.blocking_read();
        let target = id_in_file
            .filter(|u| dm.get_instance(u).is_some())
            .or_else(|| {
                crate::layout::short_id_in(named_by).and_then(|s| dm.find_by_short_uuid(&s))
            })
            // A file renamed by hand no longer sits where the tree says; the name it had
            // a moment ago still does.
            .or_else(|| old_path.and_then(|p| crate::layout::uuid_for_path(&dm, sync_dir, p)))
            .or_else(|| crate::layout::uuid_for_path(&dm, sync_dir, path));
        let owner = path.parent().and_then(|d| crate::layout::uuid_for_dir(&dm, sync_dir, d));
        (target, owner)
    };

    match target {
        Some(uuid) => update_instance_file(
            path, content, &wanted_name, uuid, id_in_file, tx_to_studio, serializer, data_model,
            tx_to_vscode, sync_dir,
        ),
        None => create_from_file(
            path, content, file_name, &wanted_name, &value, id_in_file, folder_owner,
            tx_to_studio, serializer, data_model,
        ),
    }
}

/// The file describes an object the model knows: its properties and its name are applied.
fn update_instance_file(
    path: &Path,
    content: &str,
    wanted_name: &str,
    uuid: Uuid,
    id_in_file: Option<Uuid>,
    tx_to_studio: &StudioOutbox,
    serializer: &PartSerializer,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    sync_dir: &str,
) {
    // A file whose identity was damaged by hand no longer reads as an object. The name and
    // the identity are still put right; only the properties cannot be compared this time.
    let mut node = serializer.deserialize(content).ok();
    if let Some(node) = node.as_mut() {
        node.syncix_id = uuid;
        node.name = wanted_name.to_string();
    }

    let (old_name, changed) = {
        let dm = data_model.blocking_read();
        let Some(existing) = dm.get_instance(&uuid) else {
            return;
        };
        let changed = match node.as_mut() {
            Some(node) => {
                // Where an object sits is the tree's business, not a hand-edited file's.
                node.parent = existing.parent;
                node.children = existing.children.clone();
                node.diff(existing)
            }
            None => None,
        };
        (existing.name.clone(), changed)
    };

    let mut patches = Vec::new();
    if let Some(patch) = changed {
        for (property, value) in patch.changed_properties {
            // The name travels as a rename below, which is what Studio's undo and the
            // echo guard understand; sending it as a property too applied it twice.
            if property == "Name" {
                continue;
            }
            patches.push(serde_json::json!({
                "event_type": "PROPERTY_UPDATE",
                "data": { "syncix_id": uuid, "property": property, "value": crate::pv_to_wire(&value) }
            }));
        }
    }
    let renamed = old_name != wanted_name;
    if renamed {
        patches.push(serde_json::json!({
            "event_type": "RENAME_INSTANCE",
            "data": { "id": uuid, "newName": wanted_name }
        }));
    }
    if !patches.is_empty() {
        let count = patches.len();
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": patches }),
        });
        debug!("The file changed -> Studio: {} patch(es) for {}", count, wanted_name);
    }

    match node {
        Some(mut node) => {
            node.last_updated = chrono::Utc::now().timestamp_millis();
            let mut dm = data_model.blocking_write();
            let _ = dm.upsert_instance(node);
        }
        None if renamed => {
            let mut dm = data_model.blocking_write();
            if let Some(existing) = dm.get_mut_instance(&uuid) {
                existing.name = wanted_name.to_string();
                existing.last_updated = chrono::Utc::now().timestamp_millis();
            }
        }
        None => {}
    }
    if renamed {
        info!("The file was renamed; so was the object: {} -> {}", old_name, wanted_name);
        let dm = data_model.blocking_read();
        notify_vscode_updated(&dm, &uuid, tx_to_vscode, "INSTANCE_UPDATED");
    }
    repair_data_file(path, content, uuid, id_in_file, data_model, sync_dir);
}

/// Puts back what a hand edit broke: the identity inside the file, and the identity in the
/// file's name. Only those; the rest of the file is the user's.
fn repair_data_file(
    path: &Path,
    content: &str,
    uuid: Uuid,
    id_in_file: Option<Uuid>,
    data_model: &crate::model::SharedDataModel,
    sync_dir: &str,
) {
    if id_in_file != Some(uuid) {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(content) {
            value["syncix_id"] = serde_json::json!(uuid.to_string());
            if let Ok(text) = serde_json::to_string_pretty(&value) {
                if crate::layout::write_own(path, &format!("{}\n", text)).is_ok() {
                    info!("The identity was put back into {}", path.display());
                }
            }
        }
    }

    // Only duplicate names carry an identity in the file name, and a damaged one would
    // send the file to the trash on the next full sync.
    let wanted_file = {
        let dm = data_model.blocking_read();
        crate::layout::data_file_path(&dm, sync_dir, &uuid)
    };
    let Some(wanted_name) = wanted_file.as_ref().and_then(|p| p.file_name()) else {
        return;
    };
    if path.file_name() == Some(wanted_name) {
        return;
    }
    let Some(target) = path.parent().map(|d| d.join(wanted_name)) else {
        return;
    };
    if target.exists() {
        return;
    }
    if crate::layout::rename_own(path, &target).is_ok() {
        info!("The identity in the file name was put back: {}", target.display());
    }
}

/// A data file written by hand becomes a real object: a fresh identity, the class from the
/// file's name, the parent from its folder, and the identity written back into the file.
fn create_from_file(
    path: &Path,
    content: &str,
    file_name: &str,
    wanted_name: &str,
    value: &serde_json::Value,
    id_in_file: Option<Uuid>,
    folder_owner: Option<Uuid>,
    tx_to_studio: &StudioOutbox,
    serializer: &PartSerializer,
    data_model: &crate::model::SharedDataModel,
) {
    let class = value
        .get("class_name")
        .and_then(|v| v.as_str())
        .and_then(crate::catalog::class_named)
        .or_else(|| crate::layout::class_in_file_name(file_name));
    let Some(class) = class else {
        tracing::warn!(
            "{}: which class is this? Name the file after it (Wall.part.json) or write \
             \"class_name\" inside, and the object is created in Studio.",
            path.display()
        );
        return;
    };
    let Some(parent) = folder_owner else {
        tracing::warn!(
            "{}: the folder it sits in belongs to no object, so there is nowhere to create it.",
            path.display()
        );
        return;
    };

    let mut node = serializer
        .deserialize(content)
        .unwrap_or_else(|_| crate::model::InstanceNode::new(class, wanted_name));
    node.syncix_id = id_in_file.unwrap_or_else(Uuid::new_v4);
    node.class_name = class.to_string();
    node.name = wanted_name.to_string();
    node.parent = Some(parent);
    node.children.clear();
    node.last_updated = chrono::Utc::now().timestamp_millis();

    {
        let mut dm = data_model.blocking_write();
        if let Err(e) = dm.upsert_instance(node.clone()) {
            tracing::warn!("{} could not be added: {}", path.display(), e);
            return;
        }
    }
    tx_to_studio.push(Payload {
        version: "v1".to_string(),
        event_type: EventType::PushUpdate,
        data: serde_json::to_value(&node).unwrap_or(serde_json::Value::Null),
    });
    info!("A file was written by hand; the object was created: {} ({})", node.name, class);

    // Written back so the file carries its identity: without it a second hand-written
    // file would claim the same (empty) one.
    if let Ok(text) = serializer.serialize(&node) {
        let _ = crate::layout::write_own(path, &format!("{}\n", text));
    }
}

fn handle_event(
    event: Event,
    tx_to_studio: &StudioOutbox,
    serializer: &PartSerializer,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    disk_notify: &Arc<tokio::sync::Notify>,
    sync_dir: &str,
    ignore: &[String],
    pending_deletes: &mut std::collections::HashMap<Uuid, std::time::Instant>,
) {
    // While suspended nothing is read from disk and nothing is sent to Studio.
    // This was the real danger: a file not in the model was taken for a "new object" and
    // created in Studio; with the wrong place bound, that meant the old game spilling
    // into the new place.
    if crate::project::is_sync_suspended() {
        return;
    }

    // Deletions used to be ignored entirely, because the disk writer deletes
    // files while rewriting the tree and those produced false DESTROYs.
    // Now the writer's own deletions are recorded, so they can be told apart:
    // a deletion that is not in the record is a real user deletion.
    if matches!(event.kind, EventKind::Remove(_)) {
        for path in &event.paths {
            if crate::layout::is_ignored(path, sync_dir, ignore) {
                continue;
            }
            if crate::layout::is_own_delete(path) {
                continue;
            }
            let uuid = {
                let dm = data_model.blocking_read();
                crate::layout::uuid_for_path(&dm, sync_dir, path)
            };
            match uuid {
                Some(u) => {
                    info!("A file was deleted from disk: {}", path.display());
                    pending_deletes.insert(u, std::time::Instant::now());
                }
                // Silently dropped deletes could not be traced: when fs_path matching
                // was broken, no trace was left.
                None => info!(
                    "A file was deleted but no instance matched it, so nothing was removed: {}",
                    path.display()
                ),
            }
        }
        return;
    }

    // Only content create/modify events are of interest.
    let is_relevant = matches!(
        event.kind,
        EventKind::Any | EventKind::Modify(_) | EventKind::Create(_)
    );
    if !is_relevant {
        return;
    }

    // If a file reappeared, its instance was not deleted but MOVED.
    // The pending deletion is cancelled.
    if !pending_deletes.is_empty() {
        let dm = data_model.blocking_read();
        for path in &event.paths {
            if let Some(u) = crate::layout::uuid_for_path(&dm, sync_dir, path) {
                pending_deletes.remove(&u);
            }
        }
    }

    let mut old_info: Option<(String, Option<String>)> = None;
    if event.paths.len() > 1 {
        if let Some((old_clean, old_uuid, _)) = parse_script_filename(&event.paths[0]) {
            old_info = Some((old_clean, old_uuid));
        }
    }

    for path in &event.paths {
        // Paths the user chose to ignore are not read.
        if crate::layout::is_ignored(path, sync_dir, ignore) {
            continue;
        }

        // Do not process our own writes: when the disk writer wrote a file, the watcher
        // took it for a user change. Because events arrive late, the OLD
        // version of the file was sometimes read and the model rolled back.
        if let Ok(current_value) = fs::read_to_string(path) {
            if crate::layout::is_own_write(path, &current_value) {
                continue;
            }
        }

        // The .meta.json extension is "json", so it would fall into the json branch below
        // and be silently swallowed when it failed to parse as an InstanceNode. It is caught here first.
        if path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.ends_with(".meta.json"))
            .unwrap_or(false)
            && handle_meta_file(path, tx_to_studio, data_model) {
                continue;
            }

        // .txt -> StringValue.Value
        if path
            .extension()
            .map(|e| e.to_string_lossy() == "txt")
            .unwrap_or(false)
        {
            handle_txt_file(path, tx_to_studio, data_model);
            continue;
        }

        // .csv -> LocalizationTable.Contents
        if path
            .extension()
            .map(|e| e.to_string_lossy() == "csv")
            .unwrap_or(false)
        {
            handle_csv_file(path, tx_to_studio, data_model);
            continue;
        }

        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy();

            if ext_str == "json" {
                if let Ok(content) = fs::read_to_string(path) {
                    let old_path = (event.paths.len() > 1).then(|| event.paths[0].as_path());
                    handle_instance_file(
                        path, &content, old_path, tx_to_studio, serializer, data_model, tx_to_vscode, sync_dir,
                    );
                }
            } else if ext_str == "lua" || ext_str == "luau" {
                let parsed = parse_script_filename(path);
                if let Some((clean_name, lua_uuid_opt, lua_ext)) = parsed {
                    let dm = data_model.blocking_read();
                    
                    // The parent is the instance whose folder this is, found by path. By name,
                    // the first instance of that name anywhere in the place was taken: with two
                    // PlayerModules, scripts went under the wrong one.
                    let parent_uuid_opt = path.parent().and_then(|dir| {
                        crate::layout::uuid_for_dir(&dm, sync_dir, dir).or_else(|| {
                            // A folder without a data file of its own: its name, but only when
                            // exactly one instance carries it (and its uuid suffix, if written).
                            let folder = dir.file_name()?.to_str()?;
                            let (name, short) = match folder.rsplit_once('_') {
                                Some((n, s)) if s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit()) => (n, s),
                                _ => (folder, ""),
                            };
                            let mut named = dm
                                .get_all_instances()
                                .iter()
                                .filter(|(u, n)| n.name == name && u.to_string().starts_with(short));
                            match (named.next(), named.next()) {
                                (Some((u, _)), None) => Some(*u),
                                _ => None,
                            }
                        })
                    });

                    // Search by UUID suffix; if not found (e.g. the user removed the suffix) fall back to
                    // name matching, so the file is never left without an owner.
                    let by_uuid = lua_uuid_opt
                        .as_ref()
                        .and_then(|s| dm.find_by_short_uuid(s));

                    let target_uuid = if by_uuid.is_some() {
                        by_uuid
                    } else if let Some((_, Some(ref old_short))) = old_info {
                        dm.find_by_short_uuid(old_short)
                    } else if let Some(old_name) = old_info.as_ref().map(|o| &o.0) {
                        if let Some(parent_uuid) = parent_uuid_opt {
                            dm.get_all_instances().iter().find_map(|(u, n)| {
                                if n.parent == Some(parent_uuid) && n.name.as_str() == old_name.as_str() {
                                    Some(*u)
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    } else if let Some(parent_uuid) = parent_uuid_opt {
                        // Only the script of that name. A fallback used to take any sibling
                        // script whose file did not exist yet and rename it to this file's name:
                        // while a tree was first written into an empty folder, most files did not
                        // exist yet, and unrelated scripts in Studio were renamed.
                        dm.get_all_instances().iter().find_map(|(u, n)| {
                            if n.parent == Some(parent_uuid) && n.name == clean_name {
                                Some(*u)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };

                    // MOVE. Moving a file to another folder shows up in the operating system as
                    // "delete + create". The matching above
                    // only looks at the SAME folder, so a moved file
                    // had no owner and a second instance was created.
                    // If a pending deletion has a script with the same name and class,
                    // this is not a new object but the moved one.
                    // Only when exactly one pending deletion fits: with two of that name, a
                    // guess moves the wrong one.
                    let target_uuid = target_uuid.or_else(|| {
                        let mut moved = pending_deletes.keys().copied().filter(|u| {
                            dm.get_instance(u)
                                .map(|n| n.name == clean_name && is_script_class(&n.class_name))
                                .unwrap_or(false)
                        });
                        match (moved.next(), moved.next()) {
                            (Some(u), None) => Some(u),
                            _ => None,
                        }
                    });

                    if let Some(uuid) = target_uuid {
                        // If it matched as a move, the deletion is cancelled.
                        pending_deletes.remove(&uuid);

                        // If the new folder points to another parent, the object's
                        // place in the tree must change too.
                        let parent_diff = match (parent_uuid_opt, dm.get_instance(&uuid).and_then(|n| n.parent)) {
                            (Some(fresh), previous_text) if Some(fresh) != previous_text => Some(fresh),
                            _ => None,
                        };

                        let source_diff = if let Ok(src) = fs::read_to_string(path) {
                            dm.get_instance(&uuid).map(|inst| inst.source.as_deref() != Some(src.as_str())).unwrap_or(false)
                        } else {
                            false
                        };

                        let inst_name = dm.get_instance(&uuid).map(|n| n.name.clone()).unwrap_or_default();
                        let name_diff = inst_name != clean_name && !clean_name.is_empty();

                        drop(dm);

                        if let Some(moved_to) = parent_diff {
                            {
                                let mut dm = data_model.blocking_write();
                                if let Err(e) = dm.reparent(&uuid, Some(moved_to)) {
                                    error!("The moved file could not be reparented: {}", e);
                                }
                            }
                            tx_to_studio.push(Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "REPARENT",
                                        "data": {
                                            "syncix_id": uuid,
                                            "newParentId": moved_to
                                        }
                                    }]
                                }),
                            });
                            info!("The file was moved; the instance was reparented: {}", clean_name);
                        }

                        if name_diff {
                            let payload = Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "RENAME_INSTANCE",
                                        "data": {
                                            "id": uuid,
                                            "newName": clean_name
                                        }
                                    }]
                                }),
                            };
                            tx_to_studio.push(payload);
                            debug!("Script renamed -> Studio: {} ({})", clean_name, uuid);

                            {
                                let mut dm_write = data_model.blocking_write();
                                if let Some(inst) = dm_write.get_mut_instance(&uuid) {
                                    inst.name = clean_name.clone();
                                }
                            }
                            // Notify the editor + refresh disk (the name changed, the path may change)
                            {
                                let dm_read = data_model.blocking_read();
                                notify_vscode_updated(&dm_read, &uuid, tx_to_vscode, "INSTANCE_UPDATED");
                            }
                            disk_notify.notify_one();
                        }

                        if source_diff {
                            if let Ok(source_code) = fs::read_to_string(path) {
                                let payload = Payload {
                                    version: "v1".to_string(),
                                    event_type: EventType::CompositeUpdate,
                                    data: serde_json::json!({
                                        "patches": [{
                                            "event_type": "PROPERTY_UPDATE",
                                            "data": {
                                                "syncix_id": uuid,
                                                "property": "Source",
                                                "value": source_code
                                            }
                                        }]
                                    }),
                                };
                                tx_to_studio.push(payload);

                                {
                                    let mut dm_write = data_model.blocking_write();
                                    if let Some(inst) = dm_write.get_mut_instance(&uuid) {
                                        inst.source = Some(source_code);
                                    }
                                }
                            }
                        }

                        // NOTE: the file is not renamed here. Correct naming on disk
                        // is the layout module's responsibility; when the two sides
                        // applied different rules, file names fought back and forth in a loop.
                    } else if let Some(parent_uuid) = parent_uuid_opt {
                        drop(dm);
                        let new_uuid = Uuid::new_v4();
                        let script_class = match lua_ext.as_str() {
                            "server.lua" => "Script",
                            "client.lua" => "LocalScript",
                            "lua" | "luau" => "ModuleScript",
                            _ => "Script",
                        };

                        let source_code = fs::read_to_string(path).unwrap_or_default();

                        let mut new_node = InstanceNode::new(script_class, &clean_name);
                        new_node.syncix_id = new_uuid;
                        new_node.parent = Some(parent_uuid);
                        new_node.source = Some(source_code.clone());

                        let payload = Payload {
                            version: "v1".to_string(),
                            event_type: EventType::CompositeUpdate,
                            data: serde_json::json!({
                                "patches": [
                                    {
                                        "event_type": "CREATE",
                                        "data": {
                                            "syncix_id": new_uuid,
                                            "class_name": script_class,
                                            "name": clean_name,
                                            "parent": parent_uuid
                                        }
                                    },
                                    {
                                        "event_type": "PROPERTY_UPDATE",
                                        "data": {
                                            "syncix_id": new_uuid,
                                            "property": "Source",
                                            "value": source_code
                                        }
                                    }
                                ]
                            }),
                        };

                        tx_to_studio.push(payload);

                        {
                            let mut dm_write = data_model.blocking_write();
                            let _ = dm_write.upsert_instance(new_node);
                        }
                        // Notify the editor + refresh disk
                        {
                            let dm_read = data_model.blocking_read();
                            notify_vscode_updated(&dm_read, &new_uuid, tx_to_vscode, "INSTANCE_CREATED");
                        }
                        disk_notify.notify_one();
                        info!("New script created on disk -> Studio: {} ({})", clean_name, new_uuid);
                    }
                }
            }
        }
    }
}
