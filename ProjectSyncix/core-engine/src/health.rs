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
/// Studio eklentisinin davranisini belirleyen, syncix.toml'dan gelen ayarlar.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PluginConfig {
    /// "two_way" | "studio_to_disk" | "disk_to_studio" | "manual"
    pub mode: String,
    /// "queue" | "ignore" | "apply"
    pub play_mode: String,
    /// Ilk baglantida Studio'da izin sorulsun mu.
    pub ask_permission: bool,
    /// Syncix'in degisiklikleri Studio'nun geri al yigina girsin mi.
    pub undo: bool,
    /// Izlenecek servisler. Bos ise eklentinin varsayilan listesi gecerli.
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

    /// Bu klasor hangi place'e bagli? Hic baglanmadiysa None.
    ///
    /// Eklenti port tararken buna bakiyor: kendi place'ine bagli core'u tercih
    /// ediyor, baska bir place'e bagli core'u atliyor. Yoksa iki proje acikken
    /// buldugu ILK core'a baglanip gereksiz catisma cikariyordu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_place: Option<String>,

    /// Klasor baska bir place'e bagliysa dolu olur; senkron askida demektir.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_conflict: Option<crate::server::PlaceCatismasi>,
    /// Senkron askida mi (su an yalnizca place catismasinda oluyor).
    pub sync_suspended: bool,

    /// Eklentinin uygulayacagi ayarlar.
    ///
    /// Eklenti bunlari kendi icinde tutmuyor; tek dogruluk kaynagi syncix.toml.
    /// Aksi halde ayni proje icin iki ayri ayar seti olusur ve hangisinin
    /// gecerli oldugu belirsizlesir. Eklenti baglanirken /health okudugu icin
    /// ek bir kanal acmaya gerek kalmiyor.
    pub config: PluginConfig,

    // --- Eklenti akış günlüğü özeti ---
    // Eklentinin belleğindeki günlüğü dışarıdan görebilmenin tek yolu bu.
    /// Modeldeki obje sayısı. Editörün durum çubuğu bunu gösterir; eskiden
    /// mesajları sayarak tahmin ediliyordu ve zamanla gerçekten sapıyordu.
    pub object_count: usize,

    pub activity_total: usize,
    pub activity_in: usize,
    pub activity_out: usize,
    pub conflicts: usize,
}

/// Echo döngüsü dedektörü.
///
/// Neden var: echo engelleme Roblox'un deferred signal'ları yüzünden kusursuz değil.
/// Bir döngü oluştuğunda hiçbir yerde iz bırakmıyordu, yani sessizce CPU yakıyordu.
/// Bu sınıf aynı (uuid, property) çiftinin kısa sürede kaç kez gidip geldiğini sayar
/// ve eşiği aşınca bir kez uyarı basar. PatchExecutor'a warn eklemenin renk hatasını
/// bulması gibi, görünmez olanı görünür yapar.
pub struct LoopDetector {
    /// (uuid, property) -> o pencerede görülen zaman damgaları
    gorulen: Mutex<HashMap<(String, String), Vec<StdInstant>>>,
    /// Eşik aşıldığında tekrar tekrar uyarmamak için susturulanlar
    susturulan: Mutex<HashMap<(String, String), StdInstant>>,
    pencere: Duration,
    esik: usize,
    sayac: AtomicUsize,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self {
            gorulen: Mutex::new(HashMap::new()),
            susturulan: Mutex::new(HashMap::new()),
            // 2 saniyede 12 defadan fazla aynı property gidip geliyorsa bu normal
            // bir kullanıcı düzenlemesi değildir; sürükleme bile bu sıklıkta
            // BatchQueue tarafından birleştirilerek gelir.
            pencere: Duration::from_secs(2),
            esik: 12,
            sayac: AtomicUsize::new(0),
        }
    }

    /// Bir property değişimini kaydeder. Döngü şüphesi varsa true döner.
    pub fn kaydet(&self, uuid: &str, property: &str) -> bool {
        let anahtar = (uuid.to_string(), property.to_string());
        let simdi = StdInstant::now();

        // Zaten uyarı verilmişse 30 saniye boyunca sus.
        {
            let mut susturulan = self.susturulan.lock().unwrap();
            if let Some(t) = susturulan.get(&anahtar) {
                if simdi.duration_since(*t) < Duration::from_secs(30) {
                    return false;
                }
                susturulan.remove(&anahtar);
            }
        }

        let mut gorulen = self.gorulen.lock().unwrap();
        let liste = gorulen.entry(anahtar.clone()).or_default();
        liste.retain(|t| simdi.duration_since(*t) < self.pencere);
        liste.push(simdi);

        if liste.len() > self.esik {
            liste.clear();
            drop(gorulen);
            self.susturulan.lock().unwrap().insert(anahtar, simdi);
            self.sayac.fetch_add(1, Ordering::SeqCst);
            return true;
        }

        // Bellek sızmasın: sözlük büyüdüyse ölü kayıtları at.
        if gorulen.len() > 512 {
            gorulen.retain(|_, v| {
                v.retain(|t| simdi.duration_since(*t) < self.pencere);
                !v.is_empty()
            });
        }

        false
    }

    pub fn toplam(&self) -> usize {
        self.sayac.load(Ordering::SeqCst)
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

    /// Studio'nun en son ne zaman poll ettiği. Bağlı mı değil mi bunu buradan biliyoruz.
    son_studio_temasi: Mutex<Option<StdInstant>>,

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
            son_studio_temasi: Mutex::new(None),
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

    /// Studio eklentisi kendi BatchQueue sayaçlarını bildirir.
    /// Birleştirmenin gerçekten çalışıp çalışmadığı ancak böyle ölçülebilir:
    /// queued kaç değişiklik girdi, coalesced kaçı tek mesajda birleşti.
    pub fn set_plugin_metrics(&self, queued: usize, coalesced: usize) {
        self.plugin_queued.store(queued, Ordering::SeqCst);
        self.plugin_coalesced.store(coalesced, Ordering::SeqCst);
    }

    /// Eklentinin akış günlüğü özeti.
    pub fn set_activity(&self, toplam: usize, gelen: usize, giden: usize, cakisma: usize) {
        self.activity_total.store(toplam, Ordering::SeqCst);
        self.activity_in.store(gelen, Ordering::SeqCst);
        self.activity_out.store(giden, Ordering::SeqCst);
        self.conflicts.store(cakisma, Ordering::SeqCst);
    }

    pub fn touch_studio(&self) {
        *self.son_studio_temasi.lock().unwrap() = Some(StdInstant::now());
    }

    pub fn studio_connected(&self) -> bool {
        self.son_studio_temasi
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
            loops_detected: self.loop_detector.toplam(),
            plugin_queued: self.plugin_queued.load(Ordering::SeqCst),
            plugin_coalesced: self.plugin_coalesced.load(Ordering::SeqCst),
            object_count: 0,
            bound_place: None,
            place_conflict: None,
            sync_suspended: false,
            config: PluginConfig {
                mode: project.mod_.adi().to_string(),
                play_mode: project.play.adi().to_string(),
                ask_permission: project.izin_sor,
                undo: project.geri_al,
                services: project.kapsam.servisler.clone(),
                ignore_classes: project.kapsam.sinif_disla.clone(),
                ignore_properties: project.kapsam.property_disla.clone(),
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
    let sayi = state.data_model.read().await.get_all_instances().len();
    let mut durum = state
        .health_monitor
        .get_status_with_count(&state.project, state.actual_port, sayi);
    // Catisma /health uzerinden disari veriliyor: hem CLI hem editor hem
    // Studio eklentisi ayni yerden ogrensin, uc ayri kanal olmasin.
    durum.place_conflict = state
        .place_catismasi
        .lock()
        .ok()
        .and_then(|c| c.clone());
    durum.sync_suspended = crate::project::senkron_askida();
    durum.bound_place = state.project.bagli_place();
    Json(durum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esik_altinda_dongu_bildirmez() {
        let d = LoopDetector::new();
        for _ in 0..10 {
            assert!(!d.kaydet("uuid-1", "Position"));
        }
        assert_eq!(d.toplam(), 0);
    }

    #[test]
    fn esik_ustunde_dongu_bildirir_ve_bir_kez_uyarir() {
        let d = LoopDetector::new();
        let mut uyari = 0;
        for _ in 0..40 {
            if d.kaydet("uuid-1", "Position") {
                uyari += 1;
            }
        }
        // Eşik aşılır, uyarı verilir, sonra 30 sn susturulur: tek uyarı beklenir.
        assert_eq!(uyari, 1);
        assert_eq!(d.toplam(), 1);
    }

    #[test]
    fn farkli_propertyler_birbirini_etkilemez() {
        let d = LoopDetector::new();
        for _ in 0..40 {
            d.kaydet("uuid-1", "Position");
        }
        // Başka bir property temiz sayfa ile başlamalı.
        assert!(!d.kaydet("uuid-1", "Size"));
        assert!(!d.kaydet("uuid-2", "Position"));
    }
}
