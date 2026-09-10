use crate::model::{DataModel, InstanceNode};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// KENDI YAZIMLARIMIZIN KAYDI
//
// Sorun: disk yazicisi bir dosyayi yazdiginda file_path izleyici bunu bir KULLANICI
// degisikligi sanip modele restored_count uyguluyordu. Olay kuyrugu gecikmeli oldugu icin
// izleyici bazen dosyanin ESKI halini okuyor ve modeli geriye sariyordu.
//
// Gozlemlenen outcome: diskten eklenen bir attribute bazen kaliyor bazen kayboluyordu
// (DiskTenGelen kayboldu, OyunSurumu kaldi) — davranis yaris kosuluna bagliydi.
//
// Cozum: yazdigimiz her dosyanin icerigini not ediyoruz. Izleyici bir dosyayi
// okudugunda file_content last_item yazdigimizla ayniysa ogrenecek fresh bir sey yoktur, atlanir.
// Zaman penceresi kullanilmiyor; karsilastirma file_content uzerinden yapildigi icin
// gecikmeli olaylar da dogru elenir.
// ---------------------------------------------------------------------------

fn write_log() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static WRITE_HASHES: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_hash(file_content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    file_content.hash(&mut h);
    h.finish()
}

fn record_write(path: &Path, file_content: &str) {
    if let Ok(mut record) = write_log().lock() {
        record.insert(path.to_path_buf(), content_hash(file_content));
    }
}

fn forget_write(path: &Path) {
    if let Ok(mut record) = write_log().lock() {
        record.remove(path);
    }
}

/// Bu file_content bizim en last_item yazdigimiz mi? Oyleyse izleyici bunu islememeli.
pub fn is_own_write(path: &Path, file_content: &str) -> bool {
    write_log()
        .lock()
        .map(|k| k.get(path) == Some(&content_hash(file_content)))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// KENDI SILMELERIMIZIN KAYDI
//
// Yazma kaydinin deletion tarafindaki esi. Uzlastirici agaci yeniden yazarken
// file_path siliyor; izleyici bunlari kullanici silmesi sanarsa exists_flag olan objeleri
// yok eder. Bu yuzden file_path deletion olaylari tamamen yok sayiliyordu — ama o
// zaman da editorde bir dosyayi silmek hicbir sey yapmiyor, file_path birkac yuz
// milisaniye sonra restored_count beliriyordu.
//
// Cozum yazma tarafiyla is_same: sildigimiz yollari not ediyoruz. Izleyiciye
// received deletion olayi bu listede varsa bizimdir, atlanir; yoksa kullanici
// silmistir ve instance gercekten yok edilir.
// ---------------------------------------------------------------------------

fn delete_log() -> &'static Mutex<std::collections::HashSet<PathBuf>> {
    static WRITE_HASHES: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    WRITE_HASHES.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn record_delete(path: &Path) {
    if let Ok(mut k) = delete_log().lock() {
        k.insert(path.to_path_buf());
    }
}

/// Bu deletion bizim mi? Kayit TEK KULLANIMLIK: sorulan fs_path listeden dusurulur.
/// Boylece is_same fs_path daha sonra kullanici tarafindan silinirse gercek deletion
/// olarak islenir.
pub fn is_own_delete(path: &Path) -> bool {
    delete_log()
        .lock()
        .map(|mut k| k.remove(path))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// ÇÖP KUTUSU
//
// Uzlaştırıcı, Studio'nun ağacında karşılığı olmayan dosyaları siler. Bu doğru
// davranış — ama single yönlü. Core kapalıyken diskte yapılan bir düzenlemeyi
// kimse görmemiş olur; core açılıp Studio'nun ağacını yazdığında o file_path
// "fazlalık" sayılıp yok olur ve restored_count dönüşü yoktur.
//
// Bu yüzden yönettiğimiz hiçbir file_path doğrudan silinmez, çöp kutusuna taşınır.
// Kutu sync klasörünün DIŞINDA: içeride olsaydı izleyici onu fresh içerik sanar,
// uzlaştırıcı da bir sonraki turda silerdi.
// ---------------------------------------------------------------------------

/// <sync_dir>/../.syncix/trash
fn cop_kokü(sync_dir: &str) -> PathBuf {
    let s = Path::new(sync_dir);
    let upper = s.parent().unwrap_or(s);
    upper.join(".syncix").join("trash")
}

/// En fresh TRASH_KEEP_DEFAULT kadar klasör tutulur; eskiler tamamen silinir.
const TRASH_KEEP_DEFAULT: usize = 10;

/// Aynı core oturumu içindeki tüm silmeler single klasörde toplansın diye
/// run_name adı bir kez üretilip saklanır.
fn run_label() -> String {
    static TRASH_RUN: OnceLock<String> = OnceLock::new();
    TRASH_RUN.get_or_init(|| chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string())
        .clone()
}

/// Dosyayı silmek yerine çöp kutusuna taşır. Sync klasörüne göre göreli fs_path
/// korunur, böylece restored_count koymak düz bir kopyalama işi olur.
fn move_to_trash(path: &Path, sync_dir: &str) -> std::io::Result<()> {
    // Cop kutusu kapaliysa file_path dogrudan silinir. Bunu isteyen biri restored_count
    // donusu olmadigini bilerek istiyor; ayar aciklamasinda da yaziyor.
    if !trash_config().0 {
        return fs::remove_file(path);
    }
    let rel_path = path.strip_prefix(sync_dir).unwrap_or(path);
    let dest = cop_kokü(sync_dir).join(run_label()).join(rel_path);
    if let Some(upper) = dest.parent() {
        fs::create_dir_all(upper)?;
    }
    // rename aynı disk bölümünde ucuz; farklı bölümdeyse kopyala-sil'e düşer.
    if fs::rename(path, &dest).is_err() {
        fs::copy(path, &dest)?;
        fs::remove_file(path)?;
    }
    prune_trash(sync_dir);
    Ok(())
}

/// Kutu sınırsız büyümemeli: en fresh TRASH_KEEP_DEFAULT klasör kalır.
fn prune_trash(sync_dir: &str) {
    let root_dir = cop_kokü(sync_dir);
    let Ok(input_list) = fs::read_dir(&root_dir) else {
        return;
    };
    let mut runs: Vec<PathBuf> = input_list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    let to_keep = trash_config().1;
    if runs.len() <= to_keep {
        return;
    }
    // Klasör adı zaman damgası olduğu için isim sıralaması zaman sıralamasıdır.
    runs.sort();
    let to_delete = runs.len() - to_keep;
    for previous_text in runs.into_iter().take(to_delete) {
        let _ = fs::remove_dir_all(previous_text);
    }
}

/// Çöp kutusundaki turları yeniden eskiye doğru listeler: (run_name adı, file_path sayısı).
pub fn trash_runs(sync_dir: &str) -> Vec<(String, usize)> {
    let root_dir = cop_kokü(sync_dir);
    let Ok(input_list) = fs::read_dir(&root_dir) else {
        return Vec::new();
    };
    let mut runs: Vec<PathBuf> = input_list
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    runs.sort();
    runs.reverse();
    runs
        .into_iter()
        .map(|t| {
            let mut file_list = Vec::new();
            collect_files(&t, &mut file_list);
            (
                t.file_name().unwrap_or_default().to_string_lossy().to_string(),
                file_list.len(),
            )
        })
        .collect()
}

/// Bir turdaki dosyaları sync klasörüne restored_count koyar. Var olan dosyanın üzerine
/// yazılmaz — restored_count koyma own başına bir veri kaybı olmamalı.
/// Döner: (restored_count konan, üzerine yazılmadığı için skipped).
pub fn restore_from_trash(sync_dir: &str, run_name: &str) -> (usize, usize) {
    let origin = cop_kokü(sync_dir).join(run_name);
    let mut file_list = Vec::new();
    collect_files(&origin, &mut file_list);

    let (mut restored_count, mut skipped) = (0usize, 0usize);
    for d in file_list {
        let Ok(rel_path) = d.strip_prefix(&origin) else {
            continue;
        };
        let dest = Path::new(sync_dir).join(rel_path);
        if dest.exists() {
            skipped += 1;
            continue;
        }
        if let Some(upper) = dest.parent() {
            let _ = fs::create_dir_all(upper);
        }
        if fs::copy(&d, &dest).is_ok() {
            restored_count += 1;
        }
    }
    (restored_count, skipped)
}

/// Diskteki bir yolun hangi instance'a ait oldugunu bulur.
///
/// Ters yonde (uuid -> fs_path) `data_file` exists_flag; deletion olayini isleyebilmek icin
/// yolun kendisinden yola cikmak gerekiyor, cunku file_path artik okunamiyor.
/// Yalnizca VERI dosyasi eslesirse uuid doner: meta dosyasinin silinmesi
/// instance'in silinmesi degildir.
/// Bir fs_path, expected_value yolun bilesen bazli soneki mi?
///
/// Duz esitlik ise yaramiyor: izleyici MUTLAK fs_path bildiriyor, data_file ise
/// sync_dir'den baslayan GORELI fs_path uretiyor. Path::ends_with single basina da
/// yetmiyor, cunku sync_dir cogu zaman "./src_workspace" seklinde geliyor ve
/// bastaki "." ayri bir bilesen sayilip eslesmeyi bozuyor. Bu yuzden "." ve ""
/// bilesenleri her iki tarafta da atiliyor.
///
/// Sonek icinde sync_dir de bulundugu icin yanlis eslesme pratikte mumkun degil.
fn suffix_matches(fs_path: &Path, expected_value: &Path) -> bool {
    use std::path::Component;
    let clean_up = |p: &Path| -> Vec<std::ffi::OsString> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_os_string())
            .collect()
    };
    let a = clean_up(fs_path);
    let b = clean_up(expected_value);
    if b.is_empty() || b.len() > a.len() {
        return false;
    }
    a[a.len() - b.len()..] == b[..]
}

pub fn uuid_for_path(dm: &DataModel, sync_dir: &str, path: &Path) -> Option<Uuid> {
    for uuid in dm.get_all_instances().keys() {
        let Some(expected_value) = data_file(dm, sync_dir, uuid) else {
            continue;
        };
        if suffix_matches(path, &expected_value) {
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
        // StringValue'nun single anlamli alani Value'dur; onu JSON icine gomup
        // kacis karakterleriyle ugrastirmak yerine duz text_value dosyasi olarak
        // yaziyoruz. Boylece text_value icerigi editorde dogrudan duzenlenebiliyor.
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

/// Bu class_str diske HAM ICERIK olarak mi yaziliyor? (own .json'u yerine)
/// Boyle siniflarin property/attribute'lari .meta.json dosyasinda tutulur.
fn with_raw_content(node: &InstanceNode) -> bool {
    script_ext(&node.class_name).is_some()
}

/// StringValue gibi siniflarda diske yazilacak raw text_value.
fn raw_str(node: &InstanceNode) -> String {
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
        Some(rest) => format!("{:?}", rest),
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

/// Bir objenin diskteki file_path yolu. sourcemap üretimi de bunu kullanır ki
/// fs_path hesabı single yerde kalsın.
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
/// Neden gerekli: script'ler diske raw origin script_code olarak yaziliyor (.lua), dolayisiyla
/// property'leri ve attribute'lari icin yer yok. Bu yuzden Disabled, RunContext gibi
/// alanlar ve TUM attribute'lar disk tarafinda kayboluyordu. Rojo'daki .meta.json
/// fikrinin sade hali: `$`'li sihirli key_names yok, yalnizca iki alan exists_flag.
///
/// Yalnizca yazacak bir sey varsa uretilir; bos meta dosyalariyla folder_path kirletilmez.
fn meta_file(dm: &DataModel, sync_dir: &str, uuid: &Uuid) -> Option<PathBuf> {
    if !META_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let node = dm.get_instance(uuid)?;
    if !with_raw_content(node) {
        return None; // rest objelerin own .json dosyasi zaten her seyi tutuyor
    }
    // StringValue'da Value zaten .txt dosyasinda; meta'da tekrarlanmasi anlamsiz.
    let non_value_property = node
        .properties
        .keys()
        .any(|k| is_script(node) || (k != "Value" && k != "Contents"));
    // Etiketler de yazilacak bir sey: yalnizca etiketi olan bir script'in meta
    // dosyasi uretilmezse tag_text diske hic ulasmaz.
    if !non_value_property && node.attributes.is_empty() && node.tags.is_empty() {
        return None;
    }
    let script_path = data_file(dm, sync_dir, uuid)?;
    let dir = script_path.parent()?;
    let item_name = if node.children.is_empty() {
        format!("{}.meta.json", seg(dm, node))
    } else {
        "init.meta.json".to_string()
    };
    Some(dir.join(item_name))
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
        // Ham file_content dosyasinda tutulan alanlar meta'da TEKRARLANMAZ;
        // is_same veriyi iki yerde tutmak ikisinin ayrisma riskini dogurur.
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
    if with_raw_content(node) {
        raw_str(node)
    } else {
        serde_json::to_string_pretty(node).unwrap_or_default()
    }
}

/// Bu file_path Syncix'in ÜRETEBİLECEĞİ bir file_path mı?
///
/// Reconcile yazıcısı expected_value listede olmayan dosyaları siliyordu; bu yüzden
/// syncing klasörüne konan herhangi bir file_path (README, .gitkeep, kişisel not)
/// sessizce yok oluyordu. Ölçüldü ve doğrulandı: NOTLAR.md dosyası first_item yazımda
/// silindi.
///
/// Rojo'da bu sorun yok çünkü Rojo diske hiç yazmıyor. Bizde single yönlü bir
/// "ignore listesi" yetmez; ASIL kural şudur: yalnızca own üretebileceğimiz
/// biçimdeki dosyalara dokunuruz. Tanımadığımız hiçbir file_path silinmez.
fn is_managed_file(path: &Path) -> bool {
    let Some(item_name) = path.file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    item_name.ends_with(".meta.json")
        || item_name.ends_with(".server.lua")
        || item_name.ends_with(".client.lua")
        || item_name.ends_with(".lua")
        || item_name.ends_with(".luau")
        || item_name.ends_with(".txt")
        || item_name.ends_with(".csv")
        || item_name.ends_with(".json")
}

/// Yol, kullanıcının belirlediği ignore desenlerinden birine uyuyor mu?
/// Desenler syncing klasörüne göre değerlendirilir (ör. "notlar/**", "*.md").
pub fn is_ignored(path: &Path, sync_dir: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let rel = path.strip_prefix(sync_dir).unwrap_or(path);
    let text_value = rel.to_string_lossy().replace('\\', "/");

    patterns.iter().any(|d| {
        glob::Pattern::new(d)
            .map(|p| p.matches(&text_value))
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
/// hem de her yazımda file_path izleyiciyi gereksiz tetikliyordu. Artık yalnızca fark
/// uygulanır: içeriği aynı olan dosyalara hiç dokunulmaz.
/// Cop kutusu davranisi yapilandirmadan geliyor. Global tutulmasinin sebebi
/// write_full_tree'nin cagri zincirinin uzun olmasi ve single bir ayar icin her
/// halkaya parametre eklemenin kodu okunmaz hale getirmesi.
static TRASH_CONFIG: OnceLock<Mutex<(bool, usize)>> = OnceLock::new();

/// .meta.json dosyalari yazilsin mi. Kapaliysa script'lerin property ve
/// attribute'lari diske hic yazilmaz; yalnizca origin script_code dosyasi kalir.
static META_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn configure_meta(is_enabled: bool) {
    META_ENABLED.store(is_enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn configure_trash(is_enabled: bool, run_count: usize) {
    let h = TRASH_CONFIG.get_or_init(|| Mutex::new((true, TRASH_KEEP_DEFAULT)));
    if let Ok(mut a) = h.lock() {
        *a = (is_enabled, run_count.max(1));
    }
}

fn trash_config() -> (bool, usize) {
    TRASH_CONFIG
        .get()
        .and_then(|h| h.lock().ok().map(|a| *a))
        .unwrap_or((true, TRASH_KEEP_DEFAULT))
}

/// `silme_izni`: modelin Studio'nun gercek agacini yansittigi kesinlesmeden
/// (bu oturumda FULL_SYNC tamamlanmadan) diskten HICBIR file_path kaldirilmaz.
/// Aksi halde bos model "diskteki her sey fazla" demek olur: Studio kapaliyken
/// editor acildiginda syncing klasorundeki butun file_list cope tasiniyordu.
pub fn write_full_tree(dm: &DataModel, sync_dir: &str, ignore: &[String], allow_removal: bool) {
    let root = Path::new(sync_dir);
    let _ = fs::create_dir_all(root);

    // 1) Olması gereken file_list
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

    // 2) Diskte olan file_list
    let mut existing = Vec::new();
    collect_files(root, &mut existing);

    // 3) Fazlalıkları sil — yalnızca model otoriteyse (bkz. allow_removal).
    let mut removed = 0usize;
    let mut kept = 0usize;
    for path in &existing {
        if expected.contains_key(path) {
            continue;
        }
        // Tanımadığımız dosyalara ASLA dokunma (README, .gitkeep, kişisel notlar).
        if !is_managed_file(path) {
            continue;
        }
        // Kullanıcının yok saydırdığı yollar da korunur.
        if is_ignored(path, sync_dir, ignore) {
            continue;
        }
        if !allow_removal {
            kept += 1;
            continue;
        }
        if move_to_trash(path, sync_dir).is_ok() {
            forget_write(path);
            record_delete(path);
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
                record_write(path, content);
                written += 1;
            }
            Err(e) => tracing::error!("layout: could not write file ({:?}): {}", path, e),
        }
    }

    // 5) Boşalan klasörleri clean_up
    remove_empty_dirs(root, root);

    if kept > 0 {
        tracing::debug!(
            "layout: {} file(s) not in the model were kept — Studio has not synced yet",
            kept
        );
    }
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

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// Ozelligi olmayan script icin meta dosyasi URETILMEZ.
    /// Aksi halde her script'in yaninda bos bir .meta.json birikirdi.
    #[test]
    fn script_without_properties_has_no_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        assert!(meta_file(&m, "src", &s).is_none());
    }

    /// Property ya da attribute varsa meta dosyasi script'in YANINDA olusur.
    #[test]
    fn meta_file_created_next_to_script_with_properties() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
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
    fn attribute_alone_produces_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .attributes
            .insert("Surum".into(), PropertyValue::Number(3.0));

        assert!(meta_file(&m, "src", &s).is_some());
    }

    /// Cocugu olan script konteyner klasore doner; meta name_of da init.meta.json olur.
    #[test]
    fn container_script_uses_init_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        add_instance(&mut m, "ModuleScript", "Alt", Some(s));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let meta = meta_file(&m, "src", &s).unwrap();
        assert!(meta.to_string_lossy().ends_with("init.meta.json"), "{:?}", meta);
    }

    /// Script olmayan objeler icin meta URETILMEZ: onlarin own .json dosyasi
    /// zaten property ve attribute'lari tutuyor, ikinci bir file_path kafa karistirir.
    #[test]
    fn non_script_object_has_no_meta() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Kutu", Some(ws));
        m.get_mut_instance(&p)
            .unwrap()
            .properties
            .insert("Anchored".into(), PropertyValue::Boolean(true));

        assert!(meta_file(&m, "src", &p).is_none());
    }

    /// Meta icerigi gidis-donus yapabilmeli.
    #[test]
    fn meta_content_round_trip() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        {
            let n = m.get_mut_instance(&s).unwrap();
            n.properties.insert("Disabled".into(), PropertyValue::Boolean(true));
            n.properties.insert(
                "RunContext".into(),
                PropertyValue::String("Enum.RunContext.Server".into()),
            );
            n.attributes.insert("Surum".into(), PropertyValue::Number(2.0));
        }

        let text_value = meta_content(m.get_instance(&s).unwrap());
        let restored_count: ScriptMeta = serde_json::from_str(&text_value).expect("cozulemedi");
        assert_eq!(restored_count.properties.len(), 2);
        assert_eq!(
            restored_count.properties.get("RunContext"),
            Some(&PropertyValue::String("Enum.RunContext.Server".into()))
        );
        assert_eq!(restored_count.attributes.get("Surum"), Some(&PropertyValue::Number(2.0)));
    }
}

#[cfg(test)]
mod txt_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    /// StringValue diske .txt olarak yazilir, .json olarak degil.
    #[test]
    fn stringvalue_is_written_as_txt() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Mesaj", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("merhaba".into()));

        let fs_path = data_file(&m, "src", &sv).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Mesaj.txt"), "{:?}", fs_path);
    }

    /// Dosyanin icerigi dogrudan Value'dur; JSON sarmalayici yok.
    #[test]
    fn txt_content_is_the_value() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Mesaj", Some(rs));
        m.get_mut_instance(&sv)
            .unwrap()
            .properties
            .insert("Value".into(), PropertyValue::String("satir1\nsatir2".into()));

        let file_content = node_content(m.get_instance(&sv).unwrap());
        assert_eq!(file_content, "satir1\nsatir2");
    }

    /// Value .txt dosyasinda oldugu icin meta dosyasinda TEKRARLANMAZ;
    /// iki yerde tutmak ikisinin ayrisma riskini dogurur.
    #[test]
    fn value_not_repeated_in_meta_file() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let sv = add_instance(&mut m, "StringValue", "Mesaj", Some(rs));
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

        let file_content = meta_content(m.get_instance(&sv).unwrap());
        assert!(!file_content.contains("Value"), "Value meta'da olmamali: {}", file_content);
        assert!(file_content.contains("Dil"));
    }

    /// Script'lerde Source ayri bir alan oldugu icin property'ler meta'da kalir.
    #[test]
    fn script_properties_stay_in_meta() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s)
            .unwrap()
            .properties
            .insert("Disabled".into(), PropertyValue::Boolean(true));

        let file_content = meta_content(m.get_instance(&s).unwrap());
        assert!(file_content.contains("Disabled"));
    }

    /// StringValue disindaki ValueBase siniflari hala .json kullanir:
    /// sayisal bir degeri duz metne cevirmek type_name bilgisini kaybettirirdi.
    #[test]
    fn intvalue_stays_json() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let iv = add_instance(&mut m, "IntValue", "Sayac", Some(rs));

        let fs_path = data_file(&m, "src", &iv).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Sayac.json"), "{:?}", fs_path);
    }
}

#[cfg(test)]
mod protection_tests {
    use super::*;

    /// Tanimadigimiz dosyalara ASLA dokunulmaz.
    /// Bu, syncing klasorune konan README/not dosyalarinin silinmesi bugunun testi.
    #[test]
    fn foreign_files_are_not_managed() {
        assert!(!is_managed_file(Path::new("src/NOTLAR.md")));
        assert!(!is_managed_file(Path::new("src/.gitkeep")));
        assert!(!is_managed_file(Path::new("src/resim.png")));
        assert!(!is_managed_file(Path::new("src/rapor.pdf")));
    }

    /// Kendi uretebilecegimiz bicimler yonetilir.
    #[test]
    fn our_formats_are_managed() {
        assert!(is_managed_file(Path::new("src/Kutu.json")));
        assert!(is_managed_file(Path::new("src/Ana.server.lua")));
        assert!(is_managed_file(Path::new("src/Hud.client.lua")));
        assert!(is_managed_file(Path::new("src/Modul.lua")));
        assert!(is_managed_file(Path::new("src/Modul.luau")));
        assert!(is_managed_file(Path::new("src/Mesaj.txt")));
        assert!(is_managed_file(Path::new("src/Ana.meta.json")));
    }

    #[test]
    fn ignore_patterns_match() {
        let patterns = vec!["*.md".to_string(), "notlar/**".to_string()];
        assert!(is_ignored(Path::new("src/OKU.md"), "src", &patterns));
        assert!(is_ignored(Path::new("src/notlar/a/b.lua"), "src", &patterns));
        assert!(!is_ignored(Path::new("src/Kutu.json"), "src", &patterns));
    }

    #[test]
    fn empty_pattern_list_ignores_nothing() {
        assert!(!is_ignored(Path::new("src/OKU.md"), "src", &[]));
    }

    /// Windows ters egik cizgileri de eslesmeli.
    #[test]
    fn backslashes_are_normalised() {
        let patterns = vec!["notlar/**".to_string()];
        let p = PathBuf::from("src").join("notlar").join("gizli.lua");
        assert!(is_ignored(&p, "src", &patterns));
    }

    /// Bozuk desen cokmeye fs_path acmamali.
    #[test]
    fn broken_pattern_does_not_crash() {
        let patterns = vec!["[".to_string()];
        assert!(!is_ignored(Path::new("src/Kutu.json"), "src", &patterns));
    }
}

#[cfg(test)]
mod csv_tests {
    use super::*;
    use crate::model::PropertyValue;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn localizationtable_csv_dosyasina_yazilir() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Ceviriler", Some(rs));

        let fs_path = data_file(&m, "src", &lt).unwrap();
        assert!(fs_path.to_string_lossy().ends_with("Ceviriler.csv"), "{:?}", fs_path);
    }

    #[test]
    fn csv_content_starts_with_header_row() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
        m.get_mut_instance(&lt).unwrap().properties.insert(
            "Contents".into(),
            PropertyValue::String(
                r#"[{"Key":"selam","Source":"Hello","Context":"","Values":{"tr":"Merhaba"}}]"#
                    .into(),
            ),
        );

        let file_content = node_content(m.get_instance(&lt).unwrap());
        let line_list: Vec<&str> = file_content.lines().collect();
        assert_eq!(line_list[0], "Key,Source,Context,Example,tr");
        assert!(line_list[1].contains("Merhaba"), "{}", file_content);
    }

    /// Contents .csv dosyasinda tutuldugu icin meta'da TEKRARLANMAZ.
    #[test]
    fn contents_not_repeated_in_meta_file() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
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
        let file_content = meta_content(m.get_instance(&lt).unwrap());
        assert!(!file_content.contains("Contents"), "Contents meta'da olmamali: {}", file_content);
    }

    /// Bozuk Contents cokmeye fs_path acmamali, bos file_path yazilmali.
    #[test]
    fn broken_contents_does_not_crash() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        let lt = add_instance(&mut m, "LocalizationTable", "Ceviriler", Some(rs));
        m.get_mut_instance(&lt)
            .unwrap()
            .properties
            .insert("Contents".into(), PropertyValue::String("{bozuk".into()));

        let file_content = node_content(m.get_instance(&lt).unwrap());
        assert!(file_content.is_empty());
    }

    #[test]
    fn csv_is_managed_extension() {
        assert!(is_managed_file(Path::new("src/Ceviriler.csv")));
    }
}

#[cfg(test)]
mod trash_tests {
    use super::*;
    use std::fs;

    /// Her test own klasöründe çalışsın: çöp kutusu süreç genelinde single
    /// run_name adı kullanıyor, aynı dizini paylaşan testler birbirini bozardı.
    fn scratch_root(item_name: &str) -> PathBuf {
        let root_dir = std::env::temp_dir().join(format!("syncix-cop-{}", item_name));
        let _ = fs::remove_dir_all(&root_dir);
        fs::create_dir_all(root_dir.join("src")).unwrap();
        root_dir
    }

    /// Gerçek bir olaydan: Studio bağlanmadan editör açıldı, model boştu ve
    /// uzlaştırıcı syncing klasöründeki 48 dosyanın hepsini çöpe taşıdı. Model
    /// otorite değilken diskten hiçbir şey kaldırılmamalı.
    #[test]
    fn empty_model_deletes_nothing_before_studio_syncs() {
        let root_dir = scratch_root("otorite-yok");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let game_file = sync.join("Workspace.json");
        let script_node = sync.join("ServerScriptService").join("Main.server.lua");
        fs::create_dir_all(script_node.parent().unwrap()).unwrap();
        fs::write(&game_file, "{}").unwrap();
        fs::write(&script_node, "print('oyun kodu')").unwrap();

        write_full_tree(&DataModel::new(), s, &[], false);

        assert!(game_file.exists(), "model otorite değilken dosya silinmemeli");
        assert!(script_node.exists(), "alt klasördeki betik de yerinde kalmalı");
        assert!(trash_runs(s).is_empty(), "çöp kutusuna hiçbir şey gitmemeli");
    }

    /// Otorite varsa previous_text davranış aynen sürer: modelde olmayan file_path çöpe gider.
    #[test]
    fn authoritative_model_trashes_extras() {
        let root_dir = scratch_root("otorite-var");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let extra = sync.join("Silinmis.server.lua");
        fs::write(&extra, "-- Studio'da artik yok").unwrap();

        write_full_tree(&DataModel::new(), s, &[], true);

        assert!(!extra.exists(), "otoriter modelde olmayan dosya kaldırılmalı");
        assert_eq!(trash_runs(s).len(), 1, "kaldırılan dosya çöp kutusunda olmalı");
    }

    /// Asıl mesele: uzlaştırıcı bir dosyayı sildiğinde içeriği yok olmamalı.
    #[test]
    fn deleted_file_stays_in_trash() {
        let root_dir = scratch_root("temel");
        let sync = root_dir.join("src");
        let file_path = sync.join("Onemli.server.lua");
        fs::write(&file_path, "print('kaybolmamali')").unwrap();

        move_to_trash(&file_path, sync.to_str().unwrap()).unwrap();

        assert!(!file_path.exists(), "dosya sync klasöründen kaldırılmalı");
        let runs = trash_runs(sync.to_str().unwrap());
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].1, 1, "çöp kutusunda tam olarak bir dosya olmalı");
    }

    #[test]
    fn restore_returns_content_unchanged() {
        let root_dir = scratch_root("geri");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Alt").join("Kod.server.lua");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "-- orijinal icerik").unwrap();

        move_to_trash(&file_path, s).unwrap();
        let run_name = trash_runs(s)[0].0.clone();
        let (restored_count, skipped) = restore_from_trash(s, &run_name);

        assert_eq!((restored_count, skipped), (1, 0));
        assert!(file_path.exists(), "dosya eski yerine, alt klasörüyle birlikte dönmeli");
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "-- orijinal icerik");
    }

    /// Geri alma own başına bir veri kaybı olmamalı: aynı yolda file_path varsa
    /// üzerine yazılmaz, atlanır.
    #[test]
    fn restore_does_not_overwrite_existing() {
        let root_dir = scratch_root("ezme");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Kod.server.lua");

        fs::write(&file_path, "eski").unwrap();
        move_to_trash(&file_path, s).unwrap();
        fs::write(&file_path, "yeni ve degerli").unwrap();

        let run_name = trash_runs(s)[0].0.clone();
        let (restored_count, skipped) = restore_from_trash(s, &run_name);

        assert_eq!((restored_count, skipped), (0, 1));
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "yeni ve degerli");
    }

    /// Çöp kutusu sync klasörünün DIŞINDA olmalı; içeride olsaydı izleyici onu
    /// fresh içerik sanar, uzlaştırıcı da bir sonraki turda again silerdi.
    #[test]
    fn trash_is_outside_sync_folder() {
        let root_dir = scratch_root("konum");
        let sync = root_dir.join("src");
        let s = sync.to_str().unwrap();
        let file_path = sync.join("Kod.server.lua");
        fs::write(&file_path, "x").unwrap();
        move_to_trash(&file_path, s).unwrap();

        let mut remaining_items = Vec::new();
        collect_files(&sync, &mut remaining_items);
        assert!(remaining_items.is_empty(), "sync klasöründe hiçbir kalıntı olmamalı");
        assert!(root_dir.join(".syncix").join("trash").exists());
    }
}

#[cfg(test)]
mod path_matching_tests {
    use super::*;
    use crate::model::{DataModel, InstanceNode};

    /// İzleyici absolute fs_path bildiriyor, data_file göreli fs_path üretiyor.
    /// Düz eşitlik kullanıldığında disk silmesi hiç eşleşmiyordu.
    #[test]
    fn absolute_path_matches_relative_expectation() {
        let mut dm = DataModel::new();
        let mut service_name = InstanceNode::new("ServerScriptService", "ServerScriptService");
        let service_id = service_name.syncix_id;
        service_name.parent = None;

        let mut script_node = InstanceNode::new("Script", "DiskSilTest");
        let script_id = script_node.syncix_id;
        script_node.parent = Some(service_id);
        service_name.children.push(script_id);

        dm.upsert_instance(service_name).unwrap();
        dm.upsert_instance(script_node).unwrap();

        let absolute = Path::new(r"C:\proje\src_workspace\ServerScriptService\DiskSilTest.server.lua");
        assert_eq!(
            uuid_for_path(&dm, "src_workspace", absolute),
            Some(script_id)
        );
    }

    /// Meta dosyasının silinmesi instance'ın silinmesi değildir.
    #[test]
    fn deleting_meta_file_is_not_a_delete() {
        let mut dm = DataModel::new();
        let mut script_node = InstanceNode::new("Script", "Kod");
        script_node.parent = None;
        dm.upsert_instance(script_node).unwrap();

        let meta = Path::new(r"C:\proje\src_workspace\Kod.meta.json");
        assert_eq!(uuid_for_path(&dm, "src_workspace", meta), None);
    }
}

#[cfg(test)]
mod suffix_tests {
    use super::*;

    /// Sync klasörü çoğu zaman "./src_workspace" olarak geliyor. Baştaki "."
    /// ayrı bir bileşen sayıldığı için düz ends_with eşleşmiyor ve disk
    /// silmeleri sessizce düşüyordu.
    #[test]
    fn dot_slash_prefix_does_not_break_matching() {
        assert!(suffix_matches(
            Path::new(r"C:\proje\src_workspace\SSS\Kod.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }

    #[test]
    fn different_file_does_not_match() {
        assert!(!suffix_matches(
            Path::new(r"C:\proje\src_workspace\SSS\Baska.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }

    /// Yalnızca file_path adının tutması yetmemeli; klasör yolu da eşleşmeli.
    #[test]
    fn same_name_other_folder_does_not_match() {
        assert!(!suffix_matches(
            Path::new(r"C:\proje\src_workspace\Baska\Kod.server.lua"),
            Path::new("./src_workspace/SSS/Kod.server.lua"),
        ));
    }
}
