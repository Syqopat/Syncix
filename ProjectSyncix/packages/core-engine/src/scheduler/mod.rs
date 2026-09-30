//! Background jobs: the heavy, blocking work the core must not do on an async worker.
//!
//! Writing the whole tree to disk is thousands of file operations. Run straight from an
//! async task it holds a runtime worker thread for as long as it takes, so polls and
//! pushes waited behind the disk. Jobs go through this pool instead: the queue is
//! bounded, every job is counted, and a full queue is reported rather than silently
//! growing.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{info, warn};

/// How many jobs may wait before new ones are refused. A full queue means the disk
/// cannot keep up with the changes; queueing further copies of the same work would only
/// use memory.
const QUEUE_CAPACITY: usize = 64;

/// Depth at which the queue is reported as flooded.
const FLOOD_DEPTH: usize = 8;

type Work = Box<dyn FnOnce() + Send + 'static>;

pub struct Job {
    pub name: &'static str,
    pub work: Work,
    /// Reported when the job takes longer than this. It is not cancelled: a half-written
    /// tree is worse than a slow one.
    pub slow_after: Duration,
}

impl Job {
    pub fn new<F: FnOnce() + Send + 'static>(name: &'static str, work: F) -> Self {
        Self {
            name,
            work: Box::new(work),
            slow_after: Duration::from_secs(10),
        }
    }
}

#[derive(Default)]
pub struct JobStats {
    pub queued: AtomicUsize,
    pub running: AtomicUsize,
    pub done: AtomicU64,
    pub rejected: AtomicU64,
    pub peak_queued: AtomicUsize,
    pub slow: AtomicU64,
}

impl JobStats {
    fn enter_queue(&self) {
        let depth = self.queued.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_queued.fetch_max(depth, Ordering::SeqCst);
        if depth >= FLOOD_DEPTH {
            warn!(
                "Background jobs are piling up: {} waiting. The disk is slower than the \
                 changes coming in.",
                depth
            );
        }
    }

    /// What /health reports, so a flood can be seen from outside the process.
    pub fn snapshot(&self) -> JobReport {
        JobReport {
            queued: self.queued.load(Ordering::SeqCst),
            running: self.running.load(Ordering::SeqCst),
            done: self.done.load(Ordering::SeqCst),
            rejected: self.rejected.load(Ordering::SeqCst),
            peak_queued: self.peak_queued.load(Ordering::SeqCst),
            slow: self.slow.load(Ordering::SeqCst),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq)]
pub struct JobReport {
    pub queued: usize,
    pub running: usize,
    pub done: u64,
    pub rejected: u64,
    pub peak_queued: usize,
    pub slow: u64,
}

/// Pool of worker tasks that run blocking jobs off the async runtime.
pub struct JobScheduler {
    sender: mpsc::Sender<(Job, oneshot::Sender<()>)>,
    stats: Arc<JobStats>,
}

impl JobScheduler {
    /// `worker_count` is how many jobs may run at once. Zero starts no worker, which
    /// only the tests use.
    pub fn new(worker_count: usize) -> Self {
        let (tx, rx) = mpsc::channel::<(Job, oneshot::Sender<()>)>(QUEUE_CAPACITY);
        let stats = Arc::new(JobStats::default());
        // One receiver, many workers: each worker waits for the channel in turn, so a job
        // goes to whichever worker is free.
        let shared = Arc::new(Mutex::new(rx));

        for _ in 0..worker_count {
            let inbox = shared.clone();
            let stats = stats.clone();
            tokio::spawn(async move {
                loop {
                    let next = inbox.lock().await.recv().await;
                    let Some((job, done_tx)) = next else { return };
                    stats.queued.fetch_sub(1, Ordering::SeqCst);
                    stats.running.fetch_add(1, Ordering::SeqCst);

                    let name = job.name;
                    let slow_after = job.slow_after;
                    let started = Instant::now();
                    // The work itself is blocking, so it runs on the blocking pool and
                    // this worker only waits for it.
                    let outcome = tokio::task::spawn_blocking(job.work).await;
                    let took = started.elapsed();

                    stats.running.fetch_sub(1, Ordering::SeqCst);
                    stats.done.fetch_add(1, Ordering::SeqCst);
                    if took > slow_after {
                        stats.slow.fetch_add(1, Ordering::SeqCst);
                        warn!("Job '{}' took {:.1}s.", name, took.as_secs_f32());
                    }
                    if let Err(err) = outcome {
                        warn!("Job '{}' did not finish: {}", name, err);
                    }
                    let _ = done_tx.send(());
                }
            });
        }

        info!("Background job pool started with {} worker(s).", worker_count);
        Self { sender: tx, stats }
    }

    pub fn stats(&self) -> Arc<JobStats> {
        self.stats.clone()
    }

    /// Runs a job on the pool and waits for it to finish.
    ///
    /// Waiting is what keeps repeated work in order: the caller cannot queue the next
    /// full-tree write before this one is on disk.
    pub async fn run(&self, job: Job) -> Result<(), String> {
        let name = job.name;
        let (done_tx, done_rx) = oneshot::channel();
        self.stats.enter_queue();
        if let Err(err) = self.sender.try_send((job, done_tx)) {
            self.stats.queued.fetch_sub(1, Ordering::SeqCst);
            self.stats.rejected.fetch_add(1, Ordering::SeqCst);
            return Err(format!("job '{}' was not queued: {}", name, err));
        }
        done_rx
            .await
            .map_err(|_| format!("job '{}' was dropped before it finished", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn a_job_runs_and_is_counted() {
        let pool = JobScheduler::new(2);
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();

        pool.run(Job::new("test", move || flag.store(true, Ordering::SeqCst)))
            .await
            .expect("the job should run");

        assert!(ran.load(Ordering::SeqCst));
        let report = pool.stats().snapshot();
        assert_eq!(report.done, 1);
        assert_eq!(report.queued, 0);
        assert_eq!(report.rejected, 0);
    }

    #[tokio::test]
    async fn a_full_queue_is_refused_and_reported() {
        // Nothing drains the queue without a worker, so the capacity is reached and the
        // next job has to be refused instead of waiting.
        let pool = JobScheduler::new(0);
        let mut held = Vec::new();
        for _ in 0..QUEUE_CAPACITY {
            let (tx, rx) = oneshot::channel();
            held.push(rx);
            if pool
                .sender
                .try_send((Job::new("filler", || {}), tx))
                .is_err()
            {
                break;
            }
            pool.stats.queued.fetch_add(1, Ordering::SeqCst);
        }

        let refused = pool.run(Job::new("late", || {})).await;
        assert!(refused.is_err());
        assert_eq!(pool.stats().snapshot().rejected, 1);
    }
}
