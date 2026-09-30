//! What the core does with a CreateInstance message from Studio.

use crate::model::InstanceNode;
use crate::runtime::Ctx;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, rbxmx_import};

/// Applies one CreateInstance message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code -> Studio: create a new instance.
    // The UUID is generated only here, at CREATE time (data integrity rule).
    let class_name = payload.data.get("className").and_then(|v| v.as_str()).unwrap_or("");
    let parent_id = payload.data.get("parentId").and_then(|v| v.as_str()).unwrap_or("");
    // Without a given name the class name is used (the old behaviour is kept).
    let node_name = payload
        .data
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(class_name);
    if class_name.is_empty() {
        tracing::warn!("CREATE_INSTANCE: className was empty, ignored.");
    } else if rbxmx_import::is_singleton(class_name) {
        // Studio cannot create a service or a singleton container; a model entry
        // for one would be a phantom that also makes the real one's name ambiguous.
        tracing::warn!("CREATE_INSTANCE: {} exists once per place and cannot be created, ignored.", class_name);
    } else {
        let mut instance = InstanceNode::new(class_name, node_name);
        // If the client supplied a UUID, use it; that lets an import target the object
        // it created by identity rather than by name.
        if let Some(given) = payload
            .data
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
        {
            instance.syncix_id = given;
        }
        // Properties and a script's source may come with the create (syncix import
        // sends them together): one command and one message to Studio per instance
        // instead of one per property. Values are typed wire values.
        let mut property_patches = Vec::new();
        let (mesh_id, collision_fidelity, render_fidelity) =
            mesh_create_fields(class_name, payload.data.get("properties"));
        {
            // Parent resolution: UUID or name (e.g. "Workspace").
            // When empty it attaches to the Workspace service by default.
            let dm = ctx.data_model.read().await;
            let effective_parent = if parent_id.is_empty() { "Workspace" } else { parent_id };
            instance.parent = resolve_id(&dm, effective_parent);

            if let Some(props) = payload.data.get("properties").and_then(|v| v.as_object()) {
                for (property, raw) in props {
                    if property == "Name" {
                        continue;
                    }
                    // The mesh is kept in the model, but never sent as a property:
                    // the plugin builds the part from it (see mesh_create_fields).
                    if is_creation_only_property(class_name, property) {
                        if let Some(pv) = parse_wire_value(raw) {
                            instance.properties.insert(property.clone(), pv);
                        }
                        continue;
                    }
                    let Some(mut pv) = parse_wire_value(raw) else {
                        tracing::warn!("CREATE_INSTANCE: {}.{} has a value Syncix cannot read, skipped.", node_name, property);
                        continue;
                    };
                    // A reference is sent as a full UUID, the only form Studio's cache finds.
                    if let model::PropertyValue::Ref(dest) = &pv {
                        if !dest.is_empty() {
                            match resolve_id(&dm, dest) {
                                Some(u) => pv = model::PropertyValue::Ref(u.to_string()),
                                None => {
                                    tracing::warn!(
                                        "CREATE_INSTANCE: {}.{} refers to '{}', which was not found; skipped.",
                                        node_name, property, dest
                                    );
                                    continue;
                                }
                            }
                        }
                    }
                    property_patches.push(serde_json::json!({
                        "event_type": "PROPERTY_UPDATE",
                        "data": { "syncix_id": instance.syncix_id, "property": property, "value": pv_to_wire(&pv) }
                    }));
                    instance.properties.insert(property.clone(), pv);
                }
            }
            if let Some(src) = payload.data.get("source").and_then(|v| v.as_str()) {
                instance.source = Some(src.to_string());
                property_patches.push(serde_json::json!({
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": instance.syncix_id, "property": "Source", "value": src }
                }));
            }
            // Attributes and tags travel with the create as well: half of a game's
            // logic can run through tags, and an import that dropped them created
            // objects the game's own scripts could not find.
            if let Some(attributes) = payload.data.get("attributes").and_then(|v| v.as_object()) {
                for (name, raw) in attributes {
                    let Some(pv) = parse_wire_value(raw) else {
                        tracing::warn!("CREATE_INSTANCE: attribute {}.{} could not be read, skipped.", node_name, name);
                        continue;
                    };
                    property_patches.push(serde_json::json!({
                        "event_type": "ATTRIBUTE_UPDATE",
                        "data": { "syncix_id": instance.syncix_id, "name": name, "value": pv_to_wire(&pv) }
                    }));
                    instance.attributes.insert(name.clone(), pv);
                }
            }
            if let Some(tags) = payload.data.get("tags").and_then(|v| v.as_array()) {
                let list: Vec<String> =
                    tags.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect();
                if !list.is_empty() {
                    property_patches.push(serde_json::json!({
                        "event_type": "TAGS_UPDATE",
                        "data": { "syncix_id": instance.syncix_id, "tags": list }
                    }));
                    instance.tags = list;
                }
            }
        }
        let uuid = instance.syncix_id;
        let insert_result = {
            let mut dm = ctx.data_model.write().await;
            dm.upsert_instance(instance.clone())
        };
        match insert_result {
            Ok(()) => {
                // Disk writing is done by the central debounced writer (layout).
                // Send CREATE to Studio, its properties right behind it in the same
                // message (Studio applies a message's patches in order).
                let mut create_data = serde_json::json!({
                    "syncix_id": uuid,
                    "class_name": instance.class_name,
                    "name": instance.name,
                    "parent": instance.parent.map(|u| u.to_string())
                });
                if let Some(mesh) = &mesh_id {
                    create_data["mesh_id"] = serde_json::json!(mesh);
                    if let Some(value) = &collision_fidelity {
                        create_data["collision_fidelity"] = serde_json::json!(value);
                    }
                    if let Some(value) = &render_fidelity {
                        create_data["render_fidelity"] = serde_json::json!(value);
                    }
                }
                let mut patches = vec![serde_json::json!({
                    "event_type": "CREATE",
                    "data": create_data
                })];
                patches.extend(property_patches);
                ctx.studio_outbox.push(Payload {
                    version: "v1".to_string(),
                    event_type: EventType::CompositeUpdate,
                    data: serde_json::json!({ "patches": patches }),
                });
                // Reflect in the VS Code Explorer
                let ws_msg = serde_json::json!({
                    "event_type": "INSTANCE_CREATED",
                    "data": {
                        "id": uuid,
                        "name": instance.name,
                        "className": instance.class_name,
                        "parentId": instance.parent.map(|u| u.to_string()),
                        "childrenIds": [],
                        "isExpanded": false
                    }
                });
                let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                tracing::info!("CREATE handled: {} ({})", instance.name, instance.class_name);
            }
            Err(e) => tracing::warn!("CREATE_INSTANCE upsert failed: {}", e),
        }
    }
}
