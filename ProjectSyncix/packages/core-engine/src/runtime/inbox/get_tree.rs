//! What the core does with a GetTree message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one GetTree message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    tracing::info!("GET_TREE requested. Sending state...");
    let mut ws_nodes = Vec::new();
    {
        let dm = ctx.data_model.read().await;
        for instance in dm.get_all_instances().values() {
            // Do not send the internal root "Game" (DataModel) node out
            if instance.class_name == "DataModel" {
                continue;
            }
            ws_nodes.push(serde_json::json!({
                "id": instance.syncix_id,
                "name": instance.name,
                "className": instance.class_name,
                "parentId": instance.parent.map(|u| u.to_string()),
                "childrenIds": [],
                "isExpanded": false
            }));
        }
    }
    
    let ws_msg = serde_json::json!({
        "event_type": "FULL_SYNC",
        "data": {
            "nodes": ws_nodes
        }
    });
    let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
}
