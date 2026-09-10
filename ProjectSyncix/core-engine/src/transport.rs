use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

/// Lossless delivery queue for messages going to Studio.
/// Why: a broadcast channel only delivers to subscribers waiting at that moment;
/// messages sent while Studio is between two polls would be lost. This queue keeps
/// a message in memory until the next poll.
pub struct StudioOutbox {
    queue: std::sync::Mutex<std::collections::VecDeque<Payload>>,
    notify: tokio::sync::Notify,
}

impl StudioOutbox {
    pub fn new() -> Self {
        Self {
            queue: std::sync::Mutex::new(std::collections::VecDeque::new()),
            notify: tokio::sync::Notify::new(),
        }
    }

    /// Synchronous push: callable from both async and blocking (file watcher thread) contexts.
    pub fn push(&self, payload: Payload) {
        self.queue.lock().unwrap().push_back(payload);
        self.notify.notify_one();
    }

    fn pop(&self) -> Option<Payload> {
        self.queue.lock().unwrap().pop_front()
    }

    /// Returns at once if the queue has a message; otherwise waits for the timeout.
    pub async fn pop_or_wait(&self, timeout: std::time::Duration) -> Option<Payload> {
        if let Some(p) = self.pop() {
            return Some(p);
        }
        let _ = tokio::time::timeout(timeout, self.notify.notified()).await;
        self.pop()
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
