mod full_sync;
mod in_flight;
mod outbox;

pub(crate) use full_sync::*;
pub(crate) use in_flight::*;
pub(crate) use outbox::*;

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

