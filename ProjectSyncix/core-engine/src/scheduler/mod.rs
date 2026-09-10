use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub type JobFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;

pub enum JobPriority {
    High,
    Normal,
    Background,
}

pub struct Job {
    pub id: String,
    pub priority: JobPriority,
    pub task: JobFuture,
    pub timeout: Duration,
}

/// Thread pool manager that runs long (heavy) async jobs
pub struct JobScheduler {
    sender: mpsc::Sender<Job>,
}

impl JobScheduler {
    pub fn new(worker_count: usize) -> Self {
        // Kanal kapasitesi 1000
        let (tx, mut rx) = mpsc::channel::<Job>(1000);

        // Start N worker threads
        for _i in 0..worker_count {
            let _rx_clone = tx.clone(); // a real implementation could use MPMC or crossbeam.
                                        // For now a simplified single-receiver model:
        }

        // Simply set up one async dispatcher (a real project would distribute with tokio::task::spawn)
        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                info!("Job started: [{}]", job.id);

                // Wrap the job in a timeout
                let result = tokio::time::timeout(job.timeout, job.task).await;

                match result {
                    Ok(Ok(_)) => info!("Job completed: [{}]", job.id),
                    Ok(Err(e)) => error!("Job failed: [{}] - {}", job.id, e),
                    Err(_) => warn!("Job timed out: [{}]", job.id),
                }
            }
        });

        Self { sender: tx }
    }

    /// Adds a new job to the queue
    pub async fn enqueue_job(&self, job: Job) -> Result<(), String> {
        self.sender
            .send(job)
            .await
            .map_err(|e| format!("The queue is full or closed: {}", e))
    }
}
