//! Split out of file_sync.rs.

#[allow(unused_imports)]
use super::*;

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
pub(crate) fn handle_instance_file(
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
pub(crate) fn update_instance_file(
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
                "data": { "syncix_id": uuid, "property": property, "value": crate::values::pv_to_wire(&value) }
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
pub(crate) fn repair_data_file(
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
pub(crate) fn create_from_file(
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
