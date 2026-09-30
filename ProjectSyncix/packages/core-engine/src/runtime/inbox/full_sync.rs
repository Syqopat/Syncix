//! What the core does with a FullSync message from Studio.

use crate::model::InstanceNode;
use crate::runtime::{resend_payloads, Ctx};
use crate::transport::Payload;
use crate::values::*;

/// Applies one FullSync message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    // PLACE IDENTITY GATE
    //
    // A sync folder belongs to ONE place. When another place connected to the same
    // folder, the two trees used to merge silently:
    // service UUIDs are the same in every place, so both
    // landed on the same skeleton and SINGLETON objects such as StarterPlayerScripts
    // were duplicated. On top of that, old files on disk were taken for "new objects"
    // and created inside the new place.
    //
    // We no longer merge: if a different place arrives we stop and
    // leave the decision to the user (syncix bind).
    let incoming_place = payload
        .data
        .get("place_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !incoming_place.is_empty() {
        match ctx.cfg.linked_place() {
            // The folder is empty or being bound for the first time: claim it.
            None => {
                ctx.cfg.bind_place(&incoming_place);
                tracing::info!("This folder is now bound to the connected place.");
            }
            Some(current_value) if current_value == incoming_place => {
                // Same place, no problem.
                if let Ok(mut c) = ctx.state.place_clash_state.lock() {
                    *c = None;
                }
            }
            Some(current_value) => {
                let item_name = payload
                    .data
                    .get("place_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let pid = payload
                    .data
                    .get("place_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .to_string();

                tracing::error!(
                    "This folder belongs to a different place. Sync is on hold so nothing gets mixed.
                       folder is bound to : {}
                       place connecting   : {} (\"{}\", id {})
                       Decide with: syncix bind --studio  (write this place into the folder)
                       or:          syncix bind --disk    (load the folder into this place)",
                    current_value, incoming_place, item_name, pid
                );

                if let Ok(mut c) = ctx.state.place_clash_state.lock() {
                    *c = Some(crate::server::PlaceConflict {
                        folder_place: current_value,
                        incoming_place,
                        incoming_name: item_name,
                        incoming_place_id: pid,
                    });
                }
                // Do NOT touch the model and stop every direction: until a decision is made
                // both sides must stay as they are.
                crate::project::set_sync_suspended(true);
                return;
            }
        }
    }

    tracing::info!("Received FULL_SYNC (bootstrap) from Studio. Synchronising state...");
    if let Some(instances) = payload.data.get("instances").and_then(|p| p.as_array()) {
        let mut added_count = 0;
        let mut ws_nodes = Vec::new();
        
        {
            let mut dm = ctx.data_model.write().await;
            // Recovery: when a FULL_SYNC arrives the old state is discarded completely.
            // That way objects deleted on the Studio side do not linger in memory.
            let previous = std::mem::replace(&mut *dm, crate::model::DataModel::new());
            for node_data in instances {
                if let (Some(class_name), Some(name), Some(syncix_id)) = (
                    node_data.get("class_name").and_then(|v| v.as_str()),
                    node_data.get("name").and_then(|v| v.as_str()),
                    node_data.get("syncix_id").and_then(|v| v.as_str()),
                ) {
                    // Classes the user excluded never enter the model.
                    //
                    // The plugin applies the same filter; this second gate is there so the
                    // setting still applies when an older plugin connects. If either
                    // side failed to apply the setting, you would get
                    // "I excluded it but it still comes".
                    if !ctx.cfg.class_allowed(class_name) {
                        continue;
                    }
                    if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                        let mut instance = InstanceNode::new(class_name, name);
                        instance.syncix_id = uuid;
                        
                        // Parent ID logic
                        if let Some(parent_str) = node_data.get("parent").and_then(|v| v.as_str()) {
                            if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                instance.parent = Some(parent_uuid);
                            }
                        }

                        // Script source code
                        if let Some(src) = node_data.get("source").and_then(|v| v.as_str()) {
                            instance.source = Some(src.to_string());
                        }

                        // Attributes
                        if let Some(attrs) = node_data.get("attributes").and_then(|v| v.as_object()) {
                            for (k, val) in attrs {
                                if let Some(pv) = parse_wire_value(val) {
                                    instance.attributes.insert(k.clone(), pv);
                                }
                            }
                        }

                        // CollectionService tags
                        if let Some(t) = node_data.get("tags").and_then(|v| v.as_array()) {
                            instance.tags = t
                                .iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect();
                        }

                        // Properties (wide scope — generic properties object)
                        if let Some(props) = node_data.get("properties").and_then(|v| v.as_object()) {
                            for (k, val) in props {
                                if !ctx.cfg.property_allowed(k) {
                                    continue;
                                }
                                if let Some(pv) = parse_wire_value(val) {
                                    instance.properties.insert(k.clone(), pv);
                                }
                            }
                        }

                        if let Err(e) = dm.upsert_instance(instance.clone()) {
                            tracing::warn!("FULL_SYNC upsert failed ({}): {}", instance.name, e);
                            continue;
                        }
                        added_count += 1;
                        // Disk writing is done by the central debounced writer (layout).

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
            }

            // Studio's tree holds only what Studio has applied. What the core created
            // that is still on its way (queued, in a poll reply, or parked in the
            // plugin until its parent exists) was dropped by the rebuild above; its
            // files went to the trash and it came back later as a duplicate. It is
            // put back. What Studio did receive and still lacks, it deleted or
            // refused, and that stays gone.
            let applied = if payload.data.get("applied_epoch").and_then(|v| v.as_str())
                == Some(ctx.studio_outbox.epoch())
            {
                // Read leniently: Studio's JSON encoder writes every number as a double.
                payload.data.get("applied_seq").and_then(|v| {
                    v.as_u64()
                        .or_else(|| v.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
                })
            } else {
                None
            };
            let mut in_flight = ctx.studio_outbox.in_flight(applied);
            if let Some(waiting) = payload.data.get("waiting_ids").and_then(|v| v.as_array()) {
                in_flight.creates.extend(
                    waiting
                        .iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|s| uuid::Uuid::parse_str(s).ok()),
                );
            }
            let kept = dm.carry_over(&previous, &in_flight);
            // Delivered but never applied (a lost poll reply, or dropped while sync
            // was paused): nothing would deliver it again, so the model would keep
            // what Studio never gets. It is sent once more; the plugin applies a
            // create or change it already has without harm.
            if let Some(applied) = applied {
                let delivered = ctx.studio_outbox.delivered_since(applied);
                for resend in resend_payloads(&dm, &kept, &delivered) {
                    ctx.studio_outbox.push(resend);
                }
            }
            ctx.studio_outbox.settle(applied);
            for id in &kept {
                if let Some(instance) = dm.get_instance(id) {
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
            if !kept.is_empty() {
                tracing::info!("FULL_SYNC: kept {} instance(s) Studio has not applied yet.", kept.len());
            }
        }

        tracing::info!("FULL_SYNC complete. {} instances added or updated.", added_count);
        // The model is now Studio's tree: the reconciler may delete extras.
        ctx.model_authoritative.store(true, std::sync::atomic::Ordering::SeqCst);
        
        // Notify VS Code with FULL_SYNC
        let ws_msg = serde_json::json!({
            "event_type": "FULL_SYNC",
            "data": {
                "nodes": ws_nodes
            }
        });
        let _ = ctx.state.tx_to_vscode.send(ws_msg.to_string());
    }
}
