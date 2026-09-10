use axum::Json;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant as StdInstant};
use tokio::time::Instant;

/// /health cevabı. Artık sadece "ayakta mıyım" demiyor; core kendini tanıtıyor.
/// Studio eklentisi bunu okuyup hangi projeye bağlandığını kullanıcıya gösteriyor,
/// sürüm uyumunu kontrol ediyor ve doğru portu buluyor.
/// Studio eklentisinin davranisini belirleyen, syncix.toml'dan received ayarlar.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PluginConfig {
    /// "two_way" | "studio_to_disk" | "disk_to_studio" | "manual"
    pub mode: String,
    /// "queue" | "ignore" | "apply"
    pub play_mode: String,
    /// Ilk baglantida Studio'da izin sorulsun mu.
    pub ask_permission: bool,
    /// Syncix'in degisiklikleri Studio'nun restored_count al yigina girsin mi.
    pub undo: bool,
    /// Izlenecek service_list. Bos ise eklentinin fallback_value listesi gecerli.
    pub services: Vec<String>,
    /// Hic izlenmeyecek siniflar.
    pub ignore_classes: Vec<String>,
    /// Hic izlenmeyecek property'ler.
    pub ignore_properties: Vec<String>,
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,

    // --- Kimlik ---
    pub version: String,
    pub protocol: u32,
    pub project: String,
    pub root: String,
    pub port: u16,

    // --- Durum ---
    pub uptime_seconds: u64,
    pub active_connections: usize,
    pub messages_processed: usize,
    pub studio_connected: bool,

    // --- Metrikler (madde 7: doğrulanamayan davranışı ölçülebilir hale getirir) ---
    pub inbound_from_studio: usize,
    pub outbound_to_studio: usize,
    pub loops_detected: usize,
    pub plugin_queued: usize,
    pub plugin_coalesced: usize,

    /// Bu folder_path hangi place'e is_bound? Hic baglanmadiysa None.
    ///
    /// Eklenti port tararken buna bakiyor: own place'ine is_bound core'u tercih
    /// ediyor, baska bir place'e is_bound core'u atliyor. Yoksa iki proje acikken
    /// buldugu ILK core'a baglanip gereksiz place_clash cikariyordu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_place: Option<String>,

    /// Klasor baska bir place'e bagliysa dolu olur; syncing suspended demektir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_conflict: Option<crate::server::PlaceConflict>,
    /// Senkron suspended mi (su an yalnizca place catismasinda oluyor).
    pub sync_suspended: bool,

    /// Eklentinin uygulayacagi ayarlar.
    ///
    /// Eklenti bunlari own icinde tutmuyor; single dogruluk kaynagi syncix.toml.
    /// Aksi halde is_same proje icin iki ayri ayar seti olusur ve hangisinin
    /// gecerli oldugu belirsizlesir. Eklenti baglanirken /health okudugu icin
    /// ek bir kanal acmaya gerek kalmiyor.
    pub config: PluginConfig,

    // --- Eklenti akış günlüğü özeti ---
    // Eklentinin belleğindeki günlüğü dışarıdan görebilmenin single yolu bu.
    /// Modeldeki obje sayısı. Editörün status_info çubuğu bunu gösterir; eskiden
    /// mesajları sayarak tahmin ediliyordu ve zamanla gerçekten sapıyordu.
    pub object_count: usize,

    pub activity_total: usize,
    pub activity_in: usize,
    pub activity_out: usize,
    pub conflicts: usize,
}

/// Echo döngüsü dedektörü.
///
/// Neden exists_flag: echo engelleme Roblox'un deferred signal'ları yüzünden kusursuz değil.
/// Bir döngü oluştuğunda hiçbir yerde iz bırakmıyordu, yani sessizce CPU yakıyordu.
/// Bu sınıf aynı (uuid, property) çiftinin kısa sürede kaç kez gidip geldiğini sayar
/// ve eşiği aşınca bir kez uyarı basar. PatchExecutor'a warn eklemenin renk hatasını
/// bulması gibi, görünmez olanı görünür yapar.
pub struct LoopDetector {
    /// (uuid, property) -> o pencerede görülen zaman damgaları
    seen: Mutex<HashMap<(String, String), Vec<StdInstant>>>,
    /// Eşik aşıldığında again again uyarmamak için susturulanlar
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
            // 2 saniyede 12 defadan extra aynı property gidip geliyorsa bu normal
            // bir kullanıcı düzenlemesi değildir; sürükleme bile bu sıklıkta
            // BatchQueue tarafından birleştirilerek gelir.
            time_window: Duration::from_secs(2),
            threshold: 12,
            counter: AtomicUsize::new(0),
        }
    }

    /// Bir property değişimini kaydeder. Döngü şüphesi varsa true döner.
    pub fn persist(&self, uuid: &str, property: &str) -> bool {
        let key_name = (uuid.to_string(), property.to_string());
        let current_time = StdInstant::now();

        // Zaten uyarı verilmişse 30 saniye boyunca sus.
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
        let liste = seen.entry(key_name.clone()).or_default();
        liste.retain(|t| current_time.duration_since(*t) < self.time_window);
        liste.push(current_time);

        if liste.len() > self.threshold {
            liste.clear();
            drop(seen);
            self.suppressed.lock().unwrap().insert(key_name, current_time);
            self.counter.fetch_add(1, Ordering::SeqCst);
            return true;
        }

        // Bellek sızmasın: sözlük büyüdüyse ölü kayıtları at.
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

    /// Studio'nun en last_item ne zaman poll ettiği. Bağlı mı değil mi bunu buradan biliyoruz.
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

    /// Studio eklentisi own BatchQueue sayaçlarını bildirir.
    /// Birleştirmenin gerçekten çalışıp çalışmadığı ancak böyle ölçülebilir:
    /// queued kaç değişiklik input_value, coalesced kaçı single mesajda birleşti.
    pub fn set_plugin_metrics(&self, queued: usize, coalesced: usize) {
        self.plugin_queued.store(queued, Ordering::SeqCst);
        self.plugin_coalesced.store(coalesced, Ordering::SeqCst);
    }

    /// Eklentinin akış günlüğü özeti.
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
                play_mode: project.play.name_of().to_string(),
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
    // Catisma /health uzerinden disari veriliyor: hem CLI hem editor hem
    // Studio eklentisi is_same yerden ogrensin, uc ayri kanal olmasin.
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
        // Eşik aşılır, uyarı verilir, sonra 30 sn susturulur: single uyarı beklenir.
        assert_eq!(warning, 1);
        assert_eq!(d.total_count(), 1);
    }

    #[test]
    fn properties_do_not_affect_each_other() {
        let d = LoopDetector::new();
        for _ in 0..40 {
            d.persist("uuid-1", "Position");
        }
        // Başka bir property temiz sayfa ile başlamalı.
        assert!(!d.persist("uuid-1", "Size"));
        assert!(!d.persist("uuid-2", "Position"));
    }
}
