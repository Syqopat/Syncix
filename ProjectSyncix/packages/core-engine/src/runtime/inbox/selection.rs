//! What the core does with a Selection message from Studio.

use crate::runtime::Ctx;
use crate::transport::{EventType, Payload};
use crate::values::*;

/// Applies one Selection message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // Selection is a two-way MIRROR: an object clicked in Studio is selected in the editor,
    // an object clicked in the editor is selected in Studio.
    //
    // It is not written to the model because selection is not project content but momentary
    // state. Written to disk, every click would be a file change and
    // version control would drown in noise.
    let identities: Vec<String> = payload
        .data
        .get("ids")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // The source travels with the message to stop it bouncing back: sending a selection
    // that came from Studio back to Studio would be an endless ping-pong.
    let origin = payload
        .data
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("studio");

    if origin == "studio" {
        let ws = serde_json::json!({
            "event_type": "SELECTION",
            "data": { "ids": identities }
        });
        let _ = ctx.state.tx_to_vscode.send(ws.to_string());
    } else {
        // Came from the editor: apply in Studio.
        let already_resolved: Vec<String> = {
            let dm = ctx.data_model.read().await;
            identities
                .iter()
                .filter_map(|h| resolve_id(&dm, h).map(|u| u.to_string()))
                .collect()
        };
        ctx.studio_outbox.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "SELECTION_UPDATE",
                    "data": { "ids": already_resolved }
                }]
            }),
        });
    }
}
