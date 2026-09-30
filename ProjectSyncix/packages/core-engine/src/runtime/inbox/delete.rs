//! What the core does with a DeleteInstance message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one DeleteInstance message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code -> Studio: delete
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let resolved = {
        let dm = ctx.data_model.read().await;
        resolve_id(&dm, id)
    };
    if let Some(uuid) = resolved {
        // Service nodes cannot be deleted
        let is_service = {
            let dm = ctx.data_model.read().await;
            dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
        };
        if is_service {
            tracing::warn!("DELETE_INSTANCE: services cannot be deleted ({}).", id);
            return;
        }
        let removed_instance = {
            let mut dm = ctx.data_model.write().await;
            dm.remove_instance(&uuid)
        };
        if let Some(instance) = removed_instance {
            // Disk writing is done by the central debounced writer (layout).
            // Send DESTROY to Studio
            ctx.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::CompositeUpdate,
                data: serde_json::json!({
                    "patches": [{
                        "event_type": "DESTROY",
                        "data": { "syncix_id": uuid }
                    }]
                }),
            });
            // Reflect in the VS Code Explorer
            let ws_msg = serde_json::json!({
                "event_type": "INSTANCE_REMOVED",
                "data": {
                    "id": uuid,
                    "parentId": instance.parent.map(|u| u.to_string())
                }
            });
            let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
            tracing::info!("DELETE handled: {}", instance.name);
        } else {
            tracing::warn!("DELETE_INSTANCE: unknown UUID: {}", id);
        }
    } else {
        tracing::warn!("DELETE_INSTANCE target not found: {}", id);
    }
}
