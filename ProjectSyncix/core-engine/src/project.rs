//! Proje yapılandırması ve kimliği.
//!
//! Buradaki iki şey release için kritik:
//!  1. Port artık sabit değil. syncix.toml'dan okunur, doluysa sıradaki denenir ve
//!     SEÇİLEN port diske yazılır. Editör ve CLI o dosyadan okur, tahmin etmez.
//!  2. Core artık kendini tanıtır (proje adı, kök dizin, sürüm). Studio eklentisi
//!     hangi projeye bağlandığını kullanıcıya gösterebilsin diye gerekli.

use std::fs;
use std::path::{Path, PathBuf};

/// Place kimligi uyusmadiginda senkron askiya alinir.
///
/// Global tutulmasinin sebebi: bayragi okumasi gereken uc yer birbirinden
/// bagimsiz calisiyor (dosya izleyici kendi thread'inde, disk yazicisi kendi
/// gorevinde, komut dongusu ana gorevde). Her birine ayri kanal cekmek yerine
/// tek bir atomik bayrak, bu uc yerin de ayni anda susmasini garanti ediyor.
static SENKRON_ASKIDA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn senkronu_askiya_al(askida: bool) {
    SENKRON_ASKIDA.store(askida, std::sync::atomic::Ordering::SeqCst);
}

/// Senkron askida mi? Askidayken HICBIR yon calismaz: ne disk okunur, ne
/// diske yazilir, ne Studio'ya komut gider. Amac, karar verilene kadar iki
/// tarafi da oldugu gibi birakmak.
pub fn senkron_askida() -> bool {
    SENKRON_ASKIDA.load(std::sync::atomic::Ordering::SeqCst)
}

/// Core'un kendi sürümü (Cargo.toml'dan gelir; tek kaynak).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tel protokolü sürümü. Studio eklentisi ile core arasındaki mesaj biçimi
/// uyumsuz hale geldiğinde ARTTIRILIR. Sürüm numarasından bağımsızdır:
/// 0.3.1 -> 0.3.2 gibi bir yama protokolü bozmaz, bu sayı aynı kalır.
pub const PROTOCOL_VERSION: u32 = 1;

/// Port araması bu aralıkta yapılır. Studio eklentisi de aynı aralığı tarar.
pub const PORT_SCAN_SPAN: u16 = 10;

pub const DEFAULT_PORT: u16 = 8080;

/// Senkronun hangi yonlerde aktif oldugu.
///
/// Rojo tek yonlu calisiyor: dosya sistemi tek dogruluk kaynagi, Studio yalnizca
/// alici. Syncix varsayilan olarak cift yonlu, ama herkes bunu istemiyor —
/// takim halinde calisan biri Studio'yu salt okunur tutmak, tersine bir tasarimci
/// diskin ezilmesini istemeyebilir. Bu yuzden yon bir ayar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncMode {
    /// Iki yon de acik. Varsayilan.
    TwoWay,
    /// Studio -> disk. Studio'da yapilan degisiklik diske yazilir; diskteki
    /// degisiklik Studio'ya GITMEZ. Sahne tasarimini Studio'da yapip kodu
    /// surum kontrolunde tutmak isteyenler icin.
    StudioToDisk,
    /// Disk -> Studio. Rojo'nun calisma sekli: dosya sistemi dogruluk kaynagi.
    DiskToStudio,
    /// Hicbir yon otomatik degil; yalnizca acikca verilen komutlar islenir
    /// (syncix pull, syncix set, ...). Riskli bir sahnede gozetimli calismak icin.
    Manual,
}

impl SyncMode {
    fn coz(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "two_way" | "twoway" | "both" => Some(Self::TwoWay),
            "studio_to_disk" | "studio" | "pull" => Some(Self::StudioToDisk),
            "disk_to_studio" | "disk" | "push" | "rojo" => Some(Self::DiskToStudio),
            "manual" | "off" | "none" => Some(Self::Manual),
            _ => None,
        }
    }

    pub fn adi(&self) -> &'static str {
        match self {
            Self::TwoWay => "two_way",
            Self::StudioToDisk => "studio_to_disk",
            Self::DiskToStudio => "disk_to_studio",
            Self::Manual => "manual",
        }
    }

    /// Studio'da olan bir degisiklik modele ve diske yansitilsin mi?
    pub fn studiodan_kabul(&self) -> bool {
        matches!(self, Self::TwoWay | Self::StudioToDisk)
    }

    /// Diskte olan bir degisiklik Studio'ya gonderilsin mi?
    pub fn diskten_kabul(&self) -> bool {
        matches!(self, Self::TwoWay | Self::DiskToStudio)
    }
}

/// Oyun calisirken (Play) editorden gelen degisikliklere ne olacak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayDavranisi {
    /// Kuyruga alinir, Play bitince uygulanir. Varsayilan.
    Kuyruk,
    /// Atilir. Play sirasinda hicbir sey olmasin diyenler icin.
    Yoksay,
    /// Dogrudan uygulanir. Play bitince Studio oturumla birlikte atacagi icin
    /// degisiklik kaybolur; yalnizca bilerek isteyen acsin.
    Uygula,
}

impl PlayDavranisi {
    fn coz(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "queue" | "kuyruk" => Some(Self::Kuyruk),
            "ignore" | "yoksay" | "drop" => Some(Self::Yoksay),
            "apply" | "uygula" => Some(Self::Uygula),
            _ => None,
        }
    }

    pub fn adi(&self) -> &'static str {
        match self {
            Self::Kuyruk => "queue",
            Self::Yoksay => "ignore",
            Self::Uygula => "apply",
        }
    }
}

/// Silmeye dair guvenlik ayarlari.
#[derive(Clone, Debug)]
pub struct GuvenlikAyarlari {
    /// Uzlastirici sildigi dosyalari cop kutusuna tasisin mi.
    /// Kapatilirsa dosyalar dogrudan silinir ve geri donusu olmaz.
    pub cop_kutusu: bool,
    /// Cop kutusunda saklanacak tur sayisi.
    pub cop_tur_sayisi: usize,
    /// Diskten silinen bir dosyanin gercek silme sayilmasi icin beklenecek sure.
    /// Tasima islemleri isletim sisteminde sil+olustur olarak goruldugu icin
    /// bu pencere gerekiyor. Yavas disklerde arttirilabilir.
    pub silme_bekleme_ms: u64,
    /// `syncix rm` onay istesin mi.
    pub silmeyi_onayla: bool,
}

/// Neyin senkron edilecegini belirleyen ayarlar.
#[derive(Clone, Debug)]
pub struct KapsamAyarlari {
    /// Izlenecek servisler. Bos birakilirsa eklentinin varsayilan listesi gecerli.
    pub servisler: Vec<String>,
    /// Bu siniflar hic senkron edilmez (ornegin "Camera", "Terrain").
    pub sinif_disla: Vec<String>,
    /// Bu property'ler hic senkron edilmez. Gurultulu ya da makineye ozel
    /// alanlari elemek icin.
    pub property_disla: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    /// Senkron klasörü, core'un çalışma dizinine göre (ör. "../src_workspace").
    pub sync_dir: String,
    /// syncix.toml'un bulunduğu dizin, mutlak yol.
    pub root: PathBuf,
    /// Kullanıcıya gösterilecek proje adı (kök klasörün adı).
    pub name: String,
    /// syncix.toml'da istenen port. Dolu olabilir; gerçekte bağlanılan port
    /// `bind_with_fallback` tarafından belirlenir.
    pub wanted_port: u16,
    /// sourcemap.json her senkronda güncellensin mi (luau-lsp için).
    pub sourcemap: bool,
    /// Senkron dışı bırakılacak yollar (glob). Bu dosyalar ne okunur ne silinir.
    pub ignore: Vec<String>,
    /// Port komut satırından açıkça istendiyse true olur ve devretme yapılmaz.
    /// Sebep: kullanıcı Studio eklentisine de aynı portu yazıyor; core sessizce
    /// başka bir porta kayarsa iki taraf ayrışır ve sebebi anlaşılmaz.
    pub port_sabit: bool,

    /// Senkron yonu.
    pub mod_: SyncMode,
    /// Disk yazicisinin bekleme suresi. Kucuk deger daha hizli yansitir ama
    /// yazim sayisini arttirir.
    pub debounce_ms: u64,
    /// Play sirasinda gelen degisikliklerin akibeti.
    pub play: PlayDavranisi,
    /// Ilk baglantida Studio'da izin sorulsun mu.
    pub izin_sor: bool,
    /// Syncix'in yaptigi degisiklikler Studio'nun geri al yigina girsin mi.
    pub geri_al: bool,
    /// Script'lerin yanina .meta.json yazilsin mi. Kapatilirsa script'lerin
    /// property ve attribute'lari diske hic yazilmaz.
    pub meta_dosyalari: bool,
    pub guvenlik: GuvenlikAyarlari,
    pub kapsam: KapsamAyarlari,
}

impl Default for GuvenlikAyarlari {
    fn default() -> Self {
        Self {
            cop_kutusu: true,
            cop_tur_sayisi: 10,
            silme_bekleme_ms: 800,
            silmeyi_onayla: true,
        }
    }
}

impl Default for KapsamAyarlari {
    fn default() -> Self {
        Self {
            servisler: Vec::new(),
            sinif_disla: Vec::new(),
            property_disla: Vec::new(),
        }
    }
}

/// TOML'dan bir bolumu okumak icin kucuk yardimcilar.
/// Bilinmeyen anahtarlar sessizce yutulmaz; cagiran taraf uyari basar.
fn metin_listesi(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn bool_oku(bolum: Option<&toml::Value>, anahtar: &str, varsayilan: bool) -> bool {
    bolum
        .and_then(|b| b.get(anahtar))
        .and_then(|x| x.as_bool())
        .unwrap_or(varsayilan)
}

fn sayi_oku(bolum: Option<&toml::Value>, anahtar: &str, varsayilan: u64) -> u64 {
    bolum
        .and_then(|b| b.get(anahtar))
        .and_then(|x| x.as_integer())
        .and_then(|x| u64::try_from(x).ok())
        .unwrap_or(varsayilan)
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
            return Self::coz(&value, base);
        }

        // syncix.toml yoksa taşınabilir varsayılanlar.
        Self::coz(&toml::Value::Table(Default::default()), "..")
    }

    /// Ayrıştırma testten de çağrılabilsin diye ayrı: yapılandırma davranışı
    /// dosya sistemine bağlı olmadan doğrulanabilmeli.
    pub fn coz(value: &toml::Value, base: &str) -> Self {
        // Anahtarlar hem kök seviyede hem bölüm içinde kabul ediliyor.
        // Sebep: eski syncix.toml'lar düz yazılmıştı ve bir yükseltme kimsenin
        // dosyasını bozmamalı. Bölüm varsa o kazanır.
        let sync = value.get("sync");
        let files = value.get("files");
        let safety = value.get("safety");
        let scope = value.get("scope");
        let server = value.get("server");
        let editor = value.get("editor");

        let al = |bolum: Option<&toml::Value>, anahtar: &str| -> Option<toml::Value> {
            bolum
                .and_then(|b| b.get(anahtar))
                .or_else(|| value.get(anahtar))
                .cloned()
        };

        let dir = al(files, "sync_dir")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "src_workspace".to_string());

        let port = al(server, "port")
            .and_then(|x| x.as_integer())
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or(DEFAULT_PORT);

        let mod_ = al(sync, "mode")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .and_then(|s| {
                let m = SyncMode::coz(&s);
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
                let p = PlayDavranisi::coz(&s);
                if p.is_none() {
                    tracing::warn!(
                        "Unknown play_mode '{}'; falling back to queue.                          Valid values: queue, ignore, apply.",
                        s
                    );
                }
                p
            })
            .unwrap_or(PlayDavranisi::Kuyruk);

        let root = yolu_temizle(fs::canonicalize(base).unwrap_or_else(|_| PathBuf::from(base)));

        Self {
            sync_dir: format!("{}/{}", base, dir),
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Syncix".to_string()),
            root,
            wanted_port: port,
            sourcemap: bool_oku(editor, "sourcemap", true)
                && value.get("sourcemap").and_then(|x| x.as_bool()).unwrap_or(true),
            ignore: metin_listesi(al(files, "ignore").as_ref()),
            port_sabit: false,

            mod_,
            debounce_ms: sayi_oku(sync, "debounce_ms", 120).clamp(10, 10_000),
            play,
            izin_sor: bool_oku(sync, "ask_permission", false),
            geri_al: bool_oku(sync, "undo", true),
            meta_dosyalari: bool_oku(files, "meta_files", true),

            guvenlik: GuvenlikAyarlari {
                cop_kutusu: bool_oku(safety, "trash", true),
                cop_tur_sayisi: sayi_oku(safety, "trash_keep", 10).clamp(1, 500) as usize,
                silme_bekleme_ms: sayi_oku(safety, "delete_grace_ms", 800).clamp(0, 30_000),
                silmeyi_onayla: bool_oku(safety, "confirm_delete", true),
            },
            kapsam: KapsamAyarlari {
                servisler: metin_listesi(al(scope, "services").as_ref()),
                sinif_disla: metin_listesi(al(scope, "ignore_classes").as_ref()),
                property_disla: metin_listesi(al(scope, "ignore_properties").as_ref()),
            },
        }
    }

    /// Bu sinif senkron edilecek mi?
    pub fn sinif_izinli(&self, sinif: &str) -> bool {
        !self.kapsam.sinif_disla.iter().any(|d| d == sinif)
    }

    /// Bu property senkron edilecek mi?
    pub fn property_izinli(&self, ad: &str) -> bool {
        !self.kapsam.property_disla.iter().any(|d| d == ad)
    }

    /// Bu klasorun bagli oldugu place'in kimlik dosyasi: <proje>/.syncix/place
    pub fn place_file(&self) -> PathBuf {
        self.runtime_dir().join("place")
    }

    /// Klasore daha once hangi place baglanmis? Hic baglanmadiysa None.
    pub fn bagli_place(&self) -> Option<String> {
        let icerik = fs::read_to_string(self.place_file()).ok()?;
        let k = icerik.trim().to_string();
        (!k.is_empty()).then_some(k)
    }

    /// Klasoru bir place'e baglar.
    pub fn place_bagla(&self, kimlik: &str) {
        let dir = self.runtime_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Could not create the .syncix folder: {}", e);
            return;
        }
        if let Err(e) = fs::write(self.place_file(), kimlik) {
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
/// Bu yol Studio'daki onay penceresinde kullanıcıya gösterildiği için temizlenir.
fn yolu_temizle(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(kalan) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(kalan);
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
    fn yama_farki_uyumlu_minor_farki_degil() {
        assert!(versions_compatible("0.3.1", "0.3.9"));
        assert!(versions_compatible("1.0.0", "1.0.0"));
        assert!(!versions_compatible("0.3.0", "0.4.0"));
        assert!(!versions_compatible("0.3.0", "1.3.0"));
    }

    #[test]
    fn bozuk_surum_metni_cokmez() {
        assert!(versions_compatible("abc", "abc"));
        assert!(!versions_compatible("0.3.0", "abc"));
    }
}

#[cfg(test)]
mod yapilandirma_testleri {
    use super::*;

    fn coz(toml_metni: &str) -> ProjectConfig {
        ProjectConfig::coz(&toml_metni.parse::<toml::Value>().unwrap(), ".")
    }

    #[test]
    fn bos_dosya_varsayilanlari_verir() {
        let c = coz("");
        assert_eq!(c.mod_, SyncMode::TwoWay);
        assert_eq!(c.play, PlayDavranisi::Kuyruk);
        assert_eq!(c.wanted_port, DEFAULT_PORT);
        assert!(c.guvenlik.cop_kutusu);
        assert!(c.guvenlik.silmeyi_onayla);
        assert!(c.geri_al);
    }

    /// Asıl istenen ayar: Rojo gibi tek yönlü çalışabilmek.
    #[test]
    fn tek_yonlu_mod() {
        let c = coz("[sync]\nmode = \"disk_to_studio\"\n");
        assert_eq!(c.mod_, SyncMode::DiskToStudio);
        assert!(c.mod_.diskten_kabul(), "disk -> Studio açık olmalı");
        assert!(!c.mod_.studiodan_kabul(), "Studio -> disk kapalı olmalı");

        let t = coz("[sync]\nmode = \"studio_to_disk\"\n");
        assert!(t.mod_.studiodan_kabul());
        assert!(!t.mod_.diskten_kabul());
    }

    /// "rojo" ve "push" gibi takma adlar aynı modu vermeli: kullanıcı hangi
    /// kelimeyi aklında tutuyorsa onu yazabilmeli.
    #[test]
    fn mod_takma_adlari() {
        assert_eq!(SyncMode::coz("rojo"), Some(SyncMode::DiskToStudio));
        assert_eq!(SyncMode::coz("push"), Some(SyncMode::DiskToStudio));
        assert_eq!(SyncMode::coz("PULL"), Some(SyncMode::StudioToDisk));
        assert_eq!(SyncMode::coz("Two-Way"), Some(SyncMode::TwoWay));
        assert_eq!(SyncMode::coz("off"), Some(SyncMode::Manual));
    }

    /// Manual modda hiçbir yön otomatik çalışmamalı.
    #[test]
    fn manual_mod_iki_yonu_de_kapatir() {
        let c = coz("[sync]\nmode = \"manual\"\n");
        assert!(!c.mod_.studiodan_kabul());
        assert!(!c.mod_.diskten_kabul());
    }

    /// Yazım hatası senkronu kırmamalı; varsayılana düşüp uyarmalı.
    #[test]
    fn bilinmeyen_mod_varsayilana_duser() {
        let c = coz("[sync]\nmode = \"disk-to-studioo\"\n");
        assert_eq!(c.mod_, SyncMode::TwoWay);
    }

    #[test]
    fn guvenlik_ve_kapsam_okunur() {
        let c = coz(
            "[safety]\ntrash = false\ntrash_keep = 3\ndelete_grace_ms = 1500\nconfirm_delete = false\n\
             \n[scope]\nservices = [\"Workspace\", \"Lighting\"]\nignore_classes = [\"Camera\"]\n\
             ignore_properties = [\"Transparency\"]\n",
        );
        assert!(!c.guvenlik.cop_kutusu);
        assert_eq!(c.guvenlik.cop_tur_sayisi, 3);
        assert_eq!(c.guvenlik.silme_bekleme_ms, 1500);
        assert!(!c.guvenlik.silmeyi_onayla);
        assert_eq!(c.kapsam.servisler, vec!["Workspace", "Lighting"]);
        assert!(!c.sinif_izinli("Camera"));
        assert!(c.sinif_izinli("Part"));
        assert!(!c.property_izinli("Transparency"));
        assert!(c.property_izinli("Anchored"));
    }

    /// Eski syncix.toml'lar düz yazılmıştı (bölümsüz). Bir yükseltme kimsenin
    /// dosyasını bozmamalı.
    #[test]
    fn bolumsuz_eski_bicim_hala_okunur() {
        let c = coz("sync_dir = \"kaynak\"\nport = 25565\n");
        assert_eq!(c.wanted_port, 25565);
        assert!(c.sync_dir.ends_with("kaynak"), "sync_dir: {}", c.sync_dir);
    }

    /// Saçma değerler kabul edilmemeli: 0 ms debounce sonsuz yazım demek.
    #[test]
    fn sinir_disi_degerler_kirpilir() {
        let c = coz("[sync]\ndebounce_ms = 0\n\n[safety]\ntrash_keep = 0\n");
        assert!(c.debounce_ms >= 10);
        assert!(c.guvenlik.cop_tur_sayisi >= 1);
    }
}

#[cfg(test)]
mod place_kimligi_testleri {
    use super::*;

    fn gecici(ad: &str) -> ProjectConfig {
        let kok = std::env::temp_dir().join(format!("syncix-place-{}", ad));
        let _ = fs::remove_dir_all(&kok);
        fs::create_dir_all(&kok).unwrap();
        let mut c = ProjectConfig::coz(&toml::Value::Table(Default::default()), ".");
        c.root = kok;
        c
    }

    /// İlk bağlanan place klasörü sahiplenir.
    #[test]
    fn ilk_baglanan_sahiplenir() {
        let c = gecici("ilk");
        assert_eq!(c.bagli_place(), None, "yeni klasör bir place'e bağlı olmamalı");
        c.place_bagla("place-A");
        assert_eq!(c.bagli_place(), Some("place-A".to_string()));
    }

    /// Kimlik core yeniden başlasa da kalmalı: dosyadan okunuyor.
    #[test]
    fn kimlik_kalici() {
        let c = gecici("kalici");
        c.place_bagla("place-A");
        // Aynı köke bakan ikinci bir yapılandırma nesnesi
        let mut c2 = ProjectConfig::coz(&toml::Value::Table(Default::default()), ".");
        c2.root = c.root.clone();
        assert_eq!(c2.bagli_place(), Some("place-A".to_string()));
    }

    /// Asıl mesele: farklı bir place aynı klasöre bağlanırsa fark edilmeli.
    #[test]
    fn farkli_place_fark_edilir() {
        let c = gecici("farkli");
        c.place_bagla("place-A");
        let bagli = c.bagli_place().unwrap();
        assert_ne!(bagli, "place-B", "B, A'ya bağlı klasöre girmemeli");
        // Karar verildikten sonra yeni sahip yazılabilmeli.
        c.place_bagla("place-B");
        assert_eq!(c.bagli_place(), Some("place-B".to_string()));
    }

    /// Askıya alma üç yerden de görülebilmeli ve geri alınabilmeli.
    #[test]
    fn askiya_alma_calisir() {
        senkronu_askiya_al(false);
        assert!(!senkron_askida());
        senkronu_askiya_al(true);
        assert!(senkron_askida());
        senkronu_askiya_al(false);
        assert!(!senkron_askida());
    }
}
