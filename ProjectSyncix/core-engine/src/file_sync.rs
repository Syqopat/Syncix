use crate::model::InstanceNode;
use crate::transport::{EventType, Payload, StudioOutbox};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::serializers::part::PartSerializer;
use crate::serializers::Serializer;

/// Diskteki değişiklikleri izler ve Studio'ya iletir.
/// ÖNEMLİ: Studio'ya outgoing her raw_value `pv_to_wire` ile cevrilir.
/// `PropertyValue`'yu dogrudan JSON'a koymak serde'nin disa donuk etiketli
/// bicimini uretiyor ({"Number":0.5}); eklenti ise duz bicimi bekliyor ve
/// "unsupported table value for property" diye reddediyor. Bu report_error canli
/// kullanimda Part.Transparency uzerinde yakalandi.
///
/// ÖNEMLİ: Bu modül diske ASLA yazmaz. Diske yazmak yalnızca layout modülünün işidir;
/// aksi halde iki taraf farklı isim kuralları uygulayıp birbirinin dosyasını ezer.
pub async fn start_watcher(
    tx_to_studio: Arc<StudioOutbox>,
    path: &str,
    data_model: crate::model::SharedDataModel,
    tx_to_vscode: tokio::sync::broadcast::Sender<String>,
    disk_notify: Arc<tokio::sync::Notify>,
    ignore: Vec<String>,
    settings_data: crate::project::ProjectConfig,
) {
    // Disk -> Studio yonu kapaliysa izleyiciyi hic baslatmiyoruz.
    // Sadece gonderimi susturmak yetmezdi: diskteki degisiklik yine de modele
    // islenir ve bir sonraki yazimda Studio'ya sizardi.
    configure_delete_grace(settings_data.safety_settings.delete_grace_ms);

    if !settings_data.mode_value.accepts_from_disk() {
        info!(
            "Sync mode is '{}': the file watcher was not started, disk changes are ignored.",
            settings_data.mode_value.name_of()
        );
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).unwrap();

    watcher
        .watch(Path::new(path), RecursiveMode::Recursive)
        .unwrap();
    info!("File system watcher started: {}", path);

    let serializer = PartSerializer;
    let sync_dir_owner = path.to_string();

    tokio::task::spawn_blocking(move || {
        let _watcher = watcher;
        // Silme olaylari HEMEN uygulanmaz. Sebep: bir dosyayi baska klasore
        // tasimak isletim sisteminde "sil + olustur" olarak goruluyor. Hemen
        // silseydik tasima, instance'i yok edip yerine fresh kimlikli bir tane
        // koyardi; property'ler ve sub tree kaybolurdu.
        // Bunun yerine deletion SILME_BEKLEME_SURESI kadar bekletilir; is_same uuid
        // bu sure icinde yeniden ortaya cikarsa tasima oldugu anlasilir ve
        // deletion iptal edilir.
        let mut pending_deletes: std::collections::HashMap<Uuid, std::time::Instant> =
            std::collections::HashMap::new();

        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    handle_event(
                        event,
                        &tx_to_studio,
                        &serializer,
                        &data_model,
                        &tx_to_vscode,
                        &disk_notify,
                        &sync_dir_owner,
                        &ignore,
                        &mut pending_deletes,
                    );
                }
                Ok(Err(e)) => error!("Watch error: {:?}", e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            apply_ripe_deletes(
                &mut pending_deletes,
                &tx_to_studio,
                &data_model,
                &tx_to_vscode,
                &disk_notify,
            );
        }
        error!("File watcher loop ended unexpectedly.");
    });
}

/// Bekleme suresini dolduran silmeleri gercekten uygular.
///
/// Bekleme, tasima islemini deletion sanmamak icin: isletim sistemi file_path
/// tasimayi "sil + olustur" olarak bildiriyor. Bu sure icinde file_path restored_count
/// gelirse deletion iptal edilmis oluyor.
static DELETE_GRACE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(800);

pub fn configure_delete_grace(ms: u64) {
    DELETE_GRACE_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
}

fn delete_grace_period() -> std::time::Duration {
    std::time::Duration::from_millis(DELETE_GRACE_MS.load(std::sync::atomic::Ordering::Relaxed))
}

fn apply_ripe_deletes(
    pending_item: &mut std::collections::HashMap<Uuid, std::time::Instant>,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    disk_notify: &Arc<tokio::sync::Notify>,
) {
    if pending_item.is_empty() {
        return;
    }
    let current_time = std::time::Instant::now();
    let ready_deletes: Vec<Uuid> = pending_item
        .iter()
        .filter(|(_, t)| current_time.duration_since(**t) >= delete_grace_period())
        .map(|(u, _)| *u)
        .collect();

    for uuid in ready_deletes {
        pending_item.remove(&uuid);

        let removed_node = {
            let mut dm = data_model.blocking_write();
            dm.remove_instance(&uuid)
        };
        let Some(instance) = removed_node else {
            continue;
        };

        info!(
            "The file for '{}' was deleted from disk; the instance is being removed.",
            instance.name
        );

        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "DESTROY",
                    "data": { "syncix_id": uuid }
                }]
            }),
        });

        let msg = serde_json::json!({
            "event_type": "INSTANCE_REMOVED",
            "data": {
                "id": uuid,
                "parentId": instance.parent.map(|u| u.to_string())
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());

        // Alt tree da modelden dustu; disk yazicisi agaci yeniden yazsin.
        disk_notify.notify_one();
    }
}

/// Bir instance'ın güncel halini VS Code Explorer'a bildirir.
fn notify_vscode_updated(
    dm: &crate::model::DataModel,
    uuid: &Uuid,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
) {
    if let Some(inst) = dm.get_instance(uuid) {
        let msg = serde_json::json!({
            "event_type": event_type,
            "data": {
                "id": inst.syncix_id,
                "name": inst.name,
                "className": inst.class_name,
                "parentId": inst.parent.map(|u| u.to_string()),
                "childrenIds": inst.children,
                "isExpanded": false
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());
    }
}

fn parse_script_filename(path: &Path) -> Option<(String, Option<String>, String)> {
    let fname = path.file_name()?.to_str()?;
    // Sira onemli: ".server.lua" is_same zamanda ".lua" ile bitiyor, once uzun
    // olanlar denenmeli.
    let (base, ext) = if let Some(b) = fname.strip_suffix(".server.lua") {
        (b, "server.lua")
    } else if let Some(b) = fname.strip_suffix(".client.lua") {
        (b, "client.lua")
    } else if let Some(b) = fname.strip_suffix(".luau") {
        (b, "luau")
    } else {
        (fname.strip_suffix(".lua")?, "lua")
    };

    // Konteyner script: klasör adı objenin adıdır (init.server.lua vb.)
    if base == "init" {
        let parent_dir = path.parent()?.file_name()?.to_str()?;
        if let Some((dir_name, dir_uuid)) = parent_dir.rsplit_once('_') {
            if is_short_uuid(dir_uuid) {
                return Some((dir_name.to_string(), Some(dir_uuid.to_string()), ext.to_string()));
            }
        }
        return Some((parent_dir.to_string(), None, ext.to_string()));
    }

    // Yaprak script: yalnızca layout'un ürettiği 8 haneli kısa UUID eki tanınır.
    // Sayı eki ("Health_2") ARTIK ayrıştırılmaz; layout böyle bir isim üretmiyor ve
    // ayrıştırmak "Health_2" adlı gerçek objenin adını bozuyordu.
    if let Some((name_part, potential_suffix)) = base.rsplit_once('_') {
        if is_short_uuid(potential_suffix) {
            return Some((name_part.to_string(), Some(potential_suffix.to_string()), ext.to_string()));
        }
    }

    Some((base.to_string(), None, ext.to_string()))
}

/// layout'un ürettiği kısa UUID eki mi? (tam 8 hane, hepsi hex)
fn is_short_uuid(s: &str) -> bool {
    s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_script_class(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

/// `Ad.meta.json` ya da `init.meta.json` dosyasindan ilgili script dugumunu bulur.
///
/// Kural layout.rs ile aynidir:
///
/// - `init.meta.json`  -> node_entry, iceren KLASORUN kendisidir (konteyner script)
/// - `Ad.meta.json`    -> node_entry, klasorun `Ad` isimli cocugudur
///
/// Isim cakismasinda layout file_path adina 8 haneli kisa UUID ekler; burada da o ek
/// ayristirilir, aksi halde iki is_same isimli script birbirine karisirdi.
fn meta_target(dm: &crate::model::DataModel, path: &Path) -> Option<uuid::Uuid> {
    let fname = path.file_name()?.to_str()?;
    let base = fname.strip_suffix(".meta.json")?;
    let dir_name = path.parent()?.file_name()?.to_str()?;

    let divide = |s: &str| -> (String, String) {
        match s.rsplit_once('_') {
            Some((n, k)) if is_short_uuid(k) => (n.to_string(), k.to_string()),
            _ => (s.to_string(), String::new()),
        }
    };

    let (dir_clean, dir_short) = divide(dir_name);

    let dir_node = dm.get_all_instances().iter().find_map(|(u, n)| {
        if n.name == dir_clean && (dir_short.is_empty() || u.to_string().starts_with(&dir_short)) {
            Some(*u)
        } else {
            None
        }
    });

    if base == "init" {
        // Konteyner script: klasorun kendisi bir script dugumu olmali.
        return dir_node.filter(|u| {
            dm.get_instance(u)
                .map(|n| is_script_class(&n.class_name))
                .unwrap_or(false)
        });
    }

    let (base_clean, base_short) = divide(base);
    let parent_ref = dir_node?;
    let parent_entry = dm.get_instance(&parent_ref)?;

    parent_entry.children.iter().find_map(|cid| {
        let c = dm.get_instance(cid)?;
        if c.name == base_clean
            && is_script_class(&c.class_name)
            && (base_short.is_empty() || cid.to_string().starts_with(&base_short))
        {
            Some(*cid)
        } else {
            None
        }
    })
}

/// `Ad.txt` dosyasini ilgili StringValue'nun Value'suna uygular.
///
/// StringValue diske duz text_value olarak yazilir (bkz. layout::script_ext); dolayisiyla
/// dosyanin icerigi dogrudan Value'dur. Bu, metni JSON kacis karakterleri icinde
/// duzenlemek zorunda kalmadan editorde acip yazabilmeyi saglar.
fn handle_txt_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(file_content) = fs::read_to_string(path) else {
        return;
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "txt", "StringValue")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the txt file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(file_content.clone());
                if node.properties.get("Value") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Value".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Value", "value": file_content }
                }]
            }),
        });
        debug!("Updated Value from the txt file: {:?}", path);
    }
}

/// `Ad.csv` dosyasini ilgili LocalizationTable'in Contents'ine uygular.
///
/// Ceviriler tabloda duzenlenebilsin diye diske CSV yaziliyor; Roblox tarafinda
/// karsiligi Contents adli JSON metnidir. Donusum kayipsizdir (localization.rs).
fn handle_csv_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(csv) = fs::read_to_string(path) else {
        return;
    };

    let json = match crate::localization::csv_to_json(&csv) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Could not parse CSV ({:?}): {}", path, e);
            return;
        }
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "csv", "LocalizationTable")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the csv file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(json.clone());
                if node.properties.get("Contents") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Contents".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Contents", "value": json }
                }]
            }),
        });
        debug!("Updated Contents from the csv file: {:?}", path);
    }
}

/// Ham file_content dosyalarinin (.txt gibi) sahibi olan dugumu bulur.
/// meta_target ile is_same isim kurallarini kullanir.
fn raw_file_target(
    dm: &crate::model::DataModel,
    path: &Path,
    file_ext: &str,
    class_str: &str,
) -> Option<uuid::Uuid> {
    let fname = path.file_name()?.to_str()?;
    let base = fname.strip_suffix(&format!(".{}", file_ext))?;
    let dir_name = path.parent()?.file_name()?.to_str()?;

    let divide = |s: &str| -> (String, String) {
        match s.rsplit_once('_') {
            Some((n, k)) if is_short_uuid(k) => (n.to_string(), k.to_string()),
            _ => (s.to_string(), String::new()),
        }
    };

    let (dir_clean, dir_short) = divide(dir_name);
    let dir_node = dm.get_all_instances().iter().find_map(|(u, n)| {
        if n.name == dir_clean && (dir_short.is_empty() || u.to_string().starts_with(&dir_short)) {
            Some(*u)
        } else {
            None
        }
    });

    if base == "init" {
        return dir_node.filter(|u| {
            dm.get_instance(u).map(|n| n.class_name == class_str).unwrap_or(false)
        });
    }

    let (base_clean, base_short) = divide(base);
    let parent_ref = dir_node?;
    let parent_entry = dm.get_instance(&parent_ref)?;

    parent_entry.children.iter().find_map(|cid| {
        let c = dm.get_instance(cid)?;
        if c.name == base_clean
            && c.class_name == class_str
            && (base_short.is_empty() || cid.to_string().starts_with(&base_short))
        {
            Some(*cid)
        } else {
            None
        }
    })
}

/// Disk uzerindeki bir .meta.json degisikligini modele ve Studio'ya uygular.
fn handle_meta_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let meta: crate::layout::ScriptMeta = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Could not read meta file ({:?}): {}", path, e);
            return true; // dosyayi tanidik ama icerigi bozuk; json dalina dusmesin
        }
    };

    let uuid = {
        let dm = data_model.blocking_read();
        meta_target(&dm, path)
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the meta file: {:?}", path);
        return true;
    };

    // Yalnizca GERCEKTEN degisenler gonderilir; aksi halde her disk yaziminda
    // Studio'ya gereksiz patch yagardi.
    let mut patches = Vec::new();
    {
        let mut dm = data_model.blocking_write();
        let Some(node) = dm.get_mut_instance(&uuid) else {
            return true;
        };

        for (item_name, raw_value) in &meta.properties {
            if node.properties.get(item_name) != Some(raw_value) {
                node.properties.insert(item_name.clone(), raw_value.clone());
                patches.push(serde_json::json!({
                    "event_type": "PROPERTY_UPDATE",
                    "data": {
                        "syncix_id": uuid,
                        "property": item_name,
                        "value": crate::pv_to_wire(raw_value)
                    }
                }));
            }
        }
        for (item_name, raw_value) in &meta.attributes {
            if node.attributes.get(item_name) != Some(raw_value) {
                node.attributes.insert(item_name.clone(), raw_value.clone());
                patches.push(serde_json::json!({
                    "event_type": "ATTRIBUTE_UPDATE",
                    "data": {
                        "syncix_id": uuid,
                        "name": item_name,
                        "value": crate::pv_to_wire(raw_value)
                    }
                }));
            }
        }

        // Etiketler liste halinde karsilastirilir: attribute gibi single single degil,
        // cunku tag_text "var/yok" bilgisidir; dosyadan cikarilan tag_text gercekten
        // silinmistir. Property'lerden ayrilmasinin sebebi de bu.
        {
            let mut in_file = meta.tags.clone();
            in_file.sort();
            in_file.dedup();
            if node.tags != in_file {
                node.tags = in_file.clone();
                patches.push(serde_json::json!({
                    "event_type": "TAGS_UPDATE",
                    "data": { "syncix_id": uuid, "tags": in_file }
                }));
            }
        }

        // PROPERTY ile ATTRIBUTE burada bilerek FARKLI davranir.
        //
        // Attribute kullanicinin ekledigi ek bir alandir, silinebilir.
        // Property ise Roblox'ta her zaman bir degere sahiptir: Anchored "silinemez",
        // yalnizca true/false olabilir. Bu yuzden bir property'yi meta dosyasindan
        // cikarmak "Studio'da sil" anlamina GELMEZ; olsa olsa "Syncix bunu artik
        // kaydetmesin" demektir. Studio o property'yi izlemeye devam ettigi icin
        // raw_value bir sonraki senkronda restored_count gelir.
        //
        // Bu yuzden property yoklugunda hicbir sey yapmiyoruz, ama kullanici
        // beklentisi bosa cikmasin diye durumu loga yaziyoruz.
        let missing_properties: Vec<&String> = node
            .properties
            .keys()
            .filter(|k| !meta.properties.contains_key(*k))
            .collect();
        if !missing_properties.is_empty() {
            tracing::info!(
                "Properties removed from the meta file were ignored ({:?}): {:?}. \
                 Properties cannot be deleted in Roblox; write the new value instead.",
                path.file_name().unwrap_or_default(),
                missing_properties
            );
        }

        // Dosyada ARTIK OLMAYAN attribute silinmis demektir.
        //
        // Bu ancak yazim kaydi korumasi sayesinde guvenli: own yazdigimiz dosyayi
        // again okumadigimiz icin "eksik" olan sey gercekten kullanicinin sildigi
        // seydir, gecikmis bir olayin previous_text hali degil.
        let delete_list: Vec<String> = node
            .attributes
            .keys()
            .filter(|k| !meta.attributes.contains_key(*k))
            .cloned()
            .collect();
        for item_name in delete_list {
            node.attributes.remove(&item_name);
            patches.push(serde_json::json!({
                "event_type": "ATTRIBUTE_UPDATE",
                "data": { "syncix_id": uuid, "name": item_name, "value": serde_json::Value::Null }
            }));
        }
    }

    if !patches.is_empty() {
        let number_value = patches.len();
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({ "patches": patches }),
        });
        debug!("Sent {} change(s) from the meta file to Studio", number_value);
    }
    true
}

// Argumanlarin cogu bagimsiz kanal (Studio kuyrugu, model, editor yayini, disk
// uyarisi) ve hepsi single bir olayin islenmesinde gerekli. Tek bir yapiya
// toplamak, cagri zincirindeki her halkanin o yapiyi tasimasini gerektirirdi;
// okunurlugu arttirmiyor.
#[allow(clippy::too_many_arguments)]
fn handle_event(
    event: Event,
    tx_to_studio: &StudioOutbox,
    serializer: &PartSerializer,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    disk_notify: &Arc<tokio::sync::Notify>,
    sync_dir: &str,
    ignore: &[String],
    pending_deletes: &mut std::collections::HashMap<Uuid, std::time::Instant>,
) {
    // Askidayken diskten hicbir sey okunmaz ve Studio'ya hicbir sey gonderilmez.
    // Asil tehlike buydu: modelde olmayan bir file_path "yeni obje" sanilip Studio'ya
    // yaratiliyordu; yanlis place bagliyken bu, previous_text oyunun fresh place'e
    // dolmasi demekti.
    if crate::project::is_sync_suspended() {
        return;
    }

    // Silme: uzun sure tamamen yok sayiliyordu, cunku disk yazicisi agaci
    // yeniden yazarken file_path siliyor ve bunlar yanlis DESTROY uretiyordu.
    // Artik yazicinin own silmeleri kayitli oldugu icin ayirt edilebiliyor:
    // kayitta olmayan bir deletion gercek kullanici silmesidir.
    if matches!(event.kind, EventKind::Remove(_)) {
        for path in &event.paths {
            if crate::layout::is_ignored(path, sync_dir, ignore) {
                continue;
            }
            if crate::layout::is_own_delete(path) {
                continue;
            }
            let uuid = {
                let dm = data_model.blocking_read();
                crate::layout::uuid_for_path(&dm, sync_dir, path)
            };
            match uuid {
                Some(u) => {
                    info!("A file was deleted from disk: {}", path.display());
                    pending_deletes.insert(u, std::time::Instant::now());
                }
                // Sessizce dusen silmeler tesise edilemiyordu: fs_path eslestirmesi
                // bozuldugunda hicbir iz kalmiyordu.
                None => info!(
                    "A file was deleted but no instance matched it, so nothing was removed: {}",
                    path.display()
                ),
            }
        }
        return;
    }

    // Yalnızca içerik oluşturma/değiştirme olaylarıyla ilgileniyoruz.
    let is_relevant = matches!(
        event.kind,
        EventKind::Any | EventKind::Modify(_) | EventKind::Create(_)
    );
    if !is_relevant {
        return;
    }

    // Bir file_path yeniden ortaya ciktiysa o instance silinmemis, TASINMIS demektir.
    // Bekleyen deletion iptal edilir.
    if !pending_deletes.is_empty() {
        let dm = data_model.blocking_read();
        for path in &event.paths {
            if let Some(u) = crate::layout::uuid_for_path(&dm, sync_dir, path) {
                pending_deletes.remove(&u);
            }
        }
    }

    let mut old_info: Option<(String, Option<String>)> = None;
    if event.paths.len() > 1 {
        if let Some((old_clean, old_uuid, _)) = parse_script_filename(&event.paths[0]) {
            old_info = Some((old_clean, old_uuid));
        }
    }

    for path in &event.paths {
        // Kullanicinin yok saydirdigi yollar okunmaz.
        if crate::layout::is_ignored(path, sync_dir, ignore) {
            continue;
        }

        // Kendi yazimimizi isleme: disk yazicisi bir dosyayi yazdiginda izleyici
        // bunu kullanici degisikligi saniyordu. Olaylar gecikmeli geldigi icin
        // bazen dosyanin ESKI hali okunup model geriye sariliyordu.
        if let Ok(current_value) = fs::read_to_string(path) {
            if crate::layout::is_own_write(path, &current_value) {
                continue;
            }
        }

        // .meta.json uzantisi "json" oldugu icin asagidaki json dalina duser ve
        // InstanceNode olarak cozulemeyip sessizce yutulurdu. Once burada yakalanir.
        if path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.ends_with(".meta.json"))
            .unwrap_or(false)
            && handle_meta_file(path, tx_to_studio, data_model) {
                continue;
            }

        // .txt -> StringValue.Value
        if path
            .extension()
            .map(|e| e.to_string_lossy() == "txt")
            .unwrap_or(false)
        {
            handle_txt_file(path, tx_to_studio, data_model);
            continue;
        }

        // .csv -> LocalizationTable.Contents
        if path
            .extension()
            .map(|e| e.to_string_lossy() == "csv")
            .unwrap_or(false)
        {
            handle_csv_file(path, tx_to_studio, data_model);
            continue;
        }

        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy();

            if ext_str == "json" {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(instance) = serializer.deserialize(&content) {
                        let diff_patch = {
                            let dm = data_model.blocking_read();
                            if let Some(old_node) = dm.get_instance(&instance.syncix_id) {
                                instance.diff(old_node)
                            } else {
                                None
                            }
                        };

                        if let Some(patch) = diff_patch {
                            let mut patches = Vec::new();
                            for (prop_name, prop_value) in patch.changed_properties {
                                patches.push(serde_json::json!({
                                    "event_type": "PROPERTY_UPDATE",
                                    "data": {
                                        "syncix_id": patch.syncix_id,
                                        "property": prop_name,
                                        "value": crate::pv_to_wire(&prop_value)
                                    }
                                }));
                            }

                            if !patches.is_empty() {
                                let payload = Payload {
                                    version: "v1".to_string(),
                                    event_type: EventType::CompositeUpdate,
                                    data: serde_json::json!({
                                        "patches": patches
                                    }),
                                };
                                tx_to_studio.push(payload);
                                debug!("Sent diff (CompositeUpdate): {} patch(es)", patches.len());
                            }
                        } else {
                            let is_new = {
                                let dm = data_model.blocking_read();
                                dm.get_instance(&instance.syncix_id).is_none()
                            };
                            if is_new {
                                let payload = Payload {
                                    version: "v1".to_string(),
                                    event_type: EventType::PushUpdate,
                                    data: serde_json::to_value(&instance).unwrap(),
                                };
                                tx_to_studio.push(payload);
                                debug!("Sent new instance (PushUpdate): {}", instance.name);
                            }
                        }

                        {
                            let mut dm = data_model.blocking_write();
                            let _ = dm.upsert_instance(instance);
                        }
                    }
                }
            } else if ext_str == "lua" || ext_str == "luau" {
                let parsed = parse_script_filename(path);
                if let Some((clean_name, lua_uuid_opt, lua_ext)) = parsed {
                    let dm = data_model.blocking_read();
                    
                    let parent_uuid_opt = if let Some(parent_dir) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
                        let (parent_name, parent_short) = if let Some((pn, ps)) = parent_dir.rsplit_once('_') {
                            (pn, ps)
                        } else {
                            (parent_dir, "")
                        };

                        dm.get_all_instances().iter().find_map(|(u, n)| {
                            if n.name == parent_name && (parent_short.is_empty() || u.to_string().starts_with(parent_short)) {
                                Some(*u)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };

                    // UUID ekiyle search; bulunamazsa (ör. kullanıcı eki sildi) isim
                    // eşleşmesine düşülür — böylece file_path sahipsiz kalmaz.
                    let by_uuid = lua_uuid_opt
                        .as_ref()
                        .and_then(|s| dm.find_by_short_uuid(s));

                    let target_uuid = if by_uuid.is_some() {
                        by_uuid
                    } else if let Some((_, Some(ref old_short))) = old_info {
                        dm.find_by_short_uuid(old_short)
                    } else if let Some(old_name) = old_info.as_ref().map(|o| &o.0) {
                        if let Some(parent_uuid) = parent_uuid_opt {
                            dm.get_all_instances().iter().find_map(|(u, n)| {
                                if n.parent == Some(parent_uuid) && n.name.as_str() == old_name.as_str() {
                                    Some(*u)
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    } else if let Some(parent_uuid) = parent_uuid_opt {
                        let exact_match = dm.get_all_instances().iter().find_map(|(u, n)| {
                            if n.parent == Some(parent_uuid) && n.name == clean_name {
                                Some(*u)
                            } else {
                                None
                            }
                        });

                        exact_match.or_else(|| {
                            dm.get_all_instances().iter().find_map(|(u, n)| {
                                if n.parent == Some(parent_uuid) && (n.class_name == "Script" || n.class_name == "LocalScript" || n.class_name == "ModuleScript") {
                                    let parent_dir_path = path.parent()?;
                                    let clean = n.name.clone();
                                    let f1 = parent_dir_path.join(format!("{}.server.lua", clean));
                                    let f2 = parent_dir_path.join(format!("{}.client.lua", clean));
                                    let f3 = parent_dir_path.join(format!("{}.lua", clean));

                                    let file_exists = f1.exists() || f2.exists() || f3.exists();
                                    if !file_exists {
                                        Some(*u)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                        })
                    } else {
                        None
                    };

                    // TASIMA. Bir dosyayi baska klasore tasimak isletim sisteminde
                    // "sil + olustur" olarak gorunuyor. Yukaridaki eslestirmeler
                    // yalnizca AYNI klasore bakiyor, dolayisiyla tasinan file_path
                    // sahipsiz kaliyor ve ikinci bir instance yaratiliyordu.
                    // Bekleyen silmeler arasinda is_same adda ve is_same sinifta bir
                    // script varsa bu fresh bir obje degil, tasinmis olanidir.
                    let target_uuid = target_uuid.or_else(|| {
                        pending_deletes.keys().copied().find(|u| {
                            dm.get_instance(u)
                                .map(|n| n.name == clean_name && is_script_class(&n.class_name))
                                .unwrap_or(false)
                        })
                    });

                    if let Some(uuid) = target_uuid {
                        // Tasima olarak eslestiyse deletion iptal edilir.
                        pending_deletes.remove(&uuid);

                        // Yeni folder_path baska bir ebeveyne isaret ediyorsa objenin
                        // agactaki yeri de degismeli.
                        let parent_diff = match (parent_uuid_opt, dm.get_instance(&uuid).and_then(|n| n.parent)) {
                            (Some(fresh), previous_text) if Some(fresh) != previous_text => Some(fresh),
                            _ => None,
                        };

                        let source_diff = if let Ok(src) = fs::read_to_string(path) {
                            dm.get_instance(&uuid).map(|inst| inst.source.as_deref() != Some(src.as_str())).unwrap_or(false)
                        } else {
                            false
                        };

                        let inst_name = dm.get_instance(&uuid).map(|n| n.name.clone()).unwrap_or_default();
                        let name_diff = inst_name != clean_name && !clean_name.is_empty();

                        drop(dm);

                        if let Some(moved_to) = parent_diff {
                            {
                                let mut dm = data_model.blocking_write();
                                if let Err(e) = dm.reparent(&uuid, Some(moved_to)) {
                                    error!("The moved file could not be reparented: {}", e);
                                }
                            }
                            tx_to_studio.push(Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "REPARENT",
                                        "data": {
                                            "syncix_id": uuid,
                                            "newParentId": moved_to
                                        }
                                    }]
                                }),
                            });
                            info!("The file was moved; the instance was reparented: {}", clean_name);
                        }

                        if name_diff {
                            let payload = Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "RENAME_INSTANCE",
                                        "data": {
                                            "id": uuid,
                                            "newName": clean_name
                                        }
                                    }]
                                }),
                            };
                            tx_to_studio.push(payload);
                            debug!("Script renamed -> Studio: {} ({})", clean_name, uuid);

                            {
                                let mut dm_write = data_model.blocking_write();
                                if let Some(inst) = dm_write.get_mut_instance(&uuid) {
                                    inst.name = clean_name.clone();
                                }
                            }
                            // Editöre bildir + diski tazele (isim değişti, fs_path değişebilir)
                            {
                                let dm_read = data_model.blocking_read();
                                notify_vscode_updated(&dm_read, &uuid, tx_to_vscode, "INSTANCE_UPDATED");
                            }
                            disk_notify.notify_one();
                        }

                        if source_diff {
                            if let Ok(source_code) = fs::read_to_string(path) {
                                let payload = Payload {
                                    version: "v1".to_string(),
                                    event_type: EventType::CompositeUpdate,
                                    data: serde_json::json!({
                                        "patches": [{
                                            "event_type": "PROPERTY_UPDATE",
                                            "data": {
                                                "syncix_id": uuid,
                                                "property": "Source",
                                                "value": source_code
                                            }
                                        }]
                                    }),
                                };
                                tx_to_studio.push(payload);

                                {
                                    let mut dm_write = data_model.blocking_write();
                                    if let Some(inst) = dm_write.get_mut_instance(&uuid) {
                                        inst.source = Some(source_code);
                                    }
                                }
                            }
                        }

                        // NOT: Dosyayı burada yeniden adlandırmıyoruz. Diskin doğru
                        // isimlendirmesi layout modülünün sorumluluğunda; iki taraf
                        // farklı kural uygularsa file_path adı savaşı/döngü oluşuyordu.
                    } else if let Some(parent_uuid) = parent_uuid_opt {
                        drop(dm);
                        let new_uuid = Uuid::new_v4();
                        let script_class = match lua_ext.as_str() {
                            "server.lua" => "Script",
                            "client.lua" => "LocalScript",
                            "lua" | "luau" => "ModuleScript",
                            _ => "Script",
                        };

                        let source_code = fs::read_to_string(path).unwrap_or_default();

                        let mut new_node = InstanceNode::new(script_class, &clean_name);
                        new_node.syncix_id = new_uuid;
                        new_node.parent = Some(parent_uuid);
                        new_node.source = Some(source_code.clone());

                        let payload = Payload {
                            version: "v1".to_string(),
                            event_type: EventType::CompositeUpdate,
                            data: serde_json::json!({
                                "patches": [
                                    {
                                        "event_type": "CREATE",
                                        "data": {
                                            "syncix_id": new_uuid,
                                            "class_name": script_class,
                                            "name": clean_name,
                                            "parent": parent_uuid
                                        }
                                    },
                                    {
                                        "event_type": "PROPERTY_UPDATE",
                                        "data": {
                                            "syncix_id": new_uuid,
                                            "property": "Source",
                                            "value": source_code
                                        }
                                    }
                                ]
                            }),
                        };

                        tx_to_studio.push(payload);

                        {
                            let mut dm_write = data_model.blocking_write();
                            let _ = dm_write.upsert_instance(new_node);
                        }
                        // Editöre bildir + diski tazele
                        {
                            let dm_read = data_model.blocking_read();
                            notify_vscode_updated(&dm_read, &new_uuid, tx_to_vscode, "INSTANCE_CREATED");
                        }
                        disk_notify.notify_one();
                        info!("New script created on disk -> Studio: {} ({})", clean_name, new_uuid);
                    }
                }
            }
        }
    }
}
