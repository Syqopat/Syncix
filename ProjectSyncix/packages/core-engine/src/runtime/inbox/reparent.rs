//! What the core does with a ReparentInstance message from Studio.

use crate::runtime::Ctx;
use crate::transport::{EventType, Payload};
use crate::values::*;

/// Applies one ReparentInstance message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code/CLI -> Studio: move
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let new_parent_field = payload
        .data
        .get("newParentId")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (resolved_id, resolved_parent) = {
        let dm = ctx.data_model.read().await;
        (resolve_id(&dm, id), resolve_id(&dm, new_parent_field))
    };

    if let (Some(uuid), Some(new_parent)) = (resolved_id, resolved_parent) {
        if uuid == new_parent {
            tracing::warn!("REPARENT_INSTANCE: an instance cannot be moved into itself.");
            return;
        }
        // Service nodes cannot be moved
        let is_service = {
            let dm = ctx.data_model.read().await;
            dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
        };
        if is_service {
            tracing::warn!("REPARENT_INSTANCE: services cannot be moved ({}).", id);
            return;
        }

        let result = {
            let mut dm = ctx.data_model.write().await;
            dm.reparent(&uuid, Some(new_parent))
        };

        match result {
            Ok((old_parent, _)) => {
                let instance = {
                    let dm = ctx.data_model.read().await;
                    dm.get_instance(&uuid).cloned()
                };
                if let Some(instance) = instance {
                    // Disk writing is done by the central debounced writer (layout).
                    // Send REPARENT to Studio
                    ctx.studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "REPARENT",
                                "data": {
                                    "syncix_id": uuid,
                                    "parent": new_parent.to_string()
                                }
                            }]
                        }),
                    });
                    // Reflect in the VS Code Explorer
                    let ws_msg = serde_json::json!({
                        "event_type": "INSTANCE_MOVED",
                        "data": {
                            "id": uuid,
                            "oldParentId": old_parent.map(|u| u.to_string()),
                            "newParentId": new_parent.to_string()
                        }
                    });
                    let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
                    tracing::info!("REPARENT handled: {} -> {}", instance.name, new_parent);
                }
            }
            Err(e) => tracing::warn!("REPARENT_INSTANCE failed: {}", e),
        }
    } else {
        tracing::warn!("REPARENT_INSTANCE target or parent not found (id={}, parent={})", id, new_parent_field);
    }
}
