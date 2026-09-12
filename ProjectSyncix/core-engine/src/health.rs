use axum::Json;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant as StdInstant};
use tokio::time::Instant;

/// /health response. It no longer just says "I am up"; the core introduces itself.
/// The Studio plugin reads it to show the user which project it connected to,
/// check version compatibility and find the right port.
/// Settings from syncix.toml that shape the Studio plugin's behaviour.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PluginConfig {
    /// "two_way" | "studio_to_disk" | "disk_to_studio" | "manual"
    pub mode: String,
    /// Whether Studio asks for permission on the first connection.
    pub ask_permission: bool,
    /// Whether Syncix's changes go onto Studio's undo stack.
    pub undo: bool,
    /// Services to observe. If empty, the plugin's default list applies.
    pub services: Vec<String>,
    /// Classes that are never observed.
    pub ignore_classes: Vec<String>,
    /// Properties that are never observed.
    pub ignore_properties: Vec<String>,
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,

    // --- Kimlik ---
    pub version: String,
    pub protocol: u32,
    /// What this core can do beyond the protocol, so a newer plugin can check before
    /// relying on it ("full_sync_parts": a FULL_SYNC may come in parts).
    pub features: Vec<&'static str>,
    pub project: String,
    pub root: String,
    pub port: u16,

    // --- Status ---
    pub uptime_seconds: u64,
    pub active_connections: usize,
    pub messages_processed: usize,
    pub studio_connected: bool,

    // --- Metrics (make behaviour that could not be verified measurable) ---
    pub inbound_from_studio: usize,
    pub outbound_to_studio: usize,
    pub loops_detected: usize,
    pub plugin_queued: usize,
    pub plugin_coalesced: usize,

    /// Which place is this folder bound to? None if it was never bound.
    ///
    /// The plugin checks this while scanning ports: it prefers a core bound to its own place
    /// and skips a core bound to another place. Otherwise, with two projects open, it
    /// connected to the FIRST core it found and raised a needless place conflict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_place: Option<String>,

    /// Set when the folder is bound to another place; that means sync is suspended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_conflict: Option<crate::server::PlaceConflict>,
    /// Whether sync is suspended (currently only happens on a place conflict).
    pub sync_suspended: bool,

    /// Settings the plugin applies.
    ///
    /// The plugin does not keep them itself; syncix.toml is the single source of truth.
    /// Otherwise there would be two sets of settings for the same project and it would be
    /// unclear which one applies. The plugin reads /health when it connects, so
    /// no extra channel is needed.
    pub config: PluginConfig,

    // --- Plugin activity log summary ---
    // The only way to see, from outside, the log held in the plugin's memory.
    /// Number of objects in the model. The editor's status bar shows it; it used to
    /// be estimated by counting messages and really did drift over time.
    pub object_count: usize,

    pub activity_total: usize,
    pub activity_in: usize,
    pub activity_out: usize,
    pub conflicts: usize,
}

/// Echo loop detector.
///
/// Why it exists: echo suppression is not perfect because of Roblox's deferred signals.
/// When a loop formed it left no trace anywhere, so it silently burned CPU.
/// This counts how often the same (uuid, property) pair bounces back and forth in a short time
/// and prints one warning once a threshold is crossed. Like the warning that revealed the
/// colour bug in PatchExecutor, it makes the invisible visible.
pub struct LoopDetector {
    /// (uuid, property) -> timestamps seen in the window
    seen: Mutex<HashMap<(String, String), Vec<StdInstant>>>,
    /// Suppressed entries, so a crossed threshold does not warn again and again
    suppressed: Mutex<HashMap<(String, String), StdInstant>>,
    time_window: Duration,
    threshold: usize,
    counter: AtomicUsize,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            suppressed: Mutex::new(HashMap::new()),
            // More than 12 round trips of the same property within 2 seconds is not a normal
            // user edit; even a drag arrives at this rate
            // already merged by BatchQueue.
            time_window: Duration::from_secs(2),
            threshold: 12,
            counter: AtomicUsize::new(0),
        }
    }

    /// Records a property change. Returns true if a loop is suspected.
    pub fn persist(&self, uuid: &str, property: &str) -> bool {
        let key_name = (uuid.to_string(), property.to_string());
        let current_time = StdInstant::now();

        // If a warning was already given, stay quiet for 30 seconds.
        {
            let mut suppressed = self.suppressed.lock().unwrap();
            if let Some(t) = suppressed.get(&key_name) {
                if current_time.duration_since(*t) < Duration::from_secs(30) {
                    return false;
                }
                suppressed.remove(&key_name);
            }
        }

        let mut seen = self.seen.lock().unwrap();
        let entry_list = seen.entry(key_name.clone()).or_default();
        entry_list.retain(|t| current_time.duration_since(*t) < self.time_window);
        entry_list.push(current_time);

        if entry_list.len() > self.threshold {
            entry_list.clear();
            drop(seen);
            self.suppressed.lock().unwrap().insert(key_name, current_time);
            self.counter.fetch_add(1, Ordering::SeqCst);
            return true;
        }

        // Keep memory bounded: when the map grows, drop stale entries.
        if seen.len() > 512 {
            seen.retain(|_, v| {
                v.retain(|t| current_time.duration_since(*t) < self.time_window);
                !v.is_empty()
            });
        }

        false
    }

    pub fn total_count(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HealthMonitor {
    start_time: Instant,
    active_connections: AtomicUsize,
    messages_processed: AtomicUsize,

    inbound_from_studio: AtomicUsize,
    outbound_to_studio: AtomicUsize,
    plugin_queued: AtomicUsize,
    plugin_coalesced: AtomicUsize,
    activity_total: AtomicUsize,
    activity_in: AtomicUsize,
    activity_out: AtomicUsize,
    conflicts: AtomicUsize,

    /// When Studio last polled. This is how we know whether it is connected.
    last_studio_contact: Mutex<Option<StdInstant>>,

    pub loop_detector: LoopDetector,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            active_connections: AtomicUsize::new(0),
            messages_processed: AtomicUsize::new(0),
            inbound_from_studio: AtomicUsize::new(0),
            outbound_to_studio: AtomicUsize::new(0),
            plugin_queued: AtomicUsize::new(0),
            plugin_coalesced: AtomicUsize::new(0),
            activity_total: AtomicUsize::new(0),
            activity_in: AtomicUsize::new(0),
            activity_out: AtomicUsize::new(0),
            conflicts: AtomicUsize::new(0),
            last_studio_contact: Mutex::new(None),
            loop_detector: LoopDetector::new(),
        }
    }

    pub fn add_connection(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }

    pub fn remove_connection(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn inc_messages(&self) {
        self.messages_processed.fetch_add(1, Ordering::SeqCst);
    }

    pub fn inc_inbound(&self) {
        self.inbound_from_studio.fetch_add(1, Ordering::SeqCst);
    }

    pub fn inc_outbound(&self) {
        self.outbound_to_studio.fetch_add(1, Ordering::SeqCst);
    }

    /// The Studio plugin reports its own BatchQueue counters.
    /// Only this way can we measure whether merging really works:
    /// queued is how many changes came in, coalesced is how many were merged into one message.
    pub fn set_plugin_metrics(&self, queued: usize, coalesced: usize) {
        self.plugin_queued.store(queued, Ordering::SeqCst);
        self.plugin_coalesced.store(coalesced, Ordering::SeqCst);
    }

    /// Summary of the plugin's activity log.
    pub fn set_activity(&self, total_count: usize, received: usize, outgoing: usize, conflict: usize) {
        self.activity_total.store(total_count, Ordering::SeqCst);
        self.activity_in.store(received, Ordering::SeqCst);
        self.activity_out.store(outgoing, Ordering::SeqCst);
        self.conflicts.store(conflict, Ordering::SeqCst);
    }

    pub fn touch_studio(&self) {
        *self.last_studio_contact.lock().unwrap() = Some(StdInstant::now());
    }

    pub fn studio_connected(&self) -> bool {
        self.last_studio_contact
            .lock()
            .unwrap()
            .map(|t| t.elapsed() < Duration::from_secs(30))
            .unwrap_or(false)
    }

    pub fn get_status_with_count(
        &self,
        project: &crate::project::ProjectConfig,
        port: u16,
        object_count: usize,
    ) -> HealthStatus {
        let mut s = self.get_status(project, port);
        s.object_count = object_count;
        s
    }

    pub fn get_status(&self, project: &crate::project::ProjectConfig, port: u16) -> HealthStatus {
        HealthStatus {
            status: "Healthy".to_string(),
            version: crate::project::VERSION.to_string(),
            protocol: crate::project::PROTOCOL_VERSION,
            features: vec!["full_sync_parts"],
            project: project.name.clone(),
            root: project.root.to_string_lossy().to_string(),
            port,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections: self.active_connections.load(Ordering::SeqCst),
            messages_processed: self.messages_processed.load(Ordering::SeqCst),
            studio_connected: self.studio_connected(),
            inbound_from_studio: self.inbound_from_studio.load(Ordering::SeqCst),
            outbound_to_studio: self.outbound_to_studio.load(Ordering::SeqCst),
            loops_detected: self.loop_detector.total_count(),
            plugin_queued: self.plugin_queued.load(Ordering::SeqCst),
            plugin_coalesced: self.plugin_coalesced.load(Ordering::SeqCst),
            object_count: 0,
            bound_place: None,
            place_conflict: None,
            sync_suspended: false,
            config: PluginConfig {
                mode: project.mode_value.name_of().to_string(),
                ask_permission: project.prompt_permission,
                undo: project.restore_cmd,
                services: project.scope_settings.service_list.clone(),
                ignore_classes: project.scope_settings.class_ignore_list.clone(),
                ignore_properties: project.scope_settings.property_ignore_list.clone(),
            },
            activity_total: self.activity_total.load(Ordering::SeqCst),
            activity_in: self.activity_in.load(Ordering::SeqCst),
            activity_out: self.activity_out.load(Ordering::SeqCst),
            conflicts: self.conflicts.load(Ordering::SeqCst),
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn health_handler(
    axum::extract::State(state): axum::extract::State<Arc<crate::server::AppState>>,
) -> Json<HealthStatus> {
    let number_value = state.data_model.read().await.get_all_instances().len();
    let mut status_info = state
        .health_monitor
        .get_status_with_count(&state.project, state.actual_port, number_value);
    // Conflict is exposed via /health: so CLI, editor, and Studio plugin
    // all learn it from the same place, avoiding three separate channels.
    status_info.place_conflict = state
        .place_clash_state
        .lock()
        .ok()
        .and_then(|c| c.clone());
    status_info.sync_suspended = crate::project::is_sync_suspended();
    status_info.bound_place = state.project.linked_place();
    Json(status_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_loop_reported_below_threshold() {
        let d = LoopDetector::new();
        for _ in 0..10 {
            assert!(!d.persist("uuid-1", "Position"));
        }
        assert_eq!(d.total_count(), 0);
    }

    #[test]
    fn loop_reported_once_above_threshold() {
        let d = LoopDetector::new();
        let mut warning = 0;
        for _ in 0..40 {
            if d.persist("uuid-1", "Position") {
                warning += 1;
            }
        }
        // The threshold is crossed, a warning is given, then it is silenced for 30 s: one warning expected.
        assert_eq!(warning, 1);
        assert_eq!(d.total_count(), 1);
    }

    #[test]
    fn properties_do_not_affect_each_other() {
        let d = LoopDetector::new();
        for _ in 0..40 {
            d.persist("uuid-1", "Position");
        }
        // A different property must start with a clean slate.
        assert!(!d.persist("uuid-1", "Size"));
        assert!(!d.persist("uuid-2", "Position"));
    }
}
