//! What the core does with a CompositeUpdate message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one CompositeUpdate message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    if let Some(patches) = payload.data.get("patches").and_then(|p| p.as_array()) {
        for patch in patches {
            let p_type = patch.get("event_type").and_then(|e| e.as_str()).unwrap_or("");
            if p_type == "CREATE" {
                // Extract basic info
                if let (Some(_data), Some(class_name), Some(name), Some(syncix_id)) = (
                    patch.get("data"),
                    patch.get("data").and_then(|d| d.get("class_name")).and_then(|v| v.as_str()),
                    patch.get("data").and_then(|d| d.get("name")).and_then(|v| v.as_str()),
                    patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                ) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                        let mut instance = InstanceNode::new(class_name, name);
                        instance.syncix_id = uuid;

                        // Parent UUID (for the hierarchy)
                        if let Some(parent_str) = patch
                            .get("data")
                            .and_then(|d| d.get("parent"))
                            .and_then(|v| v.as_str())
                        {
                            if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                instance.parent = Some(parent_uuid);
                            }
                        }

                        // Script source code
                        if let Some(src) = patch
                            .get("data")
                            .and_then(|d| d.get("source"))
                            .and_then(|v| v.as_str())
                        {
                            instance.source = Some(src.to_string());
                        }

                        // Attributes
                        if let Some(attrs) = patch
                            .get("data")
                            .and_then(|d| d.get("attributes"))
                            .and_then(|v| v.as_object())
                        {
                            for (k, val) in attrs {
                                if let Some(pv) = parse_wire_value(val) {
                                    instance.attributes.insert(k.clone(), pv);
                                }
                            }
                        }

                        // Properties (wide scope)
                        if let Some(props) = patch
                            .get("data")
                            .and_then(|d| d.get("properties"))
                            .and_then(|v| v.as_object())
                        {
                            for (k, val) in props {
                                if let Some(pv) = parse_wire_value(val) {
                                    instance.properties.insert(k.clone(), pv);
                                }
                            }
                        }

                        // Save to DataModel
                        {
                            let mut dm = ctx.data_model.write().await;
                            if let Err(e) = dm.upsert_instance(instance.clone()) {
                                tracing::warn!("CREATE upsert failed ({}): {}", instance.name, e);
                            }
                        }
                        
                        // Disk writing is done by the central debounced writer (layout).

                        // Notify VS Code (for every class, with or without a serializer)
                        let ws_msg = serde_json::json!({
                            "event_type": "INSTANCE_CREATED",
                            "data": {
                                "id": instance.syncix_id,
                                "name": instance.name,
                                "className": instance.class_name,
                                "parentId": instance.parent.map(|u| u.to_string()),
                                "childrenIds": instance.children,
                                "isExpanded": false
                            }
                        });
                        let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                        tracing::info!(
                            "Studio CREATE handled: {} ({})",
                            instance.name,
                            instance.class_name
                        );
                    }
                }
            } else if p_type == "PROPERTY_UPDATE" {
                if let (Some(_data), Some(syncix_id), Some(property), Some(value)) = (
                    patch.get("data"),
                    patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                    patch.get("data").and_then(|d| d.get("property")).and_then(|v| v.as_str()),
                    patch.get("data").and_then(|d| d.get("value")),
                ) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                        // Echo loop detection: if the same property bounces back and forth many times in a short
                        // time, echo suppression missed it. This used to be
                        // completely silent; now the property involved is logged.
                        if ctx.state.health_monitor
                            .loop_detector
                            .persist(syncix_id, property)
                        {
                            tracing::warn!(
                                "Suspected echo loop: {} .{} was updated many times in a short window. Studio and the core may be writing the same value back and forth.",
                                syncix_id,
                                property
                            );
                        }

                        let mut updated_instance = None;
                        let mut old_name = None;

                        {
                            let mut dm = ctx.data_model.write().await;
                            if let Some(instance) = dm.get_mut_instance(&uuid) {
                                old_name = Some(instance.name.clone());
                                
                                if property == "Name" {
                                    if let Some(new_name) = value.as_str() {
                                        instance.name = new_name.to_string();
                                    }
                                } else if property == "Source" {
                                    if let Some(src) = value.as_str() {
                                        instance.source = Some(src.to_string());
                                    }
                                } else if let Some(pv) = parse_wire_value(value) {
                                    instance.properties.insert(property.to_string(), pv);
                                }
                                updated_instance = Some(instance.clone());
                            }
                        }
                        
                        if let Some(instance) = updated_instance {
                            let _ = &old_name;
                            // Disk writing is done by the central debounced writer (layout).

                            // Notify VS Code (for every class)
                            let ws_msg = serde_json::json!({
                                "event_type": "INSTANCE_UPDATED",
                                "data": {
                                    "id": instance.syncix_id,
                                    "name": instance.name,
                                    "className": instance.class_name,
                                    "parentId": instance.parent.map(|u| u.to_string()),
                                    "childrenIds": instance.children,
                                    "isExpanded": false
                                }
                            });
                            let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                            tracing::info!(
                                "Studio PROPERTY_UPDATE handled: {} -> {}",
                                instance.name,
                                property
                            );
                        } else {
                            tracing::warn!(
                                "PROPERTY_UPDATE ignored for an unknown UUID: {} (property: {}). FULL_SYNC may not have arrived yet.",
                                uuid,
                                property
                            );
                        }
                    }
                }
            } else if p_type == "DESTROY" {
                if let Some(syncix_id) = patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()) {
                    if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                        let removed_instance = {
                            let mut dm = ctx.data_model.write().await;
                            dm.remove_instance(&uuid)
                        };
                        
                        if let Some(instance) = removed_instance {
                            // Disk writing is done by the central debounced writer (layout).
                            let ws_msg = serde_json::json!({
                                "event_type": "INSTANCE_REMOVED",
                                "data": {
                                    "id": uuid,
                                    "parentId": instance.parent.map(|u| u.to_string())
                                }
                            });
                            let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                            tracing::info!("Studio DESTROY handled: {}", instance.name);
                        } else {
                            tracing::warn!(
                                "DESTROY ignored for an unknown UUID: {}",
                                uuid
                            );
                        }
                    }
                }
            } else if p_type == "REKEY" {
                // Team Create: two people's plugins gave one new object different
                // identities; the plugins settled on one (the smaller) and the model
                // follows, so the object keeps its files and children.
                let ids = patch.get("data").and_then(|d| {
                    let old = d.get("syncix_id").and_then(|v| v.as_str()).and_then(|s| uuid::Uuid::parse_str(s).ok())?;
                    let new = d.get("new_id").and_then(|v| v.as_str()).and_then(|s| uuid::Uuid::parse_str(s).ok())?;
                    Some((old, new))
                });
                if let Some((old, new)) = ids {
                    let result = {
                        let mut dm = ctx.data_model.write().await;
                        dm.rekey(&old, &new).map(|_| dm.get_instance(&new).cloned())
                    };
                    match result {
                        Ok(Some(instance)) => {
                            let parent_id = instance.parent.map(|u| u.to_string());
                            let _ = ctx.state.tx_to_vscode.send(
                                serde_json::json!({
                                    "event_type": "INSTANCE_REMOVED",
                                    "data": { "id": old, "parentId": parent_id }
                                })
                                .to_string(),
                            );
                            let _ = ctx.state.tx_to_vscode.send(
                                serde_json::json!({
                                    "event_type": "INSTANCE_CREATED",
                                    "data": {
                                        "id": new,
                                        "name": instance.name,
                                        "className": instance.class_name,
                                        "parentId": parent_id,
                                        "childrenIds": instance.children,
                                        "isExpanded": false
                                    }
                                })
                                .to_string(),
                            );
                            tracing::info!("Studio REKEY handled: {} {} -> {}", instance.name, old, new);
                        }
                        Ok(None) => {}
                        Err(e) => tracing::warn!("REKEY {} -> {} failed: {}", old, new, e),
                    }
                }
            } else if p_type == "REPARENT" {
                // The object was moved to another parent in Studio.
                let syncix_id = patch
                    .get("data")
                    .and_then(|d| d.get("syncix_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_parent_str = patch
                    .get("data")
                    .and_then(|d| d.get("parent"))
                    .and_then(|v| v.as_str());

                if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                    let new_parent = new_parent_str
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());

                    let result = {
                        let mut dm = ctx.data_model.write().await;
                        dm.reparent(&uuid, new_parent)
                    };

                    match result {
                        Ok((old_parent, _)) => {
                            let instance = {
                                let dm = ctx.data_model.read().await;
                                dm.get_instance(&uuid).cloned()
                            };
                            if let Some(instance) = instance {
                                // Disk writing is done by the central debounced writer (layout).
                                // Report the move to the VS Code Explorer
                                let ws_msg = serde_json::json!({
                                    "event_type": "INSTANCE_MOVED",
                                    "data": {
                                        "id": uuid,
                                        "oldParentId": old_parent.map(|u| u.to_string()),
                                        "newParentId": new_parent.map(|u| u.to_string())
                                    }
                                });
                                let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                                tracing::info!(
                                    "Studio REPARENT handled: {} -> parent {}",
                                    instance.name,
                                    new_parent.map(|u| u.to_string()).unwrap_or_else(|| "none".to_string())
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("REPARENT failed ({}): {}", uuid, e);
                        }
                    }
                }
            } else if p_type == "ATTRIBUTE_UPDATE" {
                // An attribute changed in Studio.
                let syncix_id = patch
                    .get("data")
                    .and_then(|d| d.get("syncix_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let name = patch
                    .get("data")
                    .and_then(|d| d.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let value = patch.get("data").and_then(|d| d.get("value"));

                if let (Ok(uuid), false, Some(val)) =
                    (uuid::Uuid::parse_str(syncix_id), name.is_empty(), value)
                {
                    if let Some(pv) = parse_wire_value(val) {
                        let mut applied = false;
                        {
                            let mut dm = ctx.data_model.write().await;
                            if let Some(inst) = dm.get_mut_instance(&uuid) {
                                inst.attributes.insert(name.to_string(), pv);
                                applied = true;
                            }
                        }
                        if applied {
                            tracing::info!("Studio ATTRIBUTE_UPDATE handled: {} @{}", name, syncix_id);
                        }
                    }
                }
            }
        }
    }
}
