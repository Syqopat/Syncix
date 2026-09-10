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

/// Uzun süren (Heavy) asenkron görevleri işleyen Thread Pool Yöneticisi
pub struct JobScheduler {
    sender: mpsc::Sender<Job>,
}

impl JobScheduler {
    pub fn new(worker_count: usize) -> Self {
        // Kanal kapasitesi 1000
        let (tx, mut rx) = mpsc::channel::<Job>(1000);

        // N amount worker thread başlat
        for _i in 0..worker_count {
            let _rx_clone = tx.clone(); // Gerçek uygulamada MPMC veya crossbeam kullanılabilir.
                                        // Şimdilik basitleştirilmiş single receiver modeli kuruyoruz:
        }

        // Basitçe single bir asenkron dispatcher kuralım (Gerçek projede tokio::task::spawn ile dağıtılır)
        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                info!("Job started: [{}]", job.id);

                // Job'ı timeout ile sarmala
                let result = tokio::time::timeout(job.timeout, job.task).await;

                match result {
                    Ok(Ok(_)) => info!("Job completed: [{}]", job.id),
                    Ok(Err(e)) => error!("Job hata verdi: [{}] - {}", job.id, e),
                    Err(_) => warn!("Job timed out: [{}]", job.id),
                }
            }
        });

        Self { sender: tx }
    }

    /// Yeni bir görev (Job) kuyruğa ekler
    pub async fn enqueue_job(&self, job: Job) -> Result<(), String> {
        self.sender
            .send(job)
            .await
            .map_err(|e| format!("Kuyruk dolu veya koptu: {}", e))
    }
}
