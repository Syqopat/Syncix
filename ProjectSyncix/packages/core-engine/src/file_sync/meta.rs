//! Split out of file_sync.rs.

#[allow(unused_imports)]
use super::*;

/// Applies a .meta.json change on disk to the model and to Studio.
pub(crate) fn handle_meta_file(
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
                        "value": crate::values::pv_to_wire(raw_value)
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
                        "value": crate::values::pv_to_wire(raw_value)
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
