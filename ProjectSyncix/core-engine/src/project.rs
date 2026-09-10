//! Proje yapılandırması ve kimliği.
//!
//! Buradaki iki şey release için kritik:
//!  1. Port artık sabit değil. syncix.toml'dan okunur, doluysa sıradaki denenir ve
//!     SEÇİLEN port diske yazılır. Editör ve CLI o dosyadan okur, tahmin etmez.
//!  2. Core artık kendini tanıtır (proje adı, kök directory, sürüm). Studio eklentisi
//!     hangi projeye bağlandığını kullanıcıya gösterebilsin diye gerekli.

use std::fs;
use std::path::{Path, PathBuf};

/// Place kimligi uyusmadiginda syncing askiya alinir.
///
/// Global tutulmasinin sebebi: bayragi okumasi gereken uc yer birbirinden
/// bagimsiz calisiyor (file_path izleyici own thread'inde, disk yazicisi own
/// gorevinde, command_name dongusu ana gorevde). Her birine ayri kanal cekmek yerine
/// single bir atomik bayrak, bu uc yerin de is_same anda susmasini garanti ediyor.
static SYNC_SUSPENDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_sync_suspended(suspended: bool) {
    SYNC_SUSPENDED.store(suspended, std::sync::atomic::Ordering::SeqCst);
}

/// Senkron suspended mi? Askidayken HICBIR direction calismaz: ne disk okunur, ne
/// diske yazilir, ne Studio'ya command_name gider. Amac, karar verilene kadar iki
/// tarafi da oldugu gibi birakmak.
pub fn is_sync_suspended() -> bool {
    SYNC_SUSPENDED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Core'un own sürümü (Cargo.toml'dan gelir; single origin).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tel protokolü sürümü. Studio eklentisi ile core arasındaki message biçimi
/// uyumsuz hale geldiğinde ARTTIRILIR. Sürüm numarasından bağımsızdır:
/// 0.3.1 -> 0.3.2 gibi bir yama protokolü bozmaz, bu sayı aynı kalır.
pub const PROTOCOL_VERSION: u32 = 1;

/// Port araması bu aralıkta yapılır. Studio eklentisi de aynı aralığı tarar.
pub const PORT_SCAN_SPAN: u16 = 10;

pub const DEFAULT_PORT: u16 = 8080;

/// Senkronun hangi yonlerde aktif oldugu.
///
/// Rojo single yonlu calisiyor: file_path sistemi single dogruluk kaynagi, Studio yalnizca
/// alici. Syncix fallback_value olarak cift yonlu, ama herkes bunu istemiyor —
/// takim halinde calisan biri Studio'yu salt okunur tutmak, tersine bir tasarimci
/// diskin ezilmesini istemeyebilir. Bu yuzden direction bir ayar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncMode {
    /// Iki direction de is_enabled. Varsayilan.
    TwoWay,
    /// Studio -> disk. Studio'da yapilan degisiklik diske yazilir; diskteki
    /// degisiklik Studio'ya GITMEZ. Sahne tasarimini Studio'da yapip kodu
    /// surum kontrolunde tutmak isteyenler icin.
    StudioToDisk,
    /// Disk -> Studio. Rojo'nun calisma sekli: file_path sistemi dogruluk kaynagi.
    DiskToStudio,
    /// Hicbir direction otomatik degil; yalnizca acikca given komutlar islenir
    /// (syncix pull, syncix set, ...). Riskli bir sahnede gozetimli calismak icin.
    Manual,
}

impl SyncMode {
    fn resolve_arg(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "two_way" | "twoway" | "both" => Some(Self::TwoWay),
            "studio_to_disk" | "studio" | "pull" => Some(Self::StudioToDisk),
            "disk_to_studio" | "disk" | "push" | "rojo" => Some(Self::DiskToStudio),
            "manual" | "off" | "none" => Some(Self::Manual),
            _ => None,
        }
    }

    pub fn name_of(&self) -> &'static str {
        match self {
            Self::TwoWay => "two_way",
            Self::StudioToDisk => "studio_to_disk",
            Self::DiskToStudio => "disk_to_studio",
            Self::Manual => "manual",
        }
    }

    /// Studio'da olan bir degisiklik modele ve diske yansitilsin mi?
    pub fn accepts_from_studio(&self) -> bool {
        matches!(self, Self::TwoWay | Self::StudioToDisk)
    }

    /// Diskte olan bir degisiklik Studio'ya gonderilsin mi?
    pub fn accepts_from_disk(&self) -> bool {
        matches!(self, Self::TwoWay | Self::DiskToStudio)
    }
}

/// Oyun calisirken (Play) editorden received degisikliklere ne olacak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayBehavior {
    /// Kuyruga alinir, Play bitince uygulanir. Varsayilan.
    Queue,
    /// Atilir. Play sirasinda hicbir sey olmasin diyenler icin.
    Ignore,
    /// Dogrudan uygulanir. Play bitince Studio oturumla birlikte atacagi icin
    /// degisiklik kaybolur; yalnizca bilerek isteyen acsin.
    Apply,
}

impl PlayBehavior {
    fn resolve_arg(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "queue" | "kuyruk" => Some(Self::Queue),
            "ignore" | "yoksay" | "drop" => Some(Self::Ignore),
            "apply" | "uygula" => Some(Self::Apply),
            _ => None,
        }
    }

    pub fn name_of(&self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Ignore => "ignore",
            Self::Apply => "apply",
        }
    }
}

/// Silmeye dair safety_settings ayarlari.
#[derive(Clone, Debug)]
pub struct SafetySettings {
    /// Uzlastirici sildigi dosyalari cop kutusuna tasisin mi.
    /// Kapatilirsa file_list dogrudan silinir ve restored_count donusu olmaz.
    pub trash_enabled: bool,
    /// Cop kutusunda saklanacak run_name sayisi.
    pub trash_keep_runs: usize,
    /// Diskten silinen bir dosyanin gercek deletion sayilmasi icin beklenecek sure.
    /// Tasima islemleri isletim sisteminde sil+generate olarak goruldugu icin
    /// bu time_window gerekiyor. Yavas disklerde arttirilabilir.
    pub delete_grace_ms: u64,
    /// `syncix rm` onay istesin mi.
    pub confirm_delete: bool,
}

/// Neyin syncing edilecegini belirleyen ayarlar.
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ScopeSettings {
    /// Izlenecek service_list. Bos birakilirsa eklentinin fallback_value listesi gecerli.
    pub service_list: Vec<String>,
    /// Bu siniflar hic syncing edilmez (ornegin "Camera", "Terrain").
    pub class_ignore_list: Vec<String>,
    /// Bu property'ler hic syncing edilmez. Gurultulu ya da makineye ozel
    /// alanlari elemek icin.
    pub property_ignore_list: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    /// Senkron klasörü, core'un çalışma dizinine göre (ör. "../src").
    pub sync_dir: String,
    /// syncix.toml'un bulunduğu directory, absolute fs_path.
    pub root: PathBuf,
    /// Kullanıcıya gösterilecek proje adı (kök klasörün adı).
    pub name: String,
    /// syncix.toml'da istenen port. Dolu olabilir; gerçekte bağlanılan port
    /// `bind_with_fallback` tarafından belirlenir.
    pub wanted_port: u16,
    /// sourcemap.json her senkronda güncellensin mi (luau-lsp için).
    pub sourcemap: bool,
    /// Senkron dışı bırakılacak yollar (glob). Bu file_list ne okunur ne silinir.
    pub ignore: Vec<String>,
    /// Port command_name satırından açıkça istendiyse true olur ve devretme yapılmaz.
    /// Sebep: kullanıcı Studio eklentisine de aynı portu yazıyor; core sessizce
    /// başka bir porta kayarsa iki taraf ayrışır ve sebebi anlaşılmaz.
    pub port_fixed: bool,

    /// Senkron yonu.
    pub mode_value: SyncMode,
    /// Disk yazicisinin bekleme suresi. Kucuk raw_value daha hizli yansitir ama
    /// yazim sayisini arttirir.
    pub debounce_ms: u64,
    /// Play sirasinda received degisikliklerin akibeti.
    pub play: PlayBehavior,
    /// Ilk baglantida Studio'da izin sorulsun mu.
    pub prompt_permission: bool,
    /// Syncix'in yaptigi degisiklikler Studio'nun restored_count al yigina girsin mi.
    pub restore_cmd: bool,
    /// Script'lerin yanina .meta.json yazilsin mi. Kapatilirsa script'lerin
    /// property ve attribute'lari diske hic yazilmaz.
    pub meta_files: bool,
    pub safety_settings: SafetySettings,
    pub scope_settings: ScopeSettings,
}

impl Default for SafetySettings {
    fn default() -> Self {
        Self {
            trash_enabled: true,
            trash_keep_runs: 10,
            delete_grace_ms: 800,
            confirm_delete: true,
        }
    }
}


/// TOML'dan bir bolumu okumak icin kucuk yardimcilar.
/// Bilinmeyen key_names sessizce yutulmaz; cagiran taraf warning basar.
fn string_list(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn read_bool(section: Option<&toml::Value>, key_name: &str, fallback_value: bool) -> bool {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_bool())
        .unwrap_or(fallback_value)
}

fn read_number(section: Option<&toml::Value>, key_name: &str, fallback_value: u64) -> u64 {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_integer())
        .and_then(|x| u64::try_from(x).ok())
        .unwrap_or(fallback_value)
}

impl ProjectConfig {
    /// Çalışma dizininden yukarı doğru syncix.toml arar.
    /// Core hem proje kökünden hem core-engine/ içinden çalıştırılabiliyor.
    pub fn load() -> Self {
        for (config_path, base) in [("../syncix.toml", ".."), ("syncix.toml", ".")] {
            let path = Path::new(config_path);
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            let value = match text.parse::<toml::Value>() {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Could not read syncix.toml ({}): {}", config_path, e);
                    continue;
                }
            };
            return Self::resolve_arg(&value, base);
        }

        // syncix.toml yoksa taşınabilir varsayılanlar.
        Self::resolve_arg(&toml::Value::Table(Default::default()), "..")
    }

    /// Ayrıştırma testten de çağrılabilsin diye ayrı: yapılandırma davranışı
    /// file_path sistemine bağlı olmadan doğrulanabilmeli.
    pub fn resolve_arg(value: &toml::Value, base: &str) -> Self {
        // Anahtarlar hem kök seviyede hem bölüm içinde kabul ediliyor.
        // Sebep: previous_text syncix.toml'lar düz yazılmıştı ve bir yükseltme kimsenin
        // dosyasını bozmamalı. Bölüm varsa o kazanır.
        let sync = value.get("sync");
        let files = value.get("files");
        let safety = value.get("safety");
        let scope = value.get("scope");
        let server = value.get("server");
        let editor = value.get("editor");

        let al = |section: Option<&toml::Value>, key_name: &str| -> Option<toml::Value> {
            section
                .and_then(|b| b.get(key_name))
                .or_else(|| value.get(key_name))
                .cloned()
        };

        let dir = al(files, "sync_dir")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "src".to_string());

        let port = al(server, "port")
            .and_then(|x| x.as_integer())
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or(DEFAULT_PORT);

        let mode_value = al(sync, "mode")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .and_then(|s| {
                let m = SyncMode::resolve_arg(&s);
                if m.is_none() {
                    tracing::warn!(
                        "Unknown sync mode '{}'; falling back to two_way.                          Valid values: two_way, studio_to_disk, disk_to_studio, manual.",
                        s
                    );
                }
                m
            })
            .unwrap_or(SyncMode::TwoWay);

        let play = al(sync, "play_mode")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .and_then(|s| {
                let p = PlayBehavior::resolve_arg(&s);
                if p.is_none() {
                    tracing::warn!(
                        "Unknown play_mode '{}'; falling back to queue.                          Valid values: queue, ignore, apply.",
                        s
                    );
                }
                p
            })
            .unwrap_or(PlayBehavior::Queue);

        let root = clean_path(fs::canonicalize(base).unwrap_or_else(|_| PathBuf::from(base)));

        Self {
            sync_dir: format!("{}/{}", base, dir),
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Syncix".to_string()),
            root,
            wanted_port: port,
            sourcemap: read_bool(editor, "sourcemap", true)
                && value.get("sourcemap").and_then(|x| x.as_bool()).unwrap_or(true),
            ignore: string_list(al(files, "ignore").as_ref()),
            port_fixed: false,

            mode_value,
            debounce_ms: read_number(sync, "debounce_ms", 120).clamp(10, 10_000),
            play,
            prompt_permission: read_bool(sync, "ask_permission", false),
            restore_cmd: read_bool(sync, "undo", true),
            meta_files: read_bool(files, "meta_files", true),

            safety_settings: SafetySettings {
                trash_enabled: read_bool(safety, "trash", true),
                trash_keep_runs: read_number(safety, "trash_keep", 10).clamp(1, 500) as usize,
                delete_grace_ms: read_number(safety, "delete_grace_ms", 800).clamp(0, 30_000),
                confirm_delete: read_bool(safety, "confirm_delete", true),
            },
            scope_settings: ScopeSettings {
                service_list: string_list(al(scope, "services").as_ref()),
                class_ignore_list: string_list(al(scope, "ignore_classes").as_ref()),
                property_ignore_list: string_list(al(scope, "ignore_properties").as_ref()),
            },
        }
    }

    /// Bu class_str syncing edilecek mi?
    pub fn class_allowed(&self, class_str: &str) -> bool {
        !self.scope_settings.class_ignore_list.iter().any(|d| d == class_str)
    }

    /// Bu property syncing edilecek mi?
    pub fn property_allowed(&self, item_name: &str) -> bool {
        !self.scope_settings.property_ignore_list.iter().any(|d| d == item_name)
    }

    /// Bu klasorun is_bound oldugu place'in identity dosyasi: <proje>/.syncix/place
    pub fn place_file(&self) -> PathBuf {
        self.runtime_dir().join("place")
    }

    /// Klasore daha once hangi place baglanmis? Hic baglanmadiysa None.
    pub fn linked_place(&self) -> Option<String> {
        let file_content = fs::read_to_string(self.place_file()).ok()?;
        let k = file_content.trim().to_string();
        (!k.is_empty()).then_some(k)
    }

    /// Klasoru bir place'e baglar.
    pub fn bind_place(&self, identity: &str) {
        let dir = self.runtime_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Could not create the .syncix folder: {}", e);
            return;
        }
        if let Err(e) = fs::write(self.place_file(), identity) {
            tracing::warn!("Could not write the place identity: {}", e);
        }
    }

    fn runtime_dir(&self) -> PathBuf {
        self.root.join(".syncix")
    }

    pub fn port_file(&self) -> PathBuf {
        self.runtime_dir().join("port")
    }

    /// luau-lsp'nin okuduğu sourcemap dosyası, proje kökünde.
    pub fn sourcemap_file(&self) -> PathBuf {
        self.root.join("sourcemap.json")
    }

    /// Gerçekte bağlanılan portu diske yazar. Editör ve CLI bunu okur.
    /// Böylece "8080 olduğunu varsay" tahmini tamamen ortadan kalkar.
    pub fn write_port_file(&self, port: u16) {
        let dir = self.runtime_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Could not create the .syncix folder: {}", e);
            return;
        }
        if let Err(e) = fs::write(self.port_file(), port.to_string()) {
            tracing::warn!("Could not write the port file: {}", e);
        }
    }

    /// Core kapanırken bayat port dosyası bırakmamak için.
    pub fn clear_port_file(&self) {
        let _ = fs::remove_file(self.port_file());
    }
}

/// Windows'ta `fs::canonicalize` yolun başına `\\?\` (extended-length) öneki koyar.
/// Bu fs_path Studio'daki onay penceresinde kullanıcıya gösterildiği için temizlenir.
fn clean_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(remaining) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(remaining);
    }
    p
}

/// İki sürümün birlikte çalışıp çalışamayacağını söyler.
/// Kural: major ve minor eşleşmeli, patch farkı serbest.
/// (0.3.1 ile 0.3.9 uyumlu; 0.3.x ile 0.4.x değil.)
pub fn versions_compatible(a: &str, b: &str) -> bool {
    fn major_minor(v: &str) -> (u32, u32) {
        let mut it = v.split('.');
        let major = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let minor = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (major, minor)
    }
    major_minor(a) == major_minor(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_diff_compatible_minor_diff_not() {
        assert!(versions_compatible("0.3.1", "0.3.9"));
        assert!(versions_compatible("1.0.0", "1.0.0"));
        assert!(!versions_compatible("0.3.0", "0.4.0"));
        assert!(!versions_compatible("0.3.0", "1.3.0"));
    }

    #[test]
    fn bad_version_text_does_not_crash() {
        assert!(versions_compatible("abc", "abc"));
        assert!(!versions_compatible("0.3.0", "abc"));
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    fn resolve_arg(toml_text: &str) -> ProjectConfig {
        ProjectConfig::resolve_arg(&toml_text.parse::<toml::Value>().unwrap(), ".")
    }

    #[test]
    fn empty_file_gives_defaults() {
        let c = resolve_arg("");
        assert_eq!(c.mode_value, SyncMode::TwoWay);
        assert_eq!(c.play, PlayBehavior::Queue);
        assert_eq!(c.wanted_port, DEFAULT_PORT);
        assert!(c.safety_settings.trash_enabled);
        assert!(c.safety_settings.confirm_delete);
        assert!(c.restore_cmd);
    }

    /// Asıl istenen ayar: Rojo gibi single yönlü çalışabilmek.
    #[test]
    fn one_way_mode() {
        let c = resolve_arg("[sync]\nmode = \"disk_to_studio\"\n");
        assert_eq!(c.mode_value, SyncMode::DiskToStudio);
        assert!(c.mode_value.accepts_from_disk(), "disk -> Studio açık olmalı");
        assert!(!c.mode_value.accepts_from_studio(), "Studio -> disk kapalı olmalı");

        let t = resolve_arg("[sync]\nmode = \"studio_to_disk\"\n");
        assert!(t.mode_value.accepts_from_studio());
        assert!(!t.mode_value.accepts_from_disk());
    }

    /// "rojo" ve "push" gibi takma adlar aynı modu vermeli: kullanıcı hangi
    /// kelimeyi aklında tutuyorsa onu yazabilmeli.
    #[test]
    fn mode_aliases() {
        assert_eq!(SyncMode::resolve_arg("rojo"), Some(SyncMode::DiskToStudio));
        assert_eq!(SyncMode::resolve_arg("push"), Some(SyncMode::DiskToStudio));
        assert_eq!(SyncMode::resolve_arg("PULL"), Some(SyncMode::StudioToDisk));
        assert_eq!(SyncMode::resolve_arg("Two-Way"), Some(SyncMode::TwoWay));
        assert_eq!(SyncMode::resolve_arg("off"), Some(SyncMode::Manual));
    }

    /// Manual modda hiçbir yön otomatik çalışmamalı.
    #[test]
    fn manual_mode_disables_both_directions() {
        let c = resolve_arg("[sync]\nmode = \"manual\"\n");
        assert!(!c.mode_value.accepts_from_studio());
        assert!(!c.mode_value.accepts_from_disk());
    }

    /// Yazım hatası senkronu kırmamalı; varsayılana düşüp uyarmalı.
    #[test]
    fn unknown_mode_falls_back_to_default() {
        let c = resolve_arg("[sync]\nmode = \"disk-to-studioo\"\n");
        assert_eq!(c.mode_value, SyncMode::TwoWay);
    }

    #[test]
    fn safety_and_scope_are_read() {
        let c = resolve_arg(
            "[safety]\ntrash = false\ntrash_keep = 3\ndelete_grace_ms = 1500\nconfirm_delete = false\n\
             \n[scope]\nservices = [\"Workspace\", \"Lighting\"]\nignore_classes = [\"Camera\"]\n\
             ignore_properties = [\"Transparency\"]\n",
        );
        assert!(!c.safety_settings.trash_enabled);
        assert_eq!(c.safety_settings.trash_keep_runs, 3);
        assert_eq!(c.safety_settings.delete_grace_ms, 1500);
        assert!(!c.safety_settings.confirm_delete);
        assert_eq!(c.scope_settings.service_list, vec!["Workspace", "Lighting"]);
        assert!(!c.class_allowed("Camera"));
        assert!(c.class_allowed("Part"));
        assert!(!c.property_allowed("Transparency"));
        assert!(c.property_allowed("Anchored"));
    }

    /// Eski syncix.toml'lar düz yazılmıştı (bölümsüz). Bir yükseltme kimsenin
    /// dosyasını bozmamalı.
    #[test]
    fn flat_legacy_format_still_parses() {
        let c = resolve_arg("sync_dir = \"kaynak\"\nport = 25565\n");
        assert_eq!(c.wanted_port, 25565);
        assert!(c.sync_dir.ends_with("kaynak"), "sync_dir: {}", c.sync_dir);
    }

    /// Saçma değerler kabul edilmemeli: 0 ms debounce sonsuz yazım demek.
    #[test]
    fn out_of_range_values_are_clamped() {
        let c = resolve_arg("[sync]\ndebounce_ms = 0\n\n[safety]\ntrash_keep = 0\n");
        assert!(c.debounce_ms >= 10);
        assert!(c.safety_settings.trash_keep_runs >= 1);
    }
}

#[cfg(test)]
mod place_identity_tests {
    use super::*;

    fn scratch_dir(item_name: &str) -> ProjectConfig {
        let root_dir = std::env::temp_dir().join(format!("syncix-place-{}", item_name));
        let _ = fs::remove_dir_all(&root_dir);
        fs::create_dir_all(&root_dir).unwrap();
        let mut c = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
        c.root = root_dir;
        c
    }

    /// İlk bağlanan place klasörü sahiplenir.
    #[test]
    fn first_connected_place_claims_folder() {
        let c = scratch_dir("ilk");
        assert_eq!(c.linked_place(), None, "yeni klasör bir place'e bağlı olmamalı");
        c.bind_place("place-A");
        assert_eq!(c.linked_place(), Some("place-A".to_string()));
    }

    /// Kimlik core yeniden başlasa da kalmalı: dosyadan okunuyor.
    #[test]
    fn identity_is_stable() {
        let c = scratch_dir("kalici");
        c.bind_place("place-A");
        // Aynı köke bakan ikinci bir yapılandırma nesnesi
        let mut c2 = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
        c2.root = c.root.clone();
        assert_eq!(c2.linked_place(), Some("place-A".to_string()));
    }

    /// Asıl mesele: farklı bir place aynı klasöre bağlanırsa fark edilmeli.
    #[test]
    fn different_place_is_detected() {
        let c = scratch_dir("farkli");
        c.bind_place("place-A");
        let is_bound = c.linked_place().unwrap();
        assert_ne!(is_bound, "place-B", "B, A'ya bağlı klasöre girmemeli");
        // Karar verildikten sonra fresh sahip yazılabilmeli.
        c.bind_place("place-B");
        assert_eq!(c.linked_place(), Some("place-B".to_string()));
    }

    /// Süzgeçler hem eklentide hem core'da uygulanıyor. Core tarafı, previous_text bir
    /// eklenti bağlandığında ayarın yine de geçerli olması için gerekli —
    /// bir süre yalnızca eklentide vardı ve o hâlde ayar sessizce etkisizdi.
    #[test]
    fn filters_reject_excluded() {
        let c = ProjectConfig::resolve_arg(
            &"[scope]
ignore_classes = [\"Camera\", \"Terrain\"]
ignore_properties = [\"Transparency\"]
"
                .parse::<toml::Value>()
                .unwrap(),
            ".",
        );
        assert!(!c.class_allowed("Camera"));
        assert!(!c.class_allowed("Terrain"));
        assert!(c.class_allowed("Part"), "listede olmayan sınıf geçmeli");

        assert!(!c.property_allowed("Transparency"));
        assert!(c.property_allowed("Anchored"), "listede olmayan property geçmeli");
    }

    /// Boş liste "hiçbir şey geçmesin" değil, "kısıtlama yok" demek.
    #[test]
    fn empty_filter_allows_everything() {
        let c = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
        assert!(c.class_allowed("Camera"));
        assert!(c.property_allowed("Transparency"));
    }

    /// Askıya alma üç yerden de görülebilmeli ve restored_count alınabilmeli.
    #[test]
    fn suspension_works() {
        set_sync_suspended(false);
        assert!(!is_sync_suspended());
        set_sync_suspended(true);
        assert!(is_sync_suspended());
        set_sync_suspended(false);
        assert!(!is_sync_suspended());
    }
}
