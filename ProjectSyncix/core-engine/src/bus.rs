use crate::model::{InstanceNode, InstancePatch};
use tokio::sync::broadcast;

/// Sistemin merkezi sinir ağı (Event Bus).
/// Tüm bileşenler (Watcher, Transport, DataModel) sadece buraya mesaj bırakır veya buradan okur.
/// Birbirlerini doğrudan çağırmazlar (Decoupling).
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Dosya sisteminden veya Studio'dan gelen, "Tüm nesneyi ez" komutu.
    FullNodeUpdate(InstanceNode),

    /// Sadece belirli özellikleri değiştiren yama komutu (Incremental Sync).
    PatchUpdate(InstancePatch),

    /// Bir nesnenin silindiğini bildiren komut.
    NodeDeleted(uuid::Uuid),
}

/// Tüm sistemin paylaştığı Event Bus.
pub struct EventBus {
    sender: broadcast::Sender<SyncEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        // 1024 mesajlık bir kapasite (Batching ve yoğun yük için uygun)
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    /// Bir olayı sisteme yayınlar (Publish)
    pub fn publish(
        &self,
        event: SyncEvent,
    ) -> Result<usize, Box<broadcast::error::SendError<SyncEvent>>> {
        self.sender.send(event).map_err(Box::new)
    }

    /// Olayları dinlemek için bir alıcı (Subscriber) oluşturur
    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.sender.subscribe()
    }
}
