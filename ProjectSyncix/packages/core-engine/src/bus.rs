use crate::model::{InstanceNode, InstancePatch};
use tokio::sync::broadcast;

/// Central event bus of the system.
/// Every component (Watcher, Transport, DataModel) only posts messages here or reads them from here.
/// Components never call each other directly (decoupling).
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Full overwrite of an object, coming from the file system or from Studio.
    FullNodeUpdate(InstanceNode),

    /// Patch that changes only specific properties (incremental sync).
    PatchUpdate(InstancePatch),

    /// Signals that an object was deleted.
    NodeDeleted(uuid::Uuid),
}

/// Event bus shared by the whole system.
pub struct EventBus {
    sender: broadcast::Sender<SyncEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        // Capacity of 1024 messages (enough for batching and bursts).
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    /// Publishes an event to the system.
    pub fn publish(
        &self,
        event: SyncEvent,
    ) -> Result<usize, Box<broadcast::error::SendError<SyncEvent>>> {
        self.sender.send(event).map_err(Box::new)
    }

    /// Creates a receiver (subscriber) for listening to events.
    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.sender.subscribe()
    }
}
