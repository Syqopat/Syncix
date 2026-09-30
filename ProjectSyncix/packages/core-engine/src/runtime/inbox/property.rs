//! What the core does with a SetProperty message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{layout, model, rbxmx_import, transport, tree_import};

/// Applies one SetProperty message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // VS Code/CLI -> Studio: set a property
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let property = payload
        .data
        .get("property")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // The value may come as a string from the CLI ("0.5", "true", "1,2,3") or typed
    // from the Inspector ({"Color3":{...}}, number, bool).
    let value_json = payload
        .data
        .get("value")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let value_str = value_json
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| value_json.to_string());

    let resolved = {
        let dm = ctx.data_model.read().await;
        resolve_id(&dm, id)
    };

    if property.is_empty() {
        tracing::warn!("SET_PROPERTY: the property name was empty.");
    } else if let Some(uuid) = resolved {
        // A value from the terminal is plain text; it carries no type information.
        // So it is first fitted to the type of the property's CURRENT
        // value: `set X BrickColor "Really red"` becomes a BrickColor, not text,
        // `set X CFrame 0,10,0` becomes a CFrame, not text.
        // The type decision is still made from a value — the value compared
        // is simply the one already in the model. The name is fitted too, to the
        // instance's own spelling (see canonical_property_name).
        let (canonical, current_value, class_name) = {
            let dm = ctx.data_model.read().await;
            match dm.get_instance(&uuid) {
                Some(inst) => {
                    let canonical = canonical_property_name(&inst.properties, &property);
                    let current_value = inst.properties.get(&canonical).cloned();
                    (canonical, current_value, Some(inst.class_name.clone()))
                }
                None => (property.clone(), None, None),
            }
        };
        if canonical != property {
            tracing::info!(
                "SET_PROPERTY: '{}' was read as '{}' (Studio's property names are case-sensitive).",
                property,
                canonical
            );
        }
        let property = canonical;
        let parsed = match &value_json {
            serde_json::Value::String(s) => {
                value_for_class(class_name.as_deref(), &property, current_value.as_ref(), s)
            }
            other => match parse_wire_value(other) {
                Some(pv) => Ok(pv),
                // The fallback below forwards text, which a colour property refuses.
                None if color_target(&property, current_value.as_ref()).is_some() => {
                    Err(format!("{} is not a colour", value_str))
                }
                None => Ok(model::PropertyValue::String(value_str.clone())),
            },
        };
        let mut pv = match parsed {
            Ok(pv) => pv,
            Err(reason) => {
                let help = if color_target(&property, current_value.as_ref()).is_some() {
                    color_forms_help().iter().map(|l| l.trim().to_string()).collect::<Vec<_>>().join(" ")
                } else {
                    String::new()
                };
                tracing::warn!(
                    "SET_PROPERTY: {}.{} was left unchanged: {} {}",
                    id,
                    property,
                    reason,
                    help
                );
                return;
            }
        };
        // Without a current value there is no type to fit to, and "x,y,z" parses as a
        // Vector3, which Studio refuses for a CFrame. A CFrame it is, unrotated.
        if property == "CFrame" {
            if let model::PropertyValue::Vector3 { x, y, z } = pv {
                pv = model::PropertyValue::CFrame {
                    pos: [x, y, z],
                    rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                };
            }
        }

        // The reference target is resolved to a full UUID here. The user may type a short
        // handle or a name ("syncix set door Part0 hinge"),
        // but the cache in Studio is looked up by full UUID only;
        // sent unresolved, the reference would silently stay nil.
        if let model::PropertyValue::Ref(dest) = &pv {
            if !dest.is_empty() {
                let resolved_n = {
                    let dm = ctx.data_model.read().await;
                    resolve_id(&dm, dest)
                };
                match resolved_n {
                    Some(u) => pv = model::PropertyValue::Ref(u.to_string()),
                    None => {
                        tracing::warn!(
                            "SET_PROPERTY: reference target '{}' was not found; the property was left unchanged.",
                            dest
                        );
                        // continue, not return: this is main's dispatcher loop, and
                        // returning from it shut the whole core down over one typo.
                        return;
                    }
                }
            }
        }
        let mut ok = false;
        {
            let mut dm = ctx.data_model.write().await;
            if let Some(inst) = dm.get_mut_instance(&uuid) {
                if property == "Name" {
                    inst.name = value_str.clone();
                } else {
                    inst.properties.insert(property.clone(), pv.clone());
                }
                inst.last_updated = chrono::Utc::now().timestamp_millis();
                ok = true;
            }
        }

        if ok {
            // Apply in Studio
            let wire_value = if property == "Name" {
                serde_json::json!(value_str)
            } else {
                pv_to_wire(&pv)
            };
            ctx.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::CompositeUpdate,
                data: serde_json::json!({
                    "patches": [{
                        "event_type": "PROPERTY_UPDATE",
                        "data": {
                            "syncix_id": uuid,
                            "property": property,
                            "value": wire_value
                        }
                    }]
                }),
            });
            // Notify the VS Code Explorer (the name may have changed)
            let instance = {
                let dm = ctx.data_model.read().await;
                dm.get_instance(&uuid).cloned()
            };
            if let Some(instance) = instance {
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
            }
            tracing::info!("SET_PROPERTY handled: {} .{} = {}", id, property, value_str);
        } else {
            tracing::warn!("SET_PROPERTY: unknown UUID: {}", id);
        }
    } else {
        tracing::warn!("SET_PROPERTY target not found: {}", id);
    }
}
