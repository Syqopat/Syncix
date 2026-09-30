//! Split out of transport.rs.

#[allow(unused_imports)]
use super::*;

/// How many delivered payloads are remembered while Studio has not confirmed them.
pub(crate) const SENT_LOG_LIMIT: usize = 20_000;

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
