//! What the core does with a SetTags message from Studio.

use crate::runtime::Ctx;
use crate::transport::{EventType, Payload};
use crate::values::*;

/// Applies one SetTags message.
pub(crate) async fn handle(ctx: &Ctx, payload: &Payload) {
    let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let mut tag_list: Vec<String> = payload
        .data
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Sorted and deduplicated: both sides see the same list in the same order,
    // otherwise every comparison says "changed" and produces needless patches.
    tag_list.sort();
    tag_list.dedup();

    let resolved = {
        let dm = ctx.data_model.read().await;
        resolve_id(&dm, id)
    };

    if let Some(uuid) = resolved {
        let was_applied = {
            let mut dm = ctx.data_model.write().await;
            match dm.get_mut_instance(&uuid) {
                Some(inst) => {
                    inst.tags = tag_list.clone();
                    inst.last_updated = chrono::Utc::now().timestamp_millis();
                    true
                }
                None => false,
            }
        };
        if was_applied {
            ctx.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::CompositeUpdate,
                data: serde_json::json!({
                    "patches": [{
                        "event_type": "TAGS_UPDATE",
                        "data": { "syncix_id": uuid, "tags": tag_list }
                    }]
                }),
            });
            tracing::info!("SET_TAGS handled: {} -> {:?}", id, tag_list);
        }
    } else {
        tracing::warn!("SET_TAGS target not found: {}", id);
    }
}
