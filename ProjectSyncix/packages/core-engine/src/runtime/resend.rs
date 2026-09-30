//! Which messages have to be sent to Studio again after a reconnect.

use crate::model::{self};
use crate::transport::{self, EventType, Payload};
use crate::values::pv_to_wire;

/// After a FULL_SYNC: the messages that bring Studio up to what the core kept for it.
/// `kept` lists instances carried over into the model (parents first); `delivered` is
/// what Studio was handed but did not apply. Only those are sent: what is still queued
/// will be delivered anyway.
pub(crate) fn resend_payloads(
    dm: &model::DataModel,
    kept: &[uuid::Uuid],
    delivered: &transport::InFlight,
) -> Vec<Payload> {
    let mut out = Vec::new();
    for id in kept.iter().filter(|id| delivered.creates.contains(id)) {
        if let Some(node) = dm.get_instance(id) {
            if let Ok(data) = serde_json::to_value(node) {
                out.push(Payload {
                    version: "v1".to_string(),
                    event_type: EventType::PushUpdate,
                    data,
                });
            }
        }
    }

    let mut patches = Vec::new();
    for (id, property) in &delivered.properties {
        let Some(node) = dm.get_instance(id) else { continue };
        let value = match property.as_str() {
            "Name" => serde_json::json!(node.name),
            "Source" => match &node.source {
                Some(source) => serde_json::json!(source),
                None => continue,
            },
            _ => match node.properties.get(property) {
                Some(pv) => pv_to_wire(pv),
                None => continue,
            },
        };
        patches.push(serde_json::json!({
            "event_type": "PROPERTY_UPDATE",
            "data": { "syncix_id": id, "property": property, "value": value }
        }));
    }
    for id in &delivered.reparents {
        if let Some(parent) = dm.get_instance(id).and_then(|n| n.parent) {
            patches.push(serde_json::json!({
                "event_type": "REPARENT",
                "data": { "syncix_id": id, "parent": parent.to_string() }
            }));
        }
    }
    for (id, name) in &delivered.attributes {
        let Some(node) = dm.get_instance(id) else { continue };
        let value = node.attributes.get(name).map(pv_to_wire).unwrap_or(serde_json::Value::Null);
        patches.push(serde_json::json!({
            "event_type": "ATTRIBUTE_UPDATE",
            "data": { "syncix_id": id, "name": name, "value": value }
        }));
    }
    for id in &delivered.tags {
        if let Some(node) = dm.get_instance(id) {
            patches.push(serde_json::json!({
                "event_type": "TAGS_UPDATE",
                "data": { "syncix_id": id, "tags": node.tags }
            }));
        }
    }
    for id in &delivered.destroys {
        if dm.get_instance(id).is_none() {
            patches.push(serde_json::json!({ "event_type": "DESTROY", "data": { "syncix_id": id } }));
        }
    }
    if !patches.is_empty() {
        out.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": patches }),
        });
    }
    out
}
