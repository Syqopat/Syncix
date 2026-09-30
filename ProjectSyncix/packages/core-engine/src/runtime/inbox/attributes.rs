//! What the core does with a SetAttribute message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one SetAttribute message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code/CLI -> Studio: set an attribute
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let name = payload
        .data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // A JSON null value field means this is a DELETE request.
    // On the Studio side SetAttribute(name, nil) removes the attribute; the same meaning
    // travels as null on the wire, so no separate event type is needed.
    let deletion = payload
        .data
        .get("value")
        .map(|v| v.is_null())
        .unwrap_or(true);
    let value_str = payload
        .data
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let resolved = {
        let dm = ctx.data_model.read().await;
        resolve_id(&dm, id)
    };

    if name.is_empty() {
        tracing::warn!("SET_ATTRIBUTE: the attribute name was empty.");
    } else if let Some(uuid) = resolved {
        let pv = parse_property_value(&value_str);
        let mut ok = false;
        {
            let mut dm = ctx.data_model.write().await;
            if let Some(inst) = dm.get_mut_instance(&uuid) {
                if deletion {
                    inst.attributes.remove(&name);
                } else {
                    inst.attributes.insert(name.clone(), pv.clone());
                }
                inst.last_updated = chrono::Utc::now().timestamp_millis();
                ok = true;
            }
        }
        if ok {
            ctx.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::CompositeUpdate,
                data: serde_json::json!({
                    "patches": [{
                        "event_type": "ATTRIBUTE_UPDATE",
                        "data": {
                            "syncix_id": uuid,
                            "name": name,
                            "value": if deletion { serde_json::Value::Null } else { pv_to_wire(&pv) }
                        }
                    }]
                }),
            });
            tracing::info!("SET_ATTRIBUTE handled: {} @{} = {}", name, id, value_str);
        } else {
            tracing::warn!("SET_ATTRIBUTE for an unknown UUID: {}", id);
        }
    } else {
        tracing::warn!("SET_ATTRIBUTE target not found: {}", id);
    }
}
