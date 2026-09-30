//! Split out of transport.rs.

#[allow(unused_imports)]
use super::*;

/// How long the parts of one FULL_SYNC may take to arrive before they are dropped.
pub(crate) const PART_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub(crate) struct PartialSync {
    parts: Vec<Option<serde_json::Value>>,
    started: std::time::Instant,
}

/// Studio refuses to POST more than 1024 KB. A big place's tree passed that, the plugin
/// took the refusal for a lost connection and never finished connecting. The tree now
/// comes in parts (data.part_id, part_index from 1, part_count); they are put back
/// together here and handled as one FULL_SYNC. Parts may arrive in any order.
#[derive(Default)]
pub struct FullSyncAssembler {
    pending: HashMap<String, PartialSync>,
}

impl FullSyncAssembler {
    pub fn new() -> Self {
        Self { pending: HashMap::new() }
    }

    /// What to handle now: any other message as it is, the whole tree once its last part
    /// is in, None while parts are still missing.
    pub fn accept(&mut self, payload: Payload) -> Option<Payload> {
        if payload.event_type != EventType::FullSync {
            return Some(payload);
        }
        let Some(id) = payload.data.get("part_id").and_then(|v| v.as_str()).map(str::to_string) else {
            return Some(payload);
        };
        let count = payload.data.get("part_count").and_then(|v| v.as_f64()).unwrap_or(0.0) as usize;
        let index = payload.data.get("part_index").and_then(|v| v.as_f64()).unwrap_or(0.0) as usize;
        if count == 0 || count > 10_000 || index == 0 || index > count {
            tracing::warn!("FULL_SYNC part {} of {} for {} is malformed; ignored.", index, count, id);
            return None;
        }

        self.pending.retain(|_, p| p.started.elapsed() < PART_TIMEOUT);
        let entry = self.pending.entry(id.clone()).or_insert_with(|| PartialSync {
            parts: vec![None; count],
            started: std::time::Instant::now(),
        });
        if entry.parts.len() != count {
            return None;
        }
        entry.parts[index - 1] = Some(payload.data);
        if entry.parts.iter().any(|p| p.is_none()) {
            return None;
        }

        let parts: Vec<serde_json::Value> = self.pending.remove(&id)?.parts.into_iter().flatten().collect();
        let mut data = parts[0].clone();
        let instances: Vec<serde_json::Value> = parts
            .iter()
            .filter_map(|p| p.get("instances").and_then(|v| v.as_array()))
            .flatten()
            .cloned()
            .collect();
        if let Some(object) = data.as_object_mut() {
            object.insert("instances".into(), serde_json::Value::Array(instances));
            for key in ["part_id", "part_index", "part_count"] {
                object.remove(key);
            }
        }
        Some(Payload {
            version: payload.version,
            event_type: EventType::FullSync,
            data,
        })
    }
}
