use crate::model::InstanceNode;
use std::collections::VecDeque;
use std::sync::RwLock;

/// Snapshot manager
/// Takes deep copies of the given DataModel (root InstanceNode) at intervals.
/// Keeps past copies in a bounded queue (for undo/redo and crash recovery).
pub struct SnapshotManager {
    /// Undo history (older versions)
    history: RwLock<VecDeque<InstanceNode>>,

    /// Redo history (versions stepped back from)
    future: RwLock<VecDeque<InstanceNode>>,

    /// Maximum number of snapshots kept
    max_snapshots: usize,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(max_snapshots)),
            future: RwLock::new(VecDeque::new()),
            max_snapshots,
        }
    }

    /// Stores the current DataModel tree as a snapshot.
    pub fn take_snapshot(&self, current_root: &InstanceNode) {
        let mut history = self.history.write().unwrap();

        // If capacity is reached, drop the oldest
        if history.len() >= self.max_snapshots {
            history.pop_front();
        }

        // Take a deep clone of the tree and store it
        history.push_back(current_root.clone());

        // A new change clears the redo queue
        self.future.write().unwrap().clear();
    }

    /// Returns to the previous snapshot (undo)
    pub fn undo(&self, current_root: &InstanceNode) -> Option<InstanceNode> {
        let mut history = self.history.write().unwrap();
        if let Some(previous_state) = history.pop_back() {
            // Push the current state to the future queue
            let mut future = self.future.write().unwrap();
            future.push_front(current_root.clone());

            return Some(previous_state);
        }
        None
    }

    /// Brings back a snapshot that was stepped back from (redo)
    pub fn redo(&self, current_root: &InstanceNode) -> Option<InstanceNode> {
        let mut future = self.future.write().unwrap();
        if let Some(next_state) = future.pop_front() {
            // Push the current state to the history queue
            let mut history = self.history.write().unwrap();
            history.push_back(current_root.clone());

            return Some(next_state);
        }
        None
    }

    /// Returns how many snapshots are stored.
    pub fn history_count(&self) -> usize {
        self.history.read().unwrap().len()
    }
}
