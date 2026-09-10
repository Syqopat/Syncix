use crate::model::{DataModel, InstanceNode};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// KENDI YAZIMLARIMIZIN KAYDI
//
// Sorun: disk yazicisi bir dosyayi yazdiginda dosya izleyici bunu bir KULLANICI
// degisikligi sanip modele geri uyguluyordu. Olay kuyrugu gecikmeli oldugu icin
// izleyici bazen dosyanin ESKI halini okuyor ve modeli geriye sariyordu.
//
// Gozlemlenen sonuc: diskten eklenen bir attribute bazen kaliyor bazen kayboluyordu
// (DiskTenGelen kayboldu, OyunSurumu kaldi) — davranis yaris kosuluna bagliydi.
//
// Cozum: yazdigimiz her dosyanin icerigini not ediyoruz. Izleyici bir dosyayi
// okudugunda icerik son yazdigimizla ayniysa ogrenecek yeni bir sey yoktur, atlanir.
// Zaman penceresi kullanilmiyor; karsilastirma icerik uzerinden yapildigi icin
// gecikmeli olaylar da dogru elenir.
// ---------------------------------------------------------------------------

fn yazim_kaydi() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static KAYIT: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    KAYIT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn icerik_ozeti(icerik: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    icerik.hash(&mut h);
    h.finish()
}

fn yazimi_not_et(path: &Path, icerik: &str) {
    if let Ok(mut kayit) = yazim_kaydi().lock() {
        kayit.insert(path.to_path_buf(), icerik_ozeti(icerik));
    }
}

fn yazim_kaydindan_sil(path: &Path) {
    if let Ok(mut kayit) = yazim_kaydi().lock() {
        kayit.remove(path);
    }
}

/// Bu icerik bizim en son yazdigimiz mi? Oyleyse izleyici bunu islememeli.
pub fn kendi_yazimimiz(path: &Path, icerik: &str) -> bool {
    yazim_kaydi()
        .lock()
        .map(|k| k.get(path) == Some(&icerik_ozeti(icerik)))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// KENDI SILMELERIMIZIN KAYDI
//
// Yazma kaydinin silme tarafindaki esi. Uzlastirici agaci yeniden yazarken
// dosya siliyor; izleyici bunlari kullanici silmesi sanarsa var olan objeleri
// yok eder. Bu yuzden dosya silme olaylari tamamen yok sayiliyordu — ama o
// zaman da editorde bir dosyayi silmek hicbir sey yapmiyor, dosya birkac yuz
// milisaniye sonra geri beliriyordu.
//
// Cozum yazma tarafiyla ayni: sildigimiz yollari not ediyoruz. Izleyiciye
// gelen silme olayi bu listede varsa bizimdir, atlanir; yoksa kullanici
// silmistir ve instance gercekten yok edilir.
// ---------------------------------------------------------------------------

fn silme_kaydi() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    static KAYIT: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    KAYIT.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn silmeyi_not_et(path: &Path) {
    if let Ok(mut k) = silme_kaydi().lock() {
        k.insert(path.to_path_buf());
    }
}

/// Bu silme bizim mi? Kayit TEK KULLANIMLIK: sorulan yol listeden dusurulur.
/// Boylece ayni yol daha sonra kullanici tarafindan silinirse gercek silme
/// olarak islenir.
pub fn kendi_silmemiz(path: &Path) -> bool {
    silme_kaydi()
        .lock()
        .map(|mut k| k.remove(path))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// ÇÖP KUTUSU
//
// Uzlaştırıcı, Studio'nun ağacında karşılığı olmayan dosyaları siler. Bu doğru
// davranış — ama tek yönlü. Core kapalıyken diskte yapılan bir düzenlemeyi
// kimse görmemiş olur; core açılıp Studio'nun ağacını yazdığında o dosya
// "fazlalık" sayılıp yok olur ve geri dönüşü yoktur.
//
// Bu yüzden yönettiğimiz hiçbir dosya doğrudan silinmez, çöp kutusuna taşınır.
// Kutu sync klasörünün DIŞINDA: içeride olsaydı izleyici onu yeni içerik sanar,
// uzlaştırıcı da bir sonraki turda silerdi.
// ---------------------------------------------------------------------------

/// <sync_dir>/../.syncix/trash
fn cop_kokü(sync_dir: &str) -> PathBuf {
    let s = Path::new(sync_dir);
    let ust = s.parent().unwrap_or(s);
    ust.join(".syncix").join("trash")
}

/// En yeni KORUNAN_TUR kadar klasör tutulur; eskiler tamamen silinir.
const KORUNAN_TUR: usize = 10;

/// Aynı core oturumu içindeki tüm silmeler tek klasörde toplansın diye
/// tur adı bir kez üretilip saklanır.
fn tur_adi() -> String {
    static TUR: OnceLock<String> = OnceLock::new();
    TUR.get_or_init(|| chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string())
        .clone()
}

/// Dosyayı silmek yerine çöp kutusuna taşır. Sync klasörüne göre göreli yol
/// korunur, böylece geri koymak düz bir kopyalama işi olur.
fn cope_tasi(path: &Path, sync_dir: &str) -> std::io::Result<()> {
    // Cop kutusu kapaliysa dosya dogrudan silinir. Bunu isteyen biri geri
    // donusu olmadigini bilerek istiyor; ayar aciklamasinda da yaziyor.
    if !cop_ayari().0 {
        return fs::remove_file(path);
    }
    let goreli = path.strip_prefix(sync_dir).unwrap_or(path);
    let hedef = cop_kokü(sync_dir).join(tur_adi()).join(goreli);
    if let Some(ust) = hedef.parent() {
        fs::create_dir_all(ust)?;
    }
    // rename aynı disk bölümünde ucuz; farklı bölümdeyse kopyala-sil'e düşer.
    if fs::rename(path, &hedef).is_err() {
        fs::copy(path, &hedef)?;
        fs::remove_file(path)?;
    }
    cop_kutusunu_buda(sync_dir);
    Ok(())
}

/// Kutu sınırsız büyümemeli: en yeni KORUNAN_TUR klasör kalır.
fn cop_kutusunu_buda(sync_dir: &str) {
    let kok = cop_kokü(sync_dir);
    let Ok(girdiler) = fs::read_dir(&kok) else {
        return;
    };
    let mut turlar: Vec<PathBuf> = girdiler
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    let tutulacak = cop_ayari().1;
    if turlar.len() <= tutulacak {
        return;
    }
    // Klasör adı zaman damgası olduğu için isim sıralaması zaman sıralamasıdır.
    turlar.sort();
    let silinecek = turlar.len() - tutulacak;
    for eski in turlar.into_iter().take(silinecek) {
        let _ = fs::remove_dir_all(eski);
    }
}

/// Çöp kutusundaki turları yeniden eskiye doğru listeler: (tur adı, dosya sayısı).
pub fn cop_turlari(sync_dir: &str) -> Vec<(String, usize)> {
    let kok = cop_kokü(sync_dir);
    let Ok(girdiler) = fs::read_dir(&kok) else {
        return Vec::new();
    };
    let mut turlar: Vec<PathBuf> = girdiler
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    turlar.sort();
    turlar.reverse();
    turlar
        .into_iter()
        .map(|t| {
            let mut dosyalar = Vec::new();
            collect_files(&t, &mut dosyalar);
            (
                t.file_name().unwrap_or_default().to_string_lossy().to_string(),
                dosyalar.len(),
            )
        })
        .collect()
}

/// Bir turdaki dosyaları sync klasörüne geri koyar. Var olan dosyanın üzerine
/// yazılmaz — geri koyma kendi başına bir veri kaybı olmamalı.
/// Döner: (geri konan, üzerine yazılmadığı için atlanan).
pub fn coptan_geri_al(sync_dir: &str, tur: &str) -> (usize, usize) {
    let kaynak = cop_kokü(sync_dir).join(tur);
    let mut dosyalar = Vec::new();
    collect_files(&kaynak, &mut dosyalar);

    let (mut geri, mut atlanan) = (0usize, 0usize);
    for d in dosyalar {
        let Ok(goreli) = d.strip_prefix(&kaynak) else {
            continue;
        };
        let hedef = Path::new(sync_dir).join(goreli);
        if hedef.exists() {
            atlanan += 1;
            continue;
        }
        if let Some(ust) = hedef.parent() {
            let _ = fs::create_dir_all(ust);
        }
        if fs::copy(&d, &hedef).is_ok() {
            geri += 1;
        }
    }
    (geri, atlanan)
}

/// Diskteki bir yolun hangi instance'a ait oldugunu bulur.
///
/// Ters yonde (uuid -> yol) `data_file` var; silme olayini isleyebilmek icin
/// yolun kendisinden yola cikmak gerekiyor, cunku dosya artik okunamiyor.
/// Yalnizca VERI dosyasi eslesirse uuid doner: meta dosyasinin silinmesi
/// instance'in silinmesi degildir.
/// Bir yol, beklenen yolun bilesen bazli soneki mi?
///
/// Duz esitlik ise yaramiyor: izleyici MUTLAK yol bildiriyor, data_file ise
/// sync_dir'den baslayan GORELI yol uretiyor. Path::ends_with tek basina da
/// yetmiyor, cunku sync_dir cogu zaman "./src_workspace" seklinde geliyor ve
/// bastaki "." ayri bir bilesen sayilip eslesmeyi bozuyor. Bu yuzden "." ve ""
/// bilesenleri her iki tarafta da atiliyor.
///
/// Sonek icinde sync_dir de bulundugu icin yanlis eslesme pratikte mumkun degil.
fn sonek_eslesir(yol: &Path, beklenen: &Path) -> bool {
    use std::path::Component;
    let temizle = |p: &Path| -> Vec<std::ffi::OsString> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_os_string())
            .collect()
    };
    let a = temizle(yol);
    let b = temizle(beklenen);
    if b.is_empty() || b.len() > a.len() {
        return false;
    }
    a[a.len() - b.len()..] == b[..]
}

pub fn yol_icin_uuid(dm: &DataModel, sync_dir: &str, path: &Path) -> Option<Uuid> {
    for uuid in dm.get_all_instances().keys() {
        let Some(beklenen) = data_file(dm, sync_dir, uuid) else {
            continue;
        };
        if sonek_eslesir(path, &beklenen) {
            return Some(*uuid);
        }
    }
    None
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*\0".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn seg(dm: &DataModel, node: &InstanceNode) -> String {
    let clean = sanitize(&node.name);
    let has_sibling_collision = if let Some(pid) = node.parent {
        dm.get_instance(&pid).map(|p| {
            p.children.iter().any(|cid| {
                cid != &node.syncix_id && dm.get_instance(cid).map(|cn| sanitize(&cn.name) == clean).unwrap_or(false)
            })
        }).unwrap_or(false)
    } else {
        false
    };

    if has_sibling_collision {
        let short = &node.syncix_id.to_string()[0..8];
        format!("{}_{}", clean, short)
    } else {
        clean
    }
}

fn script_ext(class_name: &str) -> Option<&'static str> {
    match class_name {
        "Script" => Some("server.lua"),
        "LocalScript" => Some("client.lua"),
        "ModuleScript" => Some("lua"),
        // StringValue'nun tek anlamli alani Value'dur; onu JSON icine gomup
        // kacis karakterleriyle ugrastirmak yerine duz metin dosyasi olarak
        // yaziyoruz. Boylece metin icerigi editorde dogrudan duzenlenebiliyor.
        "StringValue" => Some("txt"),
        // LocalizationTable.Contents bir JSON metnidir; diske CSV olarak yaziyoruz
        // ki ceviriler Excel/Sheets ile duzenlenebilsin. Donusum kayipsizdir ve
        // localization.rs icindeki gidis-donus testleriyle kilitlenmistir.
        "LocalizationTable" => Some("csv"),
        _ => None,
    }
}

fn is_script(node: &InstanceNode) -> bool {
    matches!(
        node.class_name.as_str(),
        "Script" | "LocalScript" | "ModuleScript"
    )
}

/// Bu sinif diske HAM ICERIK olarak mi yaziliyor? (kendi .json'u yerine)
/// Boyle siniflarin property/attribute'lari .meta.json dosyasinda tutulur.
fn ham_icerikli(node: &InstanceNode) -> bool {
    script_ext(&node.class_name).is_some()
}

/// StringValue gibi siniflarda diske yazilacak ham metin.
fn ham_metin(node: &InstanceNode) -> String {
    if is_script(node) {
        return node.source.clone().unwrap_or_default();
    }

    if node.class_name == "LocalizationTable" {
        let contents = match node.properties.get("Contents") {
            Some(crate::model::PropertyValue::String(s)) => s.as_str(),
            _ => "[]",
        };
        return match crate::localization::json_to_csv(contents) {
            Ok(csv) => csv,
            Err(e) => {
                tracing::warn!("Could not convert LocalizationTable to CSV ({}): {}", node.name, e);
                String::new()
            }
        };
    }

    match node.properties.get("Value") {
        Some(crate::model::PropertyValue::String(s)) => s.clone(),
        Some(diger) => format!("{:?}", diger),
        None => String::new(),
    }
}

fn ancestry(dm: &DataModel, uuid: &Uuid) -> Vec<Uuid> {
    let mut chain = Vec::new();
    let mut cur = Some(*uuid);
    let mut guard = 0;
    while let Some(id) = cur {
        chain.push(id);
        cur = dm.get_instance(&id).and_then(|n| n.parent);
        guard += 1;
        if guard > 512 {
            break;
        }
    }
    chain.reverse();
    chain
}

/// Bir objenin diskteki dosya yolu. sourcemap üretimi de bunu kullanır ki
/// yol hesabı tek yerde kalsın.
pub fn data_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    let node = dm.get_instance(uuid)?;
    let chain = ancestry(dm, uuid);

    let mut path = PathBuf::from(sync_dir);
    if chain.len() > 1 {
        for aid in &chain[..chain.len() - 1] {
            if let Some(n) = dm.get_instance(aid) {
                path.push(seg(dm, n));
            }
        }
    }

    let ext = script_ext(&node.class_name).unwrap_or("json");
    if node.children.is_empty() {
        path.push(format!("{}.{}", seg(dm, node), ext));
    } else {
        path.push(seg(dm, node));
        path.push(format!("init.{}", ext));
    }
    Some(path)
}

/// Script dosyalarinin yaninda duran property/attribute dosyasi.
///
/// Neden gerekli: script'ler diske ham kaynak kod olarak yaziliyor (.lua), dolayisiyla
/// property'leri ve attribute'lari icin yer yok. Bu yuzden Disabled, RunContext gibi
/// alanlar ve TUM attribute'lar disk tarafinda kayboluyordu. Rojo'daki .meta.json
/// fikrinin sade hali: `$`'li sihirli anahtarlar yok, yalnizca iki alan var.
///
/// Yalnizca yazacak bir sey varsa uretilir; bos meta dosyalariyla klasor kirletilmez.
fn meta_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    if !META_ACIK.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let node = dm.get_instance(uuid)?;
    if !ham_icerikli(node) {
        return None; // diger objelerin kendi .json dosyasi zaten her seyi tutuyor
    }
    // StringValue'da Value zaten .txt dosyasinda; meta'da tekrarlanmasi anlamsiz.
    let deger_disi_property = node
        .properties
        .keys()
        .any(|k| is_script(node) || (k != "Value" && k != "Contents"));
    // Etiketler de yazilacak bir sey: yalnizca etiketi olan bir script'in meta
    // dosyasi uretilmezse etiket diske hic ulasmaz.
    if !deger_disi_property && node.attributes.is_empty() && node.tags.is_empty() {
        return None;
    }
    let script_path = data_file(dm, sync_dir, uuid)?;
    let dir = script_path.parent()?;
    let ad = if node.children.is_empty() {
        format!("{}.meta.json", seg(dm, node))
    } else {
        "init.meta.json".to_string()
    };
    Some(dir.join(ad))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ScriptMeta {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub properties: std::collections::BTreeMap<String, crate::model::PropertyValue>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub attributes: std::collections::BTreeMap<String, crate::model::PropertyValue>,
    /// CollectionService etiketleri. Bos ise dosyaya hic yazilmaz.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn meta_content(node: &InstanceNode) -> String {
    let mut properties = node.properties.clone();
    // StringValue'nun Value'su .txt dosyasinda tutuluyor; iki yerde tutmak
    // ikisinin ayrisma riskini dogurur.
    if !is_script(node) {
        // Ham icerik dosyasinda tutulan alanlar meta'da TEKRARLANMAZ;
        // ayni veriyi iki yerde tutmak ikisinin ayrisma riskini dogurur.
        properties.remove("Value");
        properties.remove("Contents");
    }
    let meta = ScriptMeta {
        properties,
        attributes: node.attributes.clone(),
        tags: node.tags.clone(),
    };
    serde_json::to_string_pretty(&meta).unwrap_or_default()
}

fn node_content(node: &InstanceNode) -> String {
    if ham_icerikli(node) {
        ham_metin(node)
    } else {
        serde_json::to_string_pretty(node).unwrap_or_default()
    }
}

/// Bu dosya Syncix'in ÜRETEBİLECEĞİ bir dosya mı?
///
/// Reconcile yazıcısı beklenen listede olmayan dosyaları siliyordu; bu yüzden
/// senkron klasörüne konan herhangi bir dosya (README, .gitkeep, kişisel not)
/// sessizce yok oluyordu. Ölçüldü ve doğrulandı: NOTLAR.md dosyası ilk yazımda
/// silindi.
///
/// Rojo'da bu sorun yok çünkü Rojo diske hiç yazmıyor. Bizde tek yönlü bir
/// "ignore listesi" yetmez; ASIL kural şudur: yalnızca kendi üretebileceğimiz
/// biçimdeki dosyalara dokunuruz. Tanımadığımız hiçbir dosya silinmez.
fn yonettigimiz_dosya(path: &Path) -> bool {
    let Some(ad) = path.file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    ad.ends_with(".meta.json")
        || ad.ends_with(".server.lua")
        || ad.ends_with(".client.lua")
        || ad.ends_with(".lua")
        || ad.ends_with(".luau")
        || ad.ends_with(".txt")
        || ad.ends_with(".csv")
        || ad.ends_with(".json")
}

/// Yol, kullanıcının belirlediği ignore desenlerinden birine uyuyor mu?
/// Desenler senkron klasörüne göre değerlendirilir (ör. "notlar/**", "*.md").
pub fn yok_sayilir(path: &Path, sync_dir: &str, desenler: &[String]) -> bool {
    if desenler.is_empty() {
        return false;
    }
    let gorece = path.strip_prefix(sync_dir).unwrap_or(path);
    let metin = gorece.to_string_lossy().replace('\\', "/");

    desenler.iter().any(|d| {
        glob::Pattern::new(d)
            .map(|p| p.matches(&metin))
            .unwrap_or(false)
    })
}

/// Klasördeki tüm dosyaları (özyinelemeli) toplar.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

/// İçi boşalan klasörleri (yapraktan köke doğru) temizler. Kök silinmez.
fn remove_empty_dirs(dir: &Path, root: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                remove_empty_dirs(&p, root);
            }
        }
    }
    if dir != root {
        if let Ok(mut it) = fs::read_dir(dir) {
            if it.next().is_none() {
                let _ = fs::remove_dir(dir);
            }
        }
    }
}

/// Modeli diske yansıtır — RECONCILE yöntemiyle.
/// Eskiden tüm klasör silinip yeniden yazılıyordu; bu hem büyük sahnelerde yavaştı
/// hem de her yazımda dosya izleyiciyi gereksiz tetikliyordu. Artık yalnızca fark
/// uygulanır: içeriği aynı olan dosyalara hiç dokunulmaz.
/// Cop kutusu davranisi yapilandirmadan geliyor. Global tutulmasinin sebebi
/// write_full_tree'nin cagri zincirinin uzun olmasi ve tek bir ayar icin her
/// halkaya parametre eklemenin kodu okunmaz hale getirmesi.
static COP_AYARI: OnceLock<Mutex<(bool, usize)>> = OnceLock::new();

/// .meta.json dosyalari yazilsin mi. Kapaliysa script'lerin property ve
/// attribute'lari diske hic yazilmaz; yalnizca kaynak kod dosyasi kalir.
static META_ACIK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn meta_ayarini_kur(acik: bool) {
    META_ACIK.store(acik, std::sync::atomic::Ordering::Relaxed);
}

pub fn cop_ayarini_kur(acik: bool, tur_sayisi: usize) {
    let h = COP_AYARI.get_or_init(|| Mutex::new((true, KORUNAN_TUR)));
    if let Ok(mut a) = h.lock() {
        *a = (acik, tur_sayisi.max(1));
    }
}

fn cop_ayari() -> (bool, usize) {
    COP_AYARI
        .get()
        .and_then(|h| h.lock().ok().map(|a| *a))
        .unwrap_or((true, KORUNAN_TUR))
}

pub fn write_full_tree(dm: &DataModel, sync_dir: &str, ignore: &[String]) {
    let root = Path::new(sync_dir);
    let _ = fs::create_dir_all(root);

    // 1) Olması gereken dosyalar
    let mut expected: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    for (uuid, node) in dm.get_all_instances() {
        if node.class_name == "DataModel" {
            continue;
        }
        if let Some(file) = data_file(dm, sync_dir, uuid) {
            expected.insert(file, node_content(node));
        }
        // Script'lerin property/attribute'lari ayri bir meta dosyasinda tutulur.
        if let Some(meta) = meta_file(dm, sync_dir, uuid) {
            expected.insert(meta, meta_content(node));
        }
    }

    // 2) Diskte olan dosyalar
    let mut existing = Vec::new();
    collect_files(root, &mut existing);

    // 3) Fazlalıkları sil
    let mut removed = 0usize;
    for path in &existing {
        if expected.contains_key(path) {
            continue;
        }
        // Tanımadığımız dosyalara ASLA dokunma (README, .gitkeep, kişisel notlar).
        if !yonettigimiz_dosya(path) {
            continue;
        }
        // Kullanıcının yok saydırdığı yollar da korunur.
        if yok_sayilir(path, sync_dir, ignore) {
            continue;
        }
        if cope_tasi(path, sync_dir).is_ok() {
            yazim_kaydindan_sil(path);
            silmeyi_not_et(path);
            removed += 1;
        }
    }

    // 4) Eksik/değişmiş olanları yaz (aynıysa dokunma)
    let mut written = 0usize;
    for (path, content) in &expected {
        let unchanged = fs::read_to_string(path)
            .map(|current| current == *content)
            .unwrap_or(false);
        if unchanged {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(path, content) {
            Ok(_) => {
                yazimi_not_et(path, content);
                written += 1;
            }
            Err(e) => tracing::error!("layout: could not write file ({:?}): {}", path, e),
        }
    }

    // 5) Boşalan klasörleri temizle
    remove_empty_dirs(root, root);

    if written > 0 || removed > 0 {
        tracing::debug!(
            "layout: synced ({} instances) — {} written, {} removed",
            expected.len(),
            written,
            removed
        );
    }
}
#[cfg(test)]
mod meta_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn ekle(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// Ozelligi olmayan script icin meta dosyasi URETILMEZ.
    /// Aksi halde her script'in yaninda bos bir .meta.json birikirdi.
    #[test]
    fn ozelliksiz_script_meta_dosyasi_uretmez() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        assert!(meta_file(&m, "src", &s).is_none());
    }

    /// Property ya da attribute varsa meta dosyasi script'in YANINDA olusur.
    #[test]
    fn property_varsa_meta_dosyasi_script_yaninda_olusur() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let meta = meta_file(&m, "src", &s).expect("meta dosyasi bekleniyordu");
        let script = data_file(&m, "src", &s).unwrap();
        assert_eq!(meta.parent(), script.parent(), "ayni klasorde olmali");
        assert!(meta.to_string_lossy().ends_with("Ana.meta.json"), "{:?}", meta);
    }

    #[test]
    fn attribute_tek_basina_da_meta_uretir() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .attributes
            .insert("Surum".into(), PropertyValue::Number(3.0));

        assert!(meta_file(&m, "src", &s).is_some());
    }

    /// Cocugu olan script konteyner klasore doner; meta adi da init.meta.json olur.
    #[test]
    fn konteyner_script_init_meta_kullanir() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        ekle(&mut m, "ModuleScript", "Alt", Some(s));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let meta = meta_file(&m, "src", &s).unwrap();
        assert!(meta.to_string_lossy().ends_with("init.meta.json"), "{:?}", meta);
    }

    /// Script olmayan objeler icin meta URETILMEZ: onlarin kendi .json dosyasi
    /// zaten property ve attribute'lari tutuyor, ikinci bir dosya kafa karistirir.
    #[test]
    fn script_disi_obje_meta_uretmez() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        let p = ekle(&mut m, "Part", "Kutu", Some(ws));
        m.get_mut_instance(&p)
            .unwrap()
            .properties
            .insert("Anchored".into(), PropertyValue::Boolean(true));

        assert!(meta_file(&m, "src", &p).is_none());
    }

    /// Meta icerigi gidis-donus yapabilmeli.
    #[test]
    fn meta_icerigi_gidis_donus() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        {
            let n = m.get_mut_instance(&s).unwrap();
            n.properties.insert("Disabled".into(), PropertyValue::Boolean(true));
            n.properties.insert(
                "RunContext".into(),
                PropertyValue::String("Enum.RunContext.Server".into()),
            );
            n.attributes.insert("Surum".into(), PropertyValue::Number(2.0));
        }

        let metin = meta_content(m.get_instance(&s).unwrap());
        let geri: ScriptMeta = serde_json::from_str(&metin).expect("cozulemedi");
        assert_eq!(geri.properties.len(), 2);
        assert_eq!(
            geri.properties.get("RunContext"),
            Some(&PropertyValue::String("Enum.RunContext.Server".into()))
        );
        assert_eq!(geri.attributes.get("Surum"), Some(&PropertyValue::Number(2.0)));
    }
}

#[cfg(test)]
mod txt_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn ekle(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// StringValue diske .txt olarak yazilir, .json olarak degil.
    #[test]
    fn stringvalue_txt_dosyasina_yazilir() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = ekle(&mut m, "StringValue", "Mesaj", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("merhaba".into()));

        let yol = data_file(&m, "src", &sv).unwrap();
        assert!(yol.to_string_lossy().ends_with("Mesaj.txt"), "{:?}", yol);
    }

    /// Dosyanin icerigi dogrudan Value'dur; JSON sarmalayici yok.
    #[test]
    fn txt_icerigi_dogrudan_degerdir() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = ekle(&mut m, "StringValue", "Mesaj", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("satir1\nsatir2".into()));

        let icerik = node_content(m.get_instance(&sv).unwrap());
        assert_eq!(icerik, "satir1\nsatir2");
    }

    /// Value .txt dosyasinda oldugu icin meta dosyasinda TEKRARLANMAZ;
    /// iki yerde tutmak ikisinin ayrisma riskini dogurur.
    #[test]
    fn value_meta_dosyasinda_tekrarlanmaz() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = ekle(&mut m, "StringValue", "Mesaj", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("merhaba".into()));

        // Yalnizca Value varsa meta dosyasina gerek yok
        assert!(meta_file(&m, "src", &sv).is_none());

        // Attribute eklenince meta dosyasi olusur ama Value icermez
        m.get_mut_instance(&sv)
            .unwrap()
            .attributes
            .insert("Dil".into(), PropertyValue::String("tr".into()));
        assert!(meta_file(&m, "src", &sv).is_some());

        let icerik = meta_content(m.get_instance(&sv).unwrap());
        assert!(!icerik.contains("Value"), "Value meta'da olmamali: {}", icerik);
        assert!(icerik.contains("Dil"));
    }

    /// Script'lerde Source ayri bir alan oldugu icin property'ler meta'da kalir.
    #[test]
    fn scriptte_propertyler_meta_da_kalir() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let icerik = meta_content(m.get_instance(&s).unwrap());
        assert!(icerik.contains("Disabled"));
    }

    /// StringValue disindaki ValueBase siniflari hala .json kullanir:
    /// sayisal bir degeri duz metne cevirmek tip bilgisini kaybettirirdi.
    #[test]
    fn intvalue_json_kalir() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let iv = ekle(&mut m, "IntValue", "Sayac", Some(rs));

        let yol = data_file(&m, "src", &iv).unwrap();
        assert!(yol.to_string_lossy().ends_with("Sayac.json"), "{:?}", yol);
    }
}

#[cfg(test)]
mod koruma_tests {
    use super::*;

    /// Tanimadigimiz dosyalara ASLA dokunulmaz.
    /// Bu, senkron klasorune konan README/not dosyalarinin silinmesi bugunun testi.
    #[test]
    fn yabanci_dosyalar_yonetilmez() {
        assert!(!yonettigimiz_dosya(Path::new("src/NOTLAR.md")));
        assert!(!yonettigimiz_dosya(Path::new("src/.gitkeep")));
        assert!(!yonettigimiz_dosya(Path::new("src/resim.png")));
        assert!(!yonettigimiz_dosya(Path::new("src/rapor.pdf")));
    }

    /// Kendi uretebilecegimiz bicimler yonetilir.
    #[test]
    fn kendi_bicimlerimiz_yonetilir() {
        assert!(yonettigimiz_dosya(Path::new("src/Kutu.json")));
        assert!(yonettigimiz_dosya(Path::new("src/Ana.server.lua")));
        assert!(yonettigimiz_dosya(Path::new("src/Hud.client.lua")));
        assert!(yonettigimiz_dosya(Path::new("src/Modul.lua")));
        assert!(yonettigimiz_dosya(Path::new("src/Modul.luau")));
        assert!(yonettigimiz_dosya(Path::new("src/Mesaj.txt")));
        assert!(yonettigimiz_dosya(Path::new("src/Ana.meta.json")));
    }

    #[test]
    fn ignore_desenleri_esler() {
        let desenler = vec!["*.md".to_string(), "notlar/**".to_string()];
        assert!(yok_sayilir(Path::new("src/OKU.md"), "src", &desenler));
        assert!(yok_sayilir(Path::new("src/notlar/a/b.lua"), "src", &desenler));
        assert!(!yok_sayilir(Path::new("src/Kutu.json"), "src", &desenler));
    }

    #[test]
    fn bos_desen_listesi_hicbir_seyi_yok_saymaz() {
        assert!(!yok_sayilir(Path::new("src/OKU.md"), "src", &[]));
    }

    /// Windows ters egik cizgileri de eslesmeli.
    #[test]
    fn ters_egik_cizgi_normallestirilir() {
        let desenler = vec!["notlar/**".to_string()];
        let p = PathBuf::from("src").join("notlar").join("gizli.lua");
        assert!(yok_sayilir(&p, "src", &desenler));
    }

    /// Bozuk desen cokmeye yol acmamali.
    #[test]
    fn bozuk_desen_cokmez() {
        let desenler = vec!["[".to_string()];
        assert!(!yok_sayilir(Path::new("src/Kutu.json"), "src", &desenler));
    }
}

#[cfg(test)]
mod csv_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn ekle(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn localizationtable_csv_dosyasina_yazilir() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = ekle(&mut m, "LocalizationTable", "Ceviriler", Some(rs));

        let yol = data_file(&m, "src", &lt).unwrap();
        assert!(yol.to_string_lossy().ends_with("Ceviriler.csv"), "{:?}", yol);
    }

    #[test]
    fn csv_icerigi_baslik_satiri_ile_baslar() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = ekle(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
        m.get_mut_instance(&lt).unwrap().properties.insert(
            "Contents".into(),
            PropertyValue::String(
                r#"[{"Key":"selam","Source":"Hello","Context":"","Values":{"tr":"Merhaba"}}]"#
                    .into(),
            ),
        );

        let icerik = node_content(m.get_instance(&lt).unwrap());
        let satirlar: Vec<&str> = icerik.lines().collect();
        assert_eq!(satirlar[0], "Key,Source,Context,Example,tr");
        assert!(satirlar[1].contains("Merhaba"), "{}", icerik);
    }

    /// Contents .csv dosyasinda tutuldugu icin meta'da TEKRARLANMAZ.
    #[test]
    fn contents_meta_dosyasinda_tekrarlanmaz() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = ekle(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
        {
            let n = m.get_mut_instance(&lt).unwrap();
            n.properties
                .insert("Contents".into(), PropertyValue::String("[]".into()));
        }
        // Yalnizca Contents varsa meta dosyasina gerek yok
        assert!(meta_file(&m, "src", &lt).is_none());

        m.get_mut_instance(&lt)
            .unwrap()
            .attributes
            .insert("Surum".into(), PropertyValue::Number(1.0));
        let icerik = meta_content(m.get_instance(&lt).unwrap());
        assert!(!icerik.contains("Contents"), "Contents meta'da olmamali: {}", icerik);
    }

    /// Bozuk Contents cokmeye yol acmamali, bos dosya yazilmali.
    #[test]
    fn bozuk_contents_cokmez() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = ekle(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
        m.get_mut_instance(&lt)
            .unwrap()
            .properties
            .insert("Contents".into(), PropertyValue::String("{bozuk".into()));

        let icerik = node_content(m.get_instance(&lt).unwrap());
        assert!(icerik.is_empty());
    }

    #[test]
    fn csv_yonetilen_uzanti() {
        assert!(yonettigimiz_dosya(Path::new("src/Ceviriler.csv")));
    }
}

#[cfg(test)]
mod cop_kutusu_testleri {
    use super::*;
    use std::fs;

    /// Her test kendi klasöründe çalışsın: çöp kutusu süreç genelinde tek
    /// tur adı kullanıyor, aynı dizini paylaşan testler birbirini bozardı.
    fn gecici_kok(ad: &str) -> PathBuf {
        let kok = std::env::temp_dir().join(format!("syncix-cop-{}", ad));
        let _ = fs::remove_dir_all(&kok);
        fs::create_dir_all(kok.join("src")).unwrap();
        kok
    }

    /// Asıl mesele: uzlaştırıcı bir dosyayı sildiğinde içeriği yok olmamalı.
    #[test]
    fn silinen_dosya_cop_kutusunda_durur() {
        let kok = gecici_kok("temel");
        let sync = kok.join("src");
        let dosya = sync.join("Onemli.server.lua");
        fs::write(&dosya, "print('kaybolmamali')").unwrap();

        cope_tasi(&dosya, sync.to_str().unwrap()).unwrap();

        assert!(!dosya.exists(), "dosya sync klasöründen kaldırılmalı");
        let turlar = cop_turlari(sync.to_str().unwrap());
        assert_eq!(turlar.len(), 1);
        assert_eq!(turlar[0].1, 1, "çöp kutusunda tam olarak bir dosya olmalı");
    }

    #[test]
    fn geri_alma_icerigi_aynen_dondurur() {
        let kok = gecici_kok("geri");
        let sync = kok.join("src");
        let s = sync.to_str().unwrap();
        let dosya = sync.join("Alt").join("Kod.server.lua");
        fs::create_dir_all(dosya.parent().unwrap()).unwrap();
        fs::write(&dosya, "-- orijinal icerik").unwrap();

        cope_tasi(&dosya, s).unwrap();
        let tur = cop_turlari(s)[0].0.clone();
        let (geri, atlanan) = coptan_geri_al(s, &tur);

        assert_eq!((geri, atlanan), (1, 0));
        assert!(dosya.exists(), "dosya eski yerine, alt klasörüyle birlikte dönmeli");
        assert_eq!(fs::read_to_string(&dosya).unwrap(), "-- orijinal icerik");
    }

    /// Geri alma kendi başına bir veri kaybı olmamalı: aynı yolda dosya varsa
    /// üzerine yazılmaz, atlanır.
    #[test]
    fn geri_alma_mevcut_dosyanin_uzerine_yazmaz() {
        let kok = gecici_kok("ezme");
        let sync = kok.join("src");
        let s = sync.to_str().unwrap();
        let dosya = sync.join("Kod.server.lua");

        fs::write(&dosya, "eski").unwrap();
        cope_tasi(&dosya, s).unwrap();
        fs::write(&dosya, "yeni ve degerli").unwrap();

        let tur = cop_turlari(s)[0].0.clone();
        let (geri, atlanan) = coptan_geri_al(s, &tur);

        assert_eq!((geri, atlanan), (0, 1));
        assert_eq!(fs::read_to_string(&dosya).unwrap(), "yeni ve degerli");
    }

    /// Çöp kutusu sync klasörünün DIŞINDA olmalı; içeride olsaydı izleyici onu
    /// yeni içerik sanar, uzlaştırıcı da bir sonraki turda tekrar silerdi.
    #[test]
    fn cop_kutusu_sync_klasorunun_disinda() {
        let kok = gecici_kok("konum");
        let sync = kok.join("src");
        let s = sync.to_str().unwrap();
        let dosya = sync.join("Kod.server.lua");
        fs::write(&dosya, "x").unwrap();
        cope_tasi(&dosya, s).unwrap();

        let mut kalanlar = Vec::new();
        collect_files(&sync, &mut kalanlar);
        assert!(kalanlar.is_empty(), "sync klasöründe hiçbir kalıntı olmamalı");
        assert!(kok.join(".syncix").join("trash").exists());
    }
}

#[cfg(test)]
mod yol_eslestirme_testleri {
    use super::*;
    use crate::model::{DataModel, InstanceNode};

    /// İzleyici mutlak yol bildiriyor, data_file göreli yol üretiyor.
    /// Düz eşitlik kullanıldığında disk silmesi hiç eşleşmiyordu.
    #[test]
    fn mutlak_yol_goreli_beklentiyle_eslesir() {
        let mut dm = DataModel::new();
        let mut servis = InstanceNode::new("ServerScriptService", "ServerScriptService");
        let servis_id = servis.syncix_id;
        servis.parent = None;

        let mut betik = InstanceNode::new("Script", "DiskSilTest");
        let betik_id = betik.syncix_id;
        betik.parent = Some(servis_id);
        servis.children.push(betik_id);

        dm.upsert_instance(servis).unwrap();
        dm.upsert_instance(betik).unwrap();

        let mutlak = Path::new(r"C:\proje\src_workspace\ServerScriptService\DiskSilTest.server.lua");
        assert_eq!(
            yol_icin_uuid(&dm, "src_workspace", mutlak),
            Some(betik_id)
        );
    }

    /// Meta dosyasının silinmesi instance'ın silinmesi değildir.
    #[test]
    fn meta_dosyasi_silme_sayilmaz() {
        let mut dm = DataModel::new();
        let mut betik = InstanceNode::new("Script", "Kod");
        betik.parent = None;
        dm.upsert_instance(betik).unwrap();

        let meta = Path::new(r"C:\proje\src_workspace\Kod.meta.json");
        assert_eq!(yol_icin_uuid(&dm, "src_workspace", meta), None);
    }
}

#[cfg(test)]
mod sonek_testleri {
    use super::*;

    /// Sync klasörü çoğu zaman "./src_workspace" olarak geliyor. Baştaki "."
    /// ayrı bir bileşen sayıldığı için düz ends_with eşleşmiyor ve disk
    /// silmeleri sessizce düşüyordu.
    #[test]
    fn nokta_egik_onek_eslesmeyi_bozmaz() {
        assert!(sonek_eslesir(
            Path::new(r"C:\proje\src_workspace\SSS\Kod.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }

    #[test]
    fn farkli_dosya_eslesmez() {
        assert!(!sonek_eslesir(
            Path::new(r"C:\proje\src_workspace\SSS\Baska.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }

    /// Yalnızca dosya adının tutması yetmemeli; klasör yolu da eşleşmeli.
    #[test]
    fn ayni_isim_farkli_klasor_eslesmez() {
        assert!(!sonek_eslesir(
            Path::new(r"C:\proje\src_workspace\Baska\Kod.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }
}
