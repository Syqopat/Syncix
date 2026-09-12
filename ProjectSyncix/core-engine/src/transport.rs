use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use uuid::Uuid;

/// Standard schema of every message in the system.
/// Versioned, so older clients keep working when a v2 arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub version: String, // e.g. "v1"
    pub event_type: EventType,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// Connection test between server and client
    Ping,
    Pong,
    /// Message pushed to Studio when a file changes in VS Code
    PushUpdate,
    /// Message sent to VS Code (the core) when an object changes in Studio
    ClientUpdate,
    /// When a new file is created
    PushCreate,

    CompositeUpdate,
    PropertyUpdate,
    Create,
    Destroy,
    FullSync,
    GetTree,

    /// Message in which the core asks Studio to send the whole tree AGAIN.
    ///
    /// Why it is needed: during verification "the core's model" and "Studio's real
    /// state" could be confused. The model is already updated when the command is
    /// sent, so reading the model does not prove that the command REACHED Studio.
    /// This message makes Studio speak; its reply is the single source of truth.
    FullSyncRequest,

    /// Message in which the Studio plugin reports its own counters
    /// (so BatchQueue merging can be measured).
    PluginMetrics,

    /// Commands from the VS Code Explorer (VS Code -> Rust -> Studio direction)
    CreateInstance,
    RenameInstance,
    DeleteInstance,
    /// The complete set of CollectionService tags. Sent as a whole list rather than
    /// single add/remove operations; it avoids keeping separate state on both sides.
    SetTags,
    /// Which objects are selected. NOT written to the model or to disk: selection is momentary
    /// state, not project content. Written to disk, every click would change a file
    /// and version control would fill with noise.
    Selection,
    ReparentInstance,
    SetProperty,
    SetAttribute,
}

/// What payloads ask Studio to create or change: the part of the core's model Studio
/// may not have yet.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InFlight {
    pub creates: HashSet<Uuid>,
    /// Property changes; a rename is recorded as "Name", a script edit as "Source".
    pub properties: HashSet<(Uuid, String)>,
    pub reparents: HashSet<Uuid>,
    pub destroys: HashSet<Uuid>,
    pub attributes: HashSet<(Uuid, String)>,
    pub tags: HashSet<Uuid>,
}

impl InFlight {
    fn is_empty(&self) -> bool {
        self.creates.is_empty()
            && self.properties.is_empty()
            && self.reparents.is_empty()
            && self.destroys.is_empty()
            && self.attributes.is_empty()
            && self.tags.is_empty()
    }

    fn extend(&mut self, other: &InFlight) {
        self.creates.extend(other.creates.iter().copied());
        self.properties.extend(other.properties.iter().cloned());
        self.reparents.extend(other.reparents.iter().copied());
        self.destroys.extend(other.destroys.iter().copied());
        self.attributes.extend(other.attributes.iter().cloned());
        self.tags.extend(other.tags.iter().copied());
    }
}

/// Records what one payload creates or changes in Studio. Every kind of change the core
/// applies to its model before Studio does is recorded: tracking only creates and
/// property changes let a FULL_SYNC quietly undo renames, moves, deletions, attributes
/// and tags that were still on their way.
fn record_touched(payload: &Payload, into: &mut InFlight) {
    let id_of = |v: &serde_json::Value| {
        v.get("syncix_id")
            .or_else(|| v.get("id"))
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    match payload.event_type {
        EventType::CompositeUpdate => {
            let Some(patches) = payload.data.get("patches").and_then(|x| x.as_array()) else {
                return;
            };
            for patch in patches {
                let Some(data) = patch.get("data") else { continue };
                let Some(id) = id_of(data) else { continue };
                match patch.get("event_type").and_then(|x| x.as_str()) {
                    Some("CREATE") => {
                        into.creates.insert(id);
                    }
                    Some("PROPERTY_UPDATE") => {
                        if let Some(property) = data.get("property").and_then(|x| x.as_str()) {
                            into.properties.insert((id, property.to_string()));
                        }
                    }
                    Some("RENAME_INSTANCE") | Some("RENAME") => {
                        into.properties.insert((id, "Name".to_string()));
                    }
                    Some("REPARENT") => {
                        into.reparents.insert(id);
                    }
                    Some("DESTROY") => {
                        into.destroys.insert(id);
                    }
                    Some("ATTRIBUTE_UPDATE") => {
                        if let Some(name) = data.get("name").and_then(|x| x.as_str()) {
                            into.attributes.insert((id, name.to_string()));
                        }
                    }
                    Some("TAGS_UPDATE") => {
                        into.tags.insert(id);
                    }
                    _ => {}
                }
            }
        }
        // A whole node sent from disk: Studio creates it if it does not have it yet.
        EventType::PushUpdate => {
            if let Some(id) = id_of(&payload.data) {
                into.creates.insert(id);
            }
        }
        _ => {}
    }
}

/// How many delivered payloads are remembered while Studio has not confirmed them.
const SENT_LOG_LIMIT: usize = 20_000;

/// Lossless delivery queue for messages going to Studio.
/// Why: a broadcast channel only delivers to subscribers waiting at that moment;
/// messages sent while Studio is between two polls would be lost. This queue keeps
/// a message in memory until the next poll.
///
/// It also numbers every message. The core's model runs ahead of Studio: an instance
/// is in the model as soon as its CREATE is queued. When Studio's tree arrives
/// (FULL_SYNC) the model is rebuilt from it, and whatever Studio had not applied yet
/// used to be dropped, its files trashed, only to come back later as a duplicate.
/// With the numbers the plugin can say how far it got, and the core keeps the rest.
pub struct StudioOutbox {
    queue: Mutex<VecDeque<Payload>>,
    notify: tokio::sync::Notify,
    /// The number of the last queued message; each message carries its own as data._seq.
    seq: AtomicU64,
    /// Identifies this core process (data._epoch). Numbers from an earlier core, which
    /// the plugin may still hold after a restart, mean nothing to this one.
    epoch: String,
    /// Messages Studio has been handed but has not confirmed, with what they touch.
    sent: Mutex<VecDeque<(u64, InFlight)>>,
}

impl StudioOutbox {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            notify: tokio::sync::Notify::new(),
            seq: AtomicU64::new(0),
            epoch: Uuid::new_v4().to_string(),
            sent: Mutex::new(VecDeque::new()),
        }
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// Synchronous push: callable from both async and blocking (file watcher thread) contexts.
    pub fn push(&self, mut payload: Payload) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(data) = payload.data.as_object_mut() {
            data.insert("_seq".into(), serde_json::json!(seq));
            data.insert("_epoch".into(), serde_json::json!(self.epoch));
        }
        self.queue.lock().unwrap().push_back(payload);
        self.notify.notify_one();
    }

    /// Takes the next message without waiting, remembering what it touches until
    /// Studio confirms it.
    pub fn try_pop(&self) -> Option<Payload> {
        let payload = self.queue.lock().unwrap().pop_front()?;
        let mut touched = InFlight::default();
        record_touched(&payload, &mut touched);
        if !touched.is_empty() {
            let seq = payload.data.get("_seq").and_then(|v| v.as_u64()).unwrap_or(0);
            let mut sent = self.sent.lock().unwrap();
            sent.push_back((seq, touched));
            while sent.len() > SENT_LOG_LIMIT {
                sent.pop_front();
            }
        }
        Some(payload)
    }

    /// Returns at once if the queue has a message; otherwise waits for the timeout.
    pub async fn pop_or_wait(&self, timeout: std::time::Duration) -> Option<Payload> {
        if let Some(p) = self.try_pop() {
            return Some(p);
        }
        let _ = tokio::time::timeout(timeout, self.notify.notified()).await;
        self.try_pop()
    }

    /// What Studio may not have applied yet: everything still queued, plus what was
    /// delivered after `applied`, the last number the plugin confirmed for this core.
    /// Without a confirmation (an older plugin, or one still counting for an earlier
    /// core) only the queue counts: whether a delivered message was applied is unknown.
    pub fn in_flight(&self, applied: Option<u64>) -> InFlight {
        let mut out = InFlight::default();
        for payload in self.queue.lock().unwrap().iter() {
            record_touched(payload, &mut out);
        }
        if let Some(applied) = applied {
            out.extend(&self.delivered_since(applied));
        }
        out
    }

    /// What Studio was handed after `applied` but has not confirmed: a lost poll reply,
    /// or messages dropped while sync was paused. Unlike what is still queued, nothing
    /// will deliver these again on its own.
    pub fn delivered_since(&self, applied: u64) -> InFlight {
        let mut out = InFlight::default();
        for (seq, touched) in self.sent.lock().unwrap().iter() {
            if *seq > applied {
                out.extend(touched);
            }
        }
        out
    }

    /// After a FULL_SYNC: messages up to `applied` are settled, Studio's tree shows what
    /// became of them. Without a confirmation every delivered one is.
    pub fn settle(&self, applied: Option<u64>) {
        let mut sent = self.sent.lock().unwrap();
        match applied {
            Some(applied) => sent.retain(|(seq, _)| *seq > applied),
            None => sent.clear(),
        }
    }
}

/// How long the parts of one FULL_SYNC may take to arrive before they are dropped.
const PART_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

struct PartialSync {
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

/// Interface (trait) of the transport layer.
/// Why: this interface separates business logic (the core engine) from the transport
/// (WebSocket/HTTP). The core only calls these functions and does not know what runs underneath.
#[async_trait]
pub trait Transport {
    /// Starts the communication channel (e.g. starts listening on the WebSocket server).
    async fn start(&self) -> Result<(), String>;

    /// Sends a message to every connected client (Studio instance) (broadcast).
    async fn broadcast(&self, payload: &Payload) -> Result<(), String>;

    // Provides a channel (receiver) for listening to messages from clients.
    // (A real implementation would use tokio::sync::mpsc::Receiver.)
    // async fn receive(&self) -> Receiver<Payload>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(id: Uuid) -> Payload {
        Payload {
            version: "v1".into(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": [{ "event_type": "CREATE", "data": { "syncix_id": id } }] }),
        }
    }

    fn set_property(id: Uuid, property: &str) -> Payload {
        Payload {
            version: "v1".into(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": [{
                "event_type": "PROPERTY_UPDATE",
                "data": { "syncix_id": id, "property": property, "value": 1 }
            }] }),
        }
    }

    #[test]
    fn push_numbers_every_message() {
        let outbox = StudioOutbox::new();
        outbox.push(create(Uuid::new_v4()));
        outbox.push(create(Uuid::new_v4()));
        let first = outbox.try_pop().unwrap();
        let second = outbox.try_pop().unwrap();
        assert_eq!(first.data["_seq"], 1);
        assert_eq!(second.data["_seq"], 2);
        assert_eq!(first.data["_epoch"].as_str(), Some(outbox.epoch()));
    }

    #[test]
    fn queued_and_unconfirmed_creates_are_in_flight() {
        let outbox = StudioOutbox::new();
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        outbox.push(create(a));
        outbox.push(create(b));
        outbox.push(create(c));
        outbox.try_pop(); // a, number 1
        outbox.try_pop(); // b, number 2; c is still queued

        // The plugin confirmed number 1: a is settled, b was delivered but not applied.
        let in_flight = outbox.in_flight(Some(1));
        assert!(!in_flight.creates.contains(&a));
        assert!(in_flight.creates.contains(&b));
        assert!(in_flight.creates.contains(&c));

        outbox.settle(Some(2));
        assert!(!outbox.in_flight(Some(2)).creates.contains(&b));
    }

    #[test]
    fn without_a_confirmation_only_the_queue_counts() {
        let outbox = StudioOutbox::new();
        let (delivered, queued) = (Uuid::new_v4(), Uuid::new_v4());
        outbox.push(create(delivered));
        outbox.push(create(queued));
        outbox.try_pop();
        assert_eq!(outbox.in_flight(None).creates, [queued].into_iter().collect());
        outbox.settle(None);
        assert!(!outbox.in_flight(Some(0)).creates.contains(&delivered));
    }

    fn patch(kind: &str, data: serde_json::Value) -> Payload {
        Payload {
            version: "v1".into(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": [{ "event_type": kind, "data": data }] }),
        }
    }

    #[test]
    fn every_kind_of_change_is_tracked() {
        let outbox = StudioOutbox::new();
        let id = Uuid::new_v4();
        outbox.push(patch("RENAME_INSTANCE", serde_json::json!({ "id": id, "newName": "New" })));
        outbox.push(patch("REPARENT", serde_json::json!({ "syncix_id": id, "parent": Uuid::new_v4() })));
        outbox.push(patch("DESTROY", serde_json::json!({ "syncix_id": id })));
        outbox.push(patch("ATTRIBUTE_UPDATE", serde_json::json!({ "syncix_id": id, "name": "Level", "value": 3 })));
        outbox.push(patch("TAGS_UPDATE", serde_json::json!({ "syncix_id": id, "tags": ["A"] })));
        let in_flight = outbox.in_flight(None);
        assert!(in_flight.properties.contains(&(id, "Name".to_string())));
        assert!(in_flight.reparents.contains(&id));
        assert!(in_flight.destroys.contains(&id));
        assert!(in_flight.attributes.contains(&(id, "Level".to_string())));
        assert!(in_flight.tags.contains(&id));
    }

    #[test]
    fn delivered_since_leaves_out_what_is_still_queued() {
        let outbox = StudioOutbox::new();
        let (delivered, queued) = (Uuid::new_v4(), Uuid::new_v4());
        outbox.push(create(delivered));
        outbox.push(create(queued));
        outbox.try_pop();
        let since = outbox.delivered_since(0);
        assert!(since.creates.contains(&delivered));
        assert!(!since.creates.contains(&queued));
        assert!(outbox.delivered_since(1).creates.is_empty());
    }

    fn full_sync_part(id: &str, index: usize, count: usize, names: &[&str]) -> Payload {
        let instances: Vec<serde_json::Value> = names.iter().map(|n| serde_json::json!({ "name": n })).collect();
        Payload {
            version: "v1".into(),
            event_type: EventType::FullSync,
            data: serde_json::json!({
                "instances": instances, "place_key": "p",
                "part_id": id, "part_index": index, "part_count": count
            }),
        }
    }

    #[test]
    fn full_sync_parts_are_put_back_together_in_order() {
        let mut assembler = FullSyncAssembler::new();
        assert!(assembler.accept(full_sync_part("a", 2, 3, &["c", "d"])).is_none());
        assert!(assembler.accept(full_sync_part("a", 1, 3, &["a", "b"])).is_none());
        let whole = assembler.accept(full_sync_part("a", 3, 3, &["e"])).unwrap();
        let names: Vec<&str> = whole.data["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["a", "b", "c", "d", "e"]);
        assert_eq!(whole.data["place_key"], "p");
        assert!(whole.data.get("part_id").is_none());
    }

    #[test]
    fn a_whole_full_sync_and_other_messages_pass_straight_through() {
        let mut assembler = FullSyncAssembler::new();
        let whole = Payload {
            version: "v1".into(),
            event_type: EventType::FullSync,
            data: serde_json::json!({ "instances": [] }),
        };
        assert!(assembler.accept(whole).is_some());
        assert!(assembler.accept(create(Uuid::new_v4())).is_some());
        // A malformed part is dropped rather than handled as a whole tree.
        assert!(assembler.accept(full_sync_part("b", 5, 3, &["x"])).is_none());
    }

    #[test]
    fn property_changes_are_tracked() {
        let outbox = StudioOutbox::new();
        let id = Uuid::new_v4();
        outbox.push(set_property(id, "Transparency"));
        outbox.try_pop();
        let in_flight = outbox.in_flight(Some(0));
        assert!(in_flight.properties.contains(&(id, "Transparency".to_string())));
        assert!(in_flight.creates.is_empty());
    }
}
