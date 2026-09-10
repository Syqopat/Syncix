use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Sistemdeki tüm iletişim paketlerinin standart şeması.
/// Production-Ready: Sürümlendirme eklendi (ileride v2 geldiğinde eskiler bozulmaz).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub version: String, // Örn: "v1"
    pub event_type: EventType,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// Sunucu ile istemci arasındaki bağlantı testi
    Ping,
    Pong,
    /// VS Code'da bir file_path değiştiğinde Studio'ya itilen message
    PushUpdate,
    /// Studio'da nesne değiştiğinde VS Code'a (çekirdeğe) gönderilen message
    ClientUpdate,
    /// Yeni file_path yaratıldığında
    PushCreate,
    
    CompositeUpdate,
    PropertyUpdate,
    Create,
    Destroy,
    FullSync,
    GetTree,

    /// Core'un Studio'dan ağacın tamamını YENİDEN göndermesini istediği message.
    ///
    /// Neden gerekli: doğrulama yaparken "core'un modeli" ile "Studio'nun gerçek
    /// durumu" birbirine karıştırılabiliyordu. Model, komutu gönderirken zaten
    /// güncelleniyor; dolayısıyla modeli okumak komutun Studio'ya ULAŞTIĞINI
    /// kanıtlamaz. Bu message Studio'yu konuşturur, cevabı single doğruluk kaynağıdır.
    FullSyncRequest,

    /// Studio eklentisinin own sayaçlarını bildirdiği message
    /// (BatchQueue birleştirmesinin ölçülebilir olması için).
    PluginMetrics,

    /// VS Code Explorer'dan received komutlar (VS Code -> Rust -> Studio yönü)
    CreateInstance,
    RenameInstance,
    DeleteInstance,
    /// CollectionService etiketlerinin tamami. Tek single add_instance/sil yerine liste
    /// butun halinde gonderiliyor; iki tarafta ayri status_info tutmayi onluyor.
    SetTags,
    /// Hangi objelerin secili oldugu. Model'e ve diske YAZILMAZ: secim scratch_dir
    /// bir status_info, projenin icerigi degil. Diske yazilsaydi her tiklama file_path
    /// degistirir, surum kontrolunde gurultu olurdu.
    Selection,
    ReparentInstance,
    SetProperty,
    SetAttribute,
}

/// Studio'ya gidecek mesajların kayıpsız teslimat kuyruğu.
/// Teknik Gerekçe: broadcast kanalı yalnızca o anda pending_item aboneye teslim eder;
/// Studio iki poll arasındayken gönderilen mesajlar kaybolur. Bu kuyruk mesajı
/// bir sonraki poll'e kadar bellekte tutar.
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

    /// Senkron push: hem async hem blocking (file watcher thread'i) bağlamdan çağrılabilir.
    pub fn push(&self, payload: Payload) {
        self.queue.lock().unwrap().push_back(payload);
        self.notify.notify_one();
    }

    fn pop(&self) -> Option<Payload> {
        self.queue.lock().unwrap().pop_front()
    }

    /// Kuyrukta message varsa hemen döner; yoksa timeout süresince bekler.
    pub async fn pop_or_wait(&self, timeout: std::time::Duration) -> Option<Payload> {
        if let Some(p) = self.pop() {
            return Some(p);
        }
        let _ = tokio::time::timeout(timeout, self.notify.notified()).await;
        self.pop()
    }
}

/// Transport Layer'ın arayüzü (Trait).
/// Teknik Gerekçe: İş mantığını (Core Engine) iletişim yönteminden (WebSocket/HTTP) ayırmak
/// için bu arayüz kullanılır. Çekirdek sadece bu fonksiyonları çağırır, altta ne çalıştığını bilmez.
#[async_trait]
pub trait Transport {
    /// İletişim kanalını başlatır (örn. WebSocket sunucusunu dinlemeye başlar).
    async fn start(&self) -> Result<(), String>;

    /// Bir mesajı bağlı olan tüm istemcilere (Studio'lara) gönderir (Broadcast).
    async fn broadcast(&self, payload: &Payload) -> Result<(), String>;

    // İstemcilerden received mesajları dinlemek için bir kanal (receiver) sağlar.
    // (Gerçek uygulamada tokio::sync::mpsc::Receiver kullanılacak)
    // async fn receive(&self) -> Receiver<Payload>;
}
