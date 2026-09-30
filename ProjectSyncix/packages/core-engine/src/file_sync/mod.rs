mod deletes;
mod instances;
mod meta;
mod names;
mod editor_notify;
mod raw_files;

pub(crate) use deletes::*;
pub(crate) use instances::*;
pub(crate) use meta::*;
pub(crate) use names::*;
pub(crate) use editor_notify::*;
pub(crate) use raw_files::*;

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

