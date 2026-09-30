//! What the core does with a RenameInstance message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one RenameInstance message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code -> Studio: rename
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let new_name = payload.data.get("newName").and_then(|v| v.as_str()).unwrap_or("");
    let resolved = {
        let dm = ctx.data_model.read().await;
        resolve_id(&dm, id)
    };
    if new_name.is_empty() {
        tracing::warn!("RENAME_INSTANCE: newName was empty, ignored.");
    } else if let Some(uuid) = resolved {
        let mut updated_instance = None;
        let mut old_name = None;
        {
            let mut dm = ctx.data_model.write().await;
            if let Some(instance) = dm.get_mut_instance(&uuid) {
                if instance.parent.is_none() {
                    tracing::warn!("RENAME_INSTANCE: services cannot be renamed ({}).", instance.name);
                    return;
                }
                old_name = Some(instance.name.clone());
                instance.name = new_name.to_string();
                instance.last_updated = chrono::Utc::now().timestamp_millis();
                updated_instance = Some(instance.clone());
            }
        }
        if let Some(instance) = updated_instance {
            let _ = &old_name;
            // Disk writing is done by the central debounced writer (layout).
            // Apply to Studio
            ctx.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::CompositeUpdate,
                data: serde_json::json!({
                    "patches": [{
                        "event_type": "PROPERTY_UPDATE",
                        "data": {
                            "syncix_id": uuid,
                            "property": "Name",
                            "value": instance.name
                        }
                    }]
                }),
            });
            // Reflect in the VS Code Explorer
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
                "VS Code RENAME handled: {} -> {}",
                old_name.unwrap_or_default(),
                instance.name
            );
        } else {
            tracing::warn!("RENAME_INSTANCE: unknown UUID: {}", id);
        }
    } else {
        tracing::warn!("RENAME_INSTANCE target not found: {}", id);
    }
}
