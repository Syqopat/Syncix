#[allow(dead_code)]
mod assets;
#[allow(dead_code)]
mod auth;
#[allow(dead_code)]
mod bus;
mod cli;
#[allow(dead_code)]
mod command;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod contract;
mod file_sync;
mod health;
mod layout;
mod localization;
#[allow(dead_code)]
mod logging;
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod pipeline;
mod project;
mod rbxmx;
mod rbxmx_import;
mod sourcemap;
mod upload;
#[allow(dead_code)]
mod scheduler;
#[allow(dead_code)]
mod schema;
#[allow(dead_code)]
mod serializers;
mod server;
#[allow(dead_code)]
mod snapshot;
#[allow(dead_code)]
mod transport;
#[allow(dead_code)]
mod tree_builder;
#[allow(dead_code)]
mod vfs;

use std::fs;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::bus::EventBus;
use crate::model::InstanceNode;
use crate::server::AppState;
use crate::transport::{EventType, Payload, StudioOutbox};

/// Bir komut hedefini çözümler: tam UUID, kısa UUID öneki veya isim.
/// İsim birden fazla objeyle eşleşirse belirsizlik nedeniyle None döner.
fn resolve_id(dm: &model::DataModel, target: &str) -> Option<uuid::Uuid> {
    match dm.resolve_target(target) {
        model::ResolveResult::One(u) => Some(u),
        model::ResolveResult::NotFound => None,
        model::ResolveResult::Ambiguous(candidates) => {
            tracing::warn!(
                "{} instances match the name '{}'; the request was ambiguous so nothing was done. Use the short UUID.",
                target,
                candidates.len()
            );
            None
        }
    }
}

/// CLI'dan gelen string değeri uygun PropertyValue'ya çevirir.
/// "true"/"false" -> Boolean, "x,y,z" -> Vector3, sayı -> Number, aksi -> String.
fn parse_property_value(s: &str) -> model::PropertyValue {
    use model::PropertyValue;
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        return PropertyValue::Boolean(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return PropertyValue::Boolean(false);
    }

    // Hex renk: "#ff8800" veya "ff8800" -> Color3
    // (Eskiden düz metin olarak Studio'ya gidip reddediliyordu.)
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) && t.starts_with('#') {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return PropertyValue::Color3 {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
            };
        }
    }
    let parts: Vec<&str> = t.split(',').collect();
    if parts.len() == 3 {
        if let (Ok(x), Ok(y), Ok(z)) = (
            parts[0].trim().parse::<f32>(),
            parts[1].trim().parse::<f32>(),
            parts[2].trim().parse::<f32>(),
        ) {
            return PropertyValue::Vector3 { x, y, z };
        }
    }
    if let Ok(n) = t.parse::<f64>() {
        return PropertyValue::Number(n);
    }
    PropertyValue::String(t.to_string())
}

/// Terminalden girilen metni, property'nin modeldeki MEVCUT değerinin tipine
/// uydurur. Uyduramazsa None döner ve çağıran genel ayrıştırıcıya düşer.
///
/// Neden var: `syncix set <part> BrickColor "Really red"` düz metin üretiyordu ve
/// Studio tarafında atama sessizce başarısız oluyordu. Aynı sorun CFrame, UDim2,
/// NumberRange gibi tiplerde de vardı — terminalden hiç ayarlanamıyorlardı.
fn mevcut_tipe_uydur(mevcut: &model::PropertyValue, metin: &str) -> Option<model::PropertyValue> {
    use model::PropertyValue as P;
    let t = metin.trim();

    /// "1, 2, 3" -> [1.0, 2.0, 3.0]; sayı olmayan varsa None.
    fn sayilar(t: &str) -> Option<Vec<f32>> {
        t.split(',')
            .map(|p| p.trim().parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()
    }

    match mevcut {
        P::BrickColor(_) => Some(P::BrickColor(t.to_string())),
        // Ref hedefi UUID ya da kısa tutamaç olabilir; çözümleme Studio tarafında.
        P::Ref(_) => Some(P::Ref(t.to_string())),
        P::String(_) => Some(P::String(t.to_string())),
        // Asset id'si metin olarak yaziliyor: "rbxassetid://123".
        P::Content(_) => Some(P::Content(t.to_string())),
        P::Vector2 { .. } => match sayilar(t)?[..] {
            [x, y] => Some(P::Vector2 { x, y }),
            _ => None,
        },
        P::UDim { .. } => match sayilar(t)?[..] {
            [scale, offset] => Some(P::UDim { scale, offset }),
            _ => None,
        },
        P::NumberRange { .. } => match sayilar(t)?[..] {
            [min, max] => Some(P::NumberRange { min, max }),
            // Tek sayı verilirse aralık o noktaya sabitlenir.
            [tek] => Some(P::NumberRange { min: tek, max: tek }),
            _ => None,
        },
        P::UDim2 { .. } => match sayilar(t)?[..] {
            [xs, xo, ys, yo] => Some(P::UDim2 { xs, xo, ys, yo }),
            _ => None,
        },
        P::CFrame { rot, .. } => {
            let s = sayilar(t)?;
            match s.len() {
                // Sadece konum verildi: mevcut dönme korunur. En sık istenen bu.
                3 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: *rot,
                }),
                12 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: [s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11]],
                }),
                _ => None,
            }
        }
        // Color3 için "#ff8800" ve "1,0.5,0" ikisi de geçerli.
        P::Color3 { .. } => {
            if let P::Color3 { r, g, b } = parse_property_value(t) {
                return Some(P::Color3 { r, g, b });
            }
            match sayilar(t)?[..] {
                [r, g, b] => Some(P::Color3 { r, g, b }),
                _ => None,
            }
        }
        P::Rect { .. } => match sayilar(t)?[..] {
            [x0, y0, x1, y1] => Some(P::Rect {
                min: [x0, y0],
                max: [x1, y1],
            }),
            _ => None,
        },
        P::PhysicalProperties { .. } => match sayilar(t)?[..] {
            [d, f, e, fw, ew] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: fw,
                elasticity_weight: ew,
            }),
            // Üç değer en sık kullanılan hali; ağırlıklar Roblox varsayılanında kalır.
            [d, f, e] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: 1.0,
                elasticity_weight: 1.0,
            }),
            _ => None,
        },
        // Eğri tipleri terminalden nokta nokta yazılamaz; verilen değerler
        // zaman ekseninde EŞİT aralıklarla dağıtılır. "1,0" = baştan sona sönme.
        P::NumberSequence(_) => {
            let v = sayilar(t)?;
            if v.is_empty() {
                return None;
            }
            let son = (v.len() - 1).max(1) as f32;
            Some(P::NumberSequence(
                v.iter()
                    .enumerate()
                    .map(|(i, deger)| model::NumberKeypoint {
                        t: i as f32 / son,
                        v: *deger,
                        envelope: 0.0,
                    })
                    .collect(),
            ))
        }
        // "#ff0000,#0000ff" gibi: renkler eşit aralıklarla dağıtılır.
        P::ColorSequence(_) => {
            let mut noktalar = Vec::new();
            let parcalar: Vec<&str> = t.split(',').map(|x| x.trim()).collect();
            let son = (parcalar.len().saturating_sub(1)).max(1) as f32;
            for (i, parca) in parcalar.iter().enumerate() {
                match parse_property_value(parca) {
                    P::Color3 { r, g, b } => noktalar.push(model::ColorKeypoint {
                        t: i as f32 / son,
                        r,
                        g,
                        b,
                    }),
                    // Biri bile renk değilse tamamı reddedilir: yarısı uygulanan
                    // bir eğri, hiç uygulanmayandan daha kafa karıştırıcı.
                    _ => return None,
                }
            }
            (!noktalar.is_empty()).then_some(P::ColorSequence(noktalar))
        }
        // Yalnızca aile değiştirilir; kalınlık ve stil mevcut haliyle korunur.
        P::Font { weight, style, .. } => Some(P::Font {
            family: t.to_string(),
            weight: weight.clone(),
            style: style.clone(),
        }),
        // Kalanlar (Vector3, Number, Boolean) genel ayrıştırıcıda zaten doğru çıkıyor.
        _ => None,
    }
}

/// Tel formatındaki bir JSON değeri PropertyValue'ya çevirir.
/// Hem ham skalerleri (5, "hi", true) hem de {Vector3:{..}}/{Color3:{..}} tablolarını
/// hem de serde enum formatını ({"Number":5}) kabul eder.
fn parse_wire_value(v: &serde_json::Value) -> Option<model::PropertyValue> {
    use model::PropertyValue;
    match v {
        serde_json::Value::String(s) => Some(PropertyValue::String(s.clone())),
        serde_json::Value::Bool(b) => Some(PropertyValue::Boolean(*b)),
        serde_json::Value::Number(n) => n.as_f64().map(PropertyValue::Number),
        serde_json::Value::Object(o) => {
            if let Some(v3) = o.get("Vector3") {
                Some(PropertyValue::Vector3 {
                    x: v3.get("x")?.as_f64()? as f32,
                    y: v3.get("y")?.as_f64()? as f32,
                    z: v3.get("z")?.as_f64()? as f32,
                })
            } else if let Some(c3) = o.get("Color3") {
                Some(PropertyValue::Color3 {
                    r: c3.get("r")?.as_f64()? as f32,
                    g: c3.get("g")?.as_f64()? as f32,
                    b: c3.get("b")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim2") {
                Some(PropertyValue::UDim2 {
                    xs: u.get("xs")?.as_f64()? as f32,
                    xo: u.get("xo")?.as_f64()? as f32,
                    ys: u.get("ys")?.as_f64()? as f32,
                    yo: u.get("yo")?.as_f64()? as f32,
                })
            } else if let Some(v) = o.get("Vector2") {
                Some(PropertyValue::Vector2 {
                    x: v.get("x")?.as_f64()? as f32,
                    y: v.get("y")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim") {
                Some(PropertyValue::UDim {
                    scale: u.get("scale")?.as_f64()? as f32,
                    offset: u.get("offset")?.as_f64()? as f32,
                })
            } else if let Some(c) = o.get("CFrame") {
                let pos = c.get("pos")?.as_array()?;
                let rot = c.get("rot")?.as_array()?;
                if pos.len() != 3 || rot.len() != 9 {
                    return None;
                }
                let mut p = [0f32; 3];
                let mut r = [0f32; 9];
                for (i, v) in pos.iter().enumerate() {
                    p[i] = v.as_f64()? as f32;
                }
                for (i, v) in rot.iter().enumerate() {
                    r[i] = v.as_f64()? as f32;
                }
                Some(PropertyValue::CFrame { pos: p, rot: r })
            } else if let Some(n) = o.get("NumberRange") {
                Some(PropertyValue::NumberRange {
                    min: n.get("min")?.as_f64()? as f32,
                    max: n.get("max")?.as_f64()? as f32,
                })
            } else if let Some(r) = o.get("Ref") {
                Some(PropertyValue::Ref(r.as_str()?.to_string()))
            } else if let Some(b) = o.get("BrickColor") {
                Some(PropertyValue::BrickColor(b.as_str()?.to_string()))
            } else if let Some(c) = o.get("Content") {
                Some(PropertyValue::Content(c.as_str()?.to_string()))
            } else if let Some(a) = o.get("ColorSequence") {
                let mut noktalar = Vec::new();
                for k in a.as_array()? {
                    noktalar.push(model::ColorKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        r: k.get("r")?.as_f64()? as f32,
                        g: k.get("g")?.as_f64()? as f32,
                        b: k.get("b")?.as_f64()? as f32,
                    });
                }
                Some(PropertyValue::ColorSequence(noktalar))
            } else if let Some(a) = o.get("NumberSequence") {
                let mut noktalar = Vec::new();
                for k in a.as_array()? {
                    noktalar.push(model::NumberKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        v: k.get("v")?.as_f64()? as f32,
                        envelope: k.get("envelope").and_then(|e| e.as_f64()).unwrap_or(0.0) as f32,
                    });
                }
                Some(PropertyValue::NumberSequence(noktalar))
            } else if let Some(r) = o.get("Rect") {
                let mn = r.get("min")?.as_array()?;
                let mx = r.get("max")?.as_array()?;
                if mn.len() != 2 || mx.len() != 2 {
                    return None;
                }
                Some(PropertyValue::Rect {
                    min: [mn[0].as_f64()? as f32, mn[1].as_f64()? as f32],
                    max: [mx[0].as_f64()? as f32, mx[1].as_f64()? as f32],
                })
            } else if let Some(f) = o.get("Font") {
                Some(PropertyValue::Font {
                    family: f.get("family")?.as_str()?.to_string(),
                    weight: f.get("weight")?.as_str()?.to_string(),
                    style: f.get("style")?.as_str()?.to_string(),
                })
            } else if let Some(pp) = o.get("PhysicalProperties") {
                Some(PropertyValue::PhysicalProperties {
                    density: pp.get("density")?.as_f64()? as f32,
                    friction: pp.get("friction")?.as_f64()? as f32,
                    elasticity: pp.get("elasticity")?.as_f64()? as f32,
                    friction_weight: pp.get("frictionWeight")?.as_f64()? as f32,
                    elasticity_weight: pp.get("elasticityWeight")?.as_f64()? as f32,
                })
            } else {
                serde_json::from_value(v.clone()).ok()
            }
        }
        _ => None,
    }
}

/// PropertyValue'yu Studio plugininin (PatchExecutor) beklediği tel formatına çevirir.
/// Skalarlar ham gönderilir; Vector3/Color3 tablo olarak sarılır.
pub fn pv_to_wire(pv: &model::PropertyValue) -> serde_json::Value {
    use model::PropertyValue;
    match pv {
        PropertyValue::String(s) => serde_json::json!(s),
        PropertyValue::Number(n) => serde_json::json!(n),
        PropertyValue::Boolean(b) => serde_json::json!(b),
        PropertyValue::Vector3 { x, y, z } => serde_json::json!({ "Vector3": { "x": x, "y": y, "z": z } }),
        PropertyValue::Color3 { r, g, b } => serde_json::json!({ "Color3": { "r": r, "g": g, "b": b } }),
        PropertyValue::UDim2 { xs, xo, ys, yo } => {
            serde_json::json!({ "UDim2": { "xs": xs, "xo": xo, "ys": ys, "yo": yo } })
        }
        PropertyValue::Vector2 { x, y } => serde_json::json!({ "Vector2": { "x": x, "y": y } }),
        PropertyValue::UDim { scale, offset } => {
            serde_json::json!({ "UDim": { "scale": scale, "offset": offset } })
        }
        PropertyValue::CFrame { pos, rot } => {
            serde_json::json!({ "CFrame": { "pos": pos, "rot": rot } })
        }
        PropertyValue::NumberRange { min, max } => {
            serde_json::json!({ "NumberRange": { "min": min, "max": max } })
        }
        PropertyValue::Ref(id) => serde_json::json!({ "Ref": id }),
        PropertyValue::BrickColor(ad) => serde_json::json!({ "BrickColor": ad }),
        PropertyValue::Content(u) => serde_json::json!({ "Content": u }),
        PropertyValue::ColorSequence(k) => serde_json::json!({ "ColorSequence": k }),
        PropertyValue::NumberSequence(k) => serde_json::json!({ "NumberSequence": k }),
        PropertyValue::Rect { min, max } => {
            serde_json::json!({ "Rect": { "min": min, "max": max } })
        }
        PropertyValue::Font {
            family,
            weight,
            style,
        } => serde_json::json!({
            "Font": { "family": family, "weight": weight, "style": style }
        }),
        PropertyValue::PhysicalProperties {
            density,
            friction,
            elasticity,
            friction_weight,
            elasticity_weight,
        } => serde_json::json!({
            "PhysicalProperties": {
                "density": density,
                "friction": friction,
                "elasticity": elasticity,
                "frictionWeight": friction_weight,
                "elasticityWeight": elasticity_weight
            }
        }),
    }
}

#[tokio::main]
async fn main() {
    // CLI kipi: argüman verilmişse istemci gibi davran, sunucu açma.
    // Aynı binary hem sunucu hem CLI olduğu için PowerShell/Node bağımlılığı yok.
    let argumanlar: Vec<String> = std::env::args().skip(1).collect();
    if let Some(kod) = cli::calistir(&argumanlar) {
        std::process::exit(kod);
    }

    // Log filtresi: hyper/tower gibi bağımlılıkların DEBUG gürültüsünü kapat,
    // Syncix'in kendi olayları DEBUG seviyesine kadar görünsün.
    // İstenirse RUST_LOG ortam değişkeni ile ezilebilir.
    // Loglar hem konsola hem dosyaya yazılır (../syncix-core.log); çünkü core
    // artık VS Code tarafından arka planda başlatılabiliyor ve konsolu görünmüyor.
    use tracing_subscriber::prelude::*;
    let file_appender = tracing_appender::rolling::never("../", "syncix-core.log");
    let (file_writer, _log_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,syncix_core=debug")),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer),
        )
        .init();

    info!(
        "Starting Syncix Core Engine... (version {}, protocol {})",
        project::VERSION,
        project::PROTOCOL_VERSION
    );

    // Proje yapılandırması: senkron klasörü, port ve proje kimliği.
    // `syncix serve 25565` biçiminde açık port verilmişse ayarın önüne geçer.
    let mut cfg_ham = project::ProjectConfig::load();
    if let Some(p) = argumanlar
        .get(1)
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|_| argumanlar.first().map(|s| s.as_str()) == Some("serve"))
    {
        info!("Port requested on the command line: {}", p);
        cfg_ham.wanted_port = p;
        // Açıkça istenen port SABİTTİR: doluysa sıradakine kaymaz.
        // Aksi halde Studio eklentisine yazdığın port ile core'un portu ayrışırdı.
        cfg_ham.port_sabit = true;
    }
    let cfg = Arc::new(cfg_ham);
    let sync_dir: &'static str = Box::leak(cfg.sync_dir.clone().into_boxed_str());
    info!("Project: {} ({})", cfg.name, cfg.root.display());
    info!("Sync folder: {}", sync_dir);
    fs::create_dir_all(sync_dir).unwrap();

    // Portu ŞİMDİ bağlıyoruz: gerçek port AppState'e girmeli, çünkü /health onu
    // dışarı bildiriyor ve Studio eklentisi ile editör o bilgiyle buluyor.
    let Some((listener, actual_port)) = server::bind_with_fallback(&cfg) else {
        error!("Syncix could not start: no port available.");
        std::process::exit(1);
    };
    cfg.write_port_file(actual_port);

    // 1. Merkezi Sistemlerin Başlatılması
    let _event_bus = Arc::new(EventBus::new());
    let data_model = model::create_shared_model();

    // Transport (HTTP) kanalları
    // Studio'ya giden mesajlar kayıpsız teslimat için kuyruğa (Outbox) alınır.
    let studio_outbox = Arc::new(StudioOutbox::new());
    let (tx_to_core, mut rx_from_studio) = mpsc::channel::<Payload>(100);

    // VS Code RPC Kanalı
    let (tx_to_vscode, _) = broadcast::channel::<String>(100);

    let health_monitor = Arc::new(crate::health::HealthMonitor::new());

    let app_state = Arc::new(AppState {
        studio_outbox: studio_outbox.clone(),
        tx_to_core,
        tx_to_vscode,
        health_monitor: health_monitor.clone(),
        data_model: data_model.clone(),
        chaos_mode_enabled: false, // Normalde Config'den alınmalı
        project: cfg.clone(),
        actual_port,
        place_catismasi: Arc::new(std::sync::Mutex::new(None)),
    });

    // 2. Disk Yazıcısı (Debounced): Model her değiştiğinde tetiklenir; kısa bir sessizlik
    // sonrası tüm ağacı Studio Explorer'ın birebir kopyası olarak diske yazar.
    // Debounce sayesinde sürükleme gibi hızlı değişimlerde disk fırtınası oluşmaz.
    // NOT: Diske YAZMAK yalnızca burasının işidir; file_sync sadece okur.
    let disk_notify = Arc::new(tokio::sync::Notify::new());
    {
        let data_model_for_writer = data_model.clone();
        let notify_for_writer = disk_notify.clone();
        let cfg_for_writer = cfg.clone();
        tokio::spawn(async move {
            loop {
                notify_for_writer.notified().await;
                // Sessizlik olana kadar bekle (art arda gelen değişiklikleri birleştir)
                loop {
                    tokio::select! {
                        _ = notify_for_writer.notified() => continue,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(
                            cfg_for_writer.debounce_ms,
                        )) => break,
                    }
                }
                // Askidayken diske DOKUNMA. Uzlastirici modeli dogruluk sayar;
                // model askidayken eksik ya da yanlis olabilecegi icin diski
                // ona uydurmak dosyalari silmek demek olurdu.
                if crate::project::senkron_askida() {
                    continue;
                }
                let dm = data_model_for_writer.read().await;
                layout::write_full_tree(&dm, sync_dir, &cfg_for_writer.ignore);

                // sourcemap.json: luau-lsp'nin otomatik tamamlama yapabilmesi için
                // hangi dosyanın DataModel'de nereye karşılık geldiğini bildirir.
                // Ağaç her değiştiğinde tazelenir; ayrı bir izleyici sürece gerek yok.
                if cfg_for_writer.sourcemap {
                    let icerik = sourcemap::json(&dm, sync_dir, &cfg_for_writer.root);
                    let hedef = cfg_for_writer.sourcemap_file();
                    let ayni = std::fs::read_to_string(&hedef)
                        .map(|m| m == icerik)
                        .unwrap_or(false);
                    if !ayni {
                        if let Err(e) = std::fs::write(&hedef, icerik) {
                            tracing::warn!("Could not write sourcemap.json: {}", e);
                        }
                    }
                }
            }
        });
    }

    // 2b. File Watcher'ı başlat (Diskteki değişiklikleri okur; ASLA diske yazmaz).
    // Editör bildirimi ve disk tazeleme için kanalları da alır.
    let outbox_for_watcher = studio_outbox.clone();
    let data_model_for_watcher = data_model.clone();
    let vscode_for_watcher = app_state.tx_to_vscode.clone();
    let notify_for_watcher = disk_notify.clone();
    let ignore_for_watcher = cfg.ignore.clone();
    let cfg_for_watcher = cfg.clone();
    tokio::spawn(async move {
        file_sync::start_watcher(
            outbox_for_watcher,
            sync_dir,
            data_model_for_watcher,
            vscode_for_watcher,
            notify_for_watcher,
            ignore_for_watcher,
            (*cfg_for_watcher).clone(),
        )
        .await;
    });

    // 3. HTTP Transport Katmanını başlat (Studio ile haberleşir)
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        server::start_server(state_clone, listener).await;
    });

    // Kapanışta bayat port dosyası bırakma: aksi halde editör ölü bir porta bağlanmaya çalışır.
    {
        let cfg_for_exit = cfg.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cfg_for_exit.clear_port_file();
                info!("Syncix Core shut down.");
                std::process::exit(0);
            }
        });
    }

    let cfg_for_loop = cfg.clone();
    // Ayarlarin gercekten uygulandigi acilista goruinsun; "ayarladim ama olmadi"
    // durumunu tesis etmenin en ucuz yolu.
    tracing::info!(
        "Sync mode: {} | play: {} | debounce: {} ms | trash: {} (keep {}) | undo: {}",
        cfg.mod_.adi(),
        cfg.play.adi(),
        cfg.debounce_ms,
        cfg.guvenlik.cop_kutusu,
        cfg.guvenlik.cop_tur_sayisi,
        cfg.geri_al
    );
    layout::cop_ayarini_kur(cfg.guvenlik.cop_kutusu, cfg.guvenlik.cop_tur_sayisi);
    layout::meta_ayarini_kur(cfg.meta_dosyalari);

    // 4. Message Dispatcher (Event Bus'ı dinleyip yönlendirme yapar)
    // Production-Ready: Çekirdek transport'u bilmez, sadece kanaldan Payload okur.
    while let Some(payload) = rx_from_studio.recv().await {
        // Studio -> disk yonu kapaliysa Studio'dan gelen higbir degisiklik
        // modele islenmez. Tek istisna FULL_SYNC: disk_to_studio modunda bile
        // core'un Studio'daki UUID'leri bilmesi gerekiyor, yoksa hangi objeye
        // yazacagini bulamaz.
        if !cfg_for_loop.mod_.studiodan_kabul() && payload.event_type != EventType::FullSync {
            continue;
        }

        if payload.event_type == EventType::FullSync {
            // PLACE KIMLIGI KAPISI
            //
            // Bir sync klasoru TEK bir place'e aittir. Baska bir place ayni
            // klasore baglandiginda eskiden iki agac sessizce birlesiyordu:
            // servis UUID'leri butun place'lerde ayni oldugu icin ikisi ayni
            // iskelete oturuyor, StarterPlayerScripts gibi TEKIL objeler ikiser
            // tane oluyordu. Ustelik diskteki eski dosyalar "yeni obje" sanilip
            // yeni place'in icine yaratiliyordu.
            //
            // Artik birlestirmiyoruz: farkli bir place gelirse duruyoruz ve
            // karari kullaniciya birakiyoruz (syncix bind).
            let gelen_place = payload
                .data
                .get("place_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !gelen_place.is_empty() {
                match cfg_for_loop.bagli_place() {
                    // Klasor bos ya da ilk kez baglaniyor: sahiplen.
                    None => {
                        cfg_for_loop.place_bagla(&gelen_place);
                        tracing::info!("This folder is now bound to the connected place.");
                    }
                    Some(mevcut) if mevcut == gelen_place => {
                        // Ayni place, sorun yok.
                        if let Ok(mut c) = app_state.place_catismasi.lock() {
                            *c = None;
                        }
                    }
                    Some(mevcut) => {
                        let ad = payload
                            .data
                            .get("place_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let pid = payload
                            .data
                            .get("place_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string();

                        tracing::error!(
                            "This folder belongs to a different place. Sync is on hold so nothing gets mixed.
                               folder is bound to : {}
                               place connecting   : {} (\"{}\", id {})
                               Decide with: syncix bind --studio  (write this place into the folder)
                               or:          syncix bind --disk    (load the folder into this place)",
                            mevcut, gelen_place, ad, pid
                        );

                        if let Ok(mut c) = app_state.place_catismasi.lock() {
                            *c = Some(crate::server::PlaceCatismasi {
                                klasorun_place: mevcut,
                                gelen_place,
                                gelen_ad: ad,
                                gelen_place_id: pid,
                            });
                        }
                        // Modele DOKUNMA ve her yonu durdur: karar verilene
                        // kadar iki taraf da oldugu gibi kalmali.
                        crate::project::senkronu_askiya_al(true);
                        continue;
                    }
                }
            }

            tracing::info!("Received FULL_SYNC (bootstrap) from Studio. Synchronising state...");
            if let Some(instances) = payload.data.get("instances").and_then(|p| p.as_array()) {
                let mut added_count = 0;
                let mut ws_nodes = Vec::new();
                
                {
                    let mut dm = data_model.write().await;
                    // Recovery: FULL_SYNC geldiğinde eski state tamamen atılır.
                    // Böylece Studio tarafında silinmiş objeler bellekte kalmaz.
                    *dm = crate::model::DataModel::new();
                    for node_data in instances {
                        if let (Some(class_name), Some(name), Some(syncix_id)) = (
                            node_data.get("class_name").and_then(|v| v.as_str()),
                            node_data.get("name").and_then(|v| v.as_str()),
                            node_data.get("syncix_id").and_then(|v| v.as_str()),
                        ) {
                            // Kullanicinin disladigi siniflar modele hic girmez.
                            //
                            // Ayni suzgec eklentide de var; buradaki ikinci kapi
                            // eski bir eklenti baglandiginda ayarin yine de
                            // gecerli olmasi icin. Ayar iki taraftan birinde
                            // uygulanmazsa "disladim ama geliyor" durumu olusur.
                            if !cfg_for_loop.sinif_izinli(class_name) {
                                continue;
                            }
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let mut instance = InstanceNode::new(class_name, name);
                                instance.syncix_id = uuid;
                                
                                // Parent ID logic
                                if let Some(parent_str) = node_data.get("parent").and_then(|v| v.as_str()) {
                                    if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                        instance.parent = Some(parent_uuid);
                                    }
                                }

                                // Script kaynak kodu
                                if let Some(src) = node_data.get("source").and_then(|v| v.as_str()) {
                                    instance.source = Some(src.to_string());
                                }

                                // Attribute'lar
                                if let Some(attrs) = node_data.get("attributes").and_then(|v| v.as_object()) {
                                    for (k, val) in attrs {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.attributes.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // CollectionService etiketleri
                                if let Some(t) = node_data.get("tags").and_then(|v| v.as_array()) {
                                    instance.tags = t
                                        .iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect();
                                }

                                // Property'ler (geniş kapsam — generic properties objesi)
                                if let Some(props) = node_data.get("properties").and_then(|v| v.as_object()) {
                                    for (k, val) in props {
                                        if !cfg_for_loop.property_izinli(k) {
                                            continue;
                                        }
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.properties.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                if let Err(e) = dm.upsert_instance(instance.clone()) {
                                    tracing::warn!("FULL_SYNC upsert failed ({}): {}", instance.name, e);
                                    continue;
                                }
                                added_count += 1;
                                // Disk yazımı artık merkezi debounced yazıcı (layout) tarafından yapılır.

                                ws_nodes.push(serde_json::json!({
                                    "id": instance.syncix_id,
                                    "name": instance.name,
                                    "className": instance.class_name,
                                    "parentId": instance.parent.map(|u| u.to_string()),
                                    "childrenIds": [],
                                    "isExpanded": false
                                }));
                            }
                        }
                    }
                }
                
                tracing::info!("FULL_SYNC complete. {} instances added or updated.", added_count);
                
                // Notify VS Code with FULL_SYNC
                let ws_msg = serde_json::json!({
                    "event_type": "FULL_SYNC",
                    "data": {
                        "nodes": ws_nodes
                    }
                });
                let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
            }
        } else if payload.event_type == EventType::GetTree {
            tracing::info!("GET_TREE requested. Sending state...");
            let mut ws_nodes = Vec::new();
            {
                let dm = data_model.read().await;
                for instance in dm.get_all_instances().values() {
                    // İç kök "Game" (DataModel) düğümünü dışarı gönderme
                    if instance.class_name == "DataModel" {
                        continue;
                    }
                    ws_nodes.push(serde_json::json!({
                        "id": instance.syncix_id,
                        "name": instance.name,
                        "className": instance.class_name,
                        "parentId": instance.parent.map(|u| u.to_string()),
                        "childrenIds": [],
                        "isExpanded": false
                    }));
                }
            }
            
            let ws_msg = serde_json::json!({
                "event_type": "FULL_SYNC",
                "data": {
                    "nodes": ws_nodes
                }
            });
            let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
        } else if payload.event_type == EventType::RenameInstance {
            // VS Code -> Studio: Yeniden adlandırma
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = payload.data.get("newName").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };
            if new_name.is_empty() {
                tracing::warn!("RENAME_INSTANCE: newName was empty, ignored.");
            } else if let Some(uuid) = resolved {
                let mut updated_instance = None;
                let mut old_name = None;
                {
                    let mut dm = data_model.write().await;
                    if let Some(instance) = dm.get_mut_instance(&uuid) {
                        if instance.parent.is_none() {
                            tracing::warn!("RENAME_INSTANCE: services cannot be renamed ({}).", instance.name);
                            continue;
                        }
                        old_name = Some(instance.name.clone());
                        instance.name = new_name.to_string();
                        instance.last_updated = chrono::Utc::now().timestamp_millis();
                        updated_instance = Some(instance.clone());
                    }
                }
                if let Some(instance) = updated_instance {
                    let _ = &old_name;
                    // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                    // Studio'ya uygula
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "PROPERTY_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "property": "Name",
                                    "value": instance.name
                                }
                            }]
                        }),
                    });
                    // VS Code Explorer'a yansıt
                    let ws_msg = serde_json::json!({
                        "event_type": "INSTANCE_UPDATED",
                        "data": {
                            "id": instance.syncix_id,
                            "name": instance.name,
                            "className": instance.class_name,
                            "parentId": instance.parent.map(|u| u.to_string()),
                            "childrenIds": instance.children,
                            "isExpanded": false
                        }
                    });
                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    tracing::info!(
                        "VS Code RENAME işlendi: {} -> {}",
                        old_name.unwrap_or_default(),
                        instance.name
                    );
                } else {
                    tracing::warn!("RENAME_INSTANCE bilinmeyen UUID: {}", id);
                }
            } else {
                tracing::warn!("RENAME_INSTANCE target not found: {}", id);
            }
        } else if payload.event_type == EventType::CreateInstance {
            // VS Code -> Studio: Yeni instance yaratma.
            // UUID yalnızca burada, CREATE anında üretilir (Data Integrity kuralı).
            let class_name = payload.data.get("className").and_then(|v| v.as_str()).unwrap_or("");
            let parent_id = payload.data.get("parentId").and_then(|v| v.as_str()).unwrap_or("");
            // İsim verilmemişse sınıf adı kullanılır (eski davranış korunur).
            let node_name = payload
                .data
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(class_name);
            if class_name.is_empty() {
                tracing::warn!("CREATE_INSTANCE: className was empty, ignored.");
            } else {
                let mut instance = InstanceNode::new(class_name, node_name);
                // Istemci bir UUID verdiyse onu kullan; ice aktarma bu sayede
                // olusturdugu objeyi ismiyle degil kimligiyle hedefleyebiliyor.
                if let Some(verilen) = payload
                    .data
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                {
                    instance.syncix_id = verilen;
                }
                {
                    // Parent çözümlemesi: UUID veya isim (örn. "Workspace").
                    // Boşsa varsayılan olarak Workspace servisine bağlanır.
                    let dm = data_model.read().await;
                    let effective_parent = if parent_id.is_empty() { "Workspace" } else { parent_id };
                    instance.parent = resolve_id(&dm, effective_parent);
                }
                let uuid = instance.syncix_id;
                let insert_result = {
                    let mut dm = data_model.write().await;
                    dm.upsert_instance(instance.clone())
                };
                match insert_result {
                    Ok(()) => {
                        // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                        // Studio'ya CREATE gönder
                        studio_outbox.push(Payload {
                            version: "v1".to_string(),
                            event_type: EventType::CompositeUpdate,
                            data: serde_json::json!({
                                "patches": [{
                                    "event_type": "CREATE",
                                    "data": {
                                        "syncix_id": uuid,
                                        "class_name": instance.class_name,
                                        "name": instance.name,
                                        "parent": instance.parent.map(|u| u.to_string())
                                    }
                                }]
                            }),
                        });
                        // VS Code Explorer'a yansıt
                        let ws_msg = serde_json::json!({
                            "event_type": "INSTANCE_CREATED",
                            "data": {
                                "id": uuid,
                                "name": instance.name,
                                "className": instance.class_name,
                                "parentId": instance.parent.map(|u| u.to_string()),
                                "childrenIds": [],
                                "isExpanded": false
                            }
                        });
                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                        tracing::info!("CREATE handled: {} ({})", instance.name, instance.class_name);
                    }
                    Err(e) => tracing::warn!("CREATE_INSTANCE upsert failed: {}", e),
                }
            }
        } else if payload.event_type == EventType::DeleteInstance {
            // VS Code -> Studio: Silme
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };
            if let Some(uuid) = resolved {
                // Servis düğümleri silinemez
                let is_service = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
                };
                if is_service {
                    tracing::warn!("DELETE_INSTANCE: servisler silinemez ({}).", id);
                    continue;
                }
                let removed_instance = {
                    let mut dm = data_model.write().await;
                    dm.remove_instance(&uuid)
                };
                if let Some(instance) = removed_instance {
                    // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                    // Studio'ya DESTROY gönder
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "DESTROY",
                                "data": { "syncix_id": uuid }
                            }]
                        }),
                    });
                    // VS Code Explorer'a yansıt
                    let ws_msg = serde_json::json!({
                        "event_type": "INSTANCE_REMOVED",
                        "data": {
                            "id": uuid,
                            "parentId": instance.parent.map(|u| u.to_string())
                        }
                    });
                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    tracing::info!("DELETE handled: {}", instance.name);
                } else {
                    tracing::warn!("DELETE_INSTANCE bilinmeyen UUID: {}", id);
                }
            } else {
                tracing::warn!("DELETE_INSTANCE target not found: {}", id);
            }
        } else if payload.event_type == EventType::ReparentInstance {
            // VS Code/CLI -> Studio: Taşıma
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let new_parent_field = payload
                .data
                .get("newParentId")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let (resolved_id, resolved_parent) = {
                let dm = data_model.read().await;
                (resolve_id(&dm, id), resolve_id(&dm, new_parent_field))
            };

            if let (Some(uuid), Some(new_parent)) = (resolved_id, resolved_parent) {
                if uuid == new_parent {
                    tracing::warn!("REPARENT_INSTANCE: an instance cannot be moved into itself.");
                    continue;
                }
                // Servis düğümleri taşınamaz
                let is_service = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
                };
                if is_service {
                    tracing::warn!("REPARENT_INSTANCE: services cannot be moved ({}).", id);
                    continue;
                }

                let result = {
                    let mut dm = data_model.write().await;
                    dm.reparent(&uuid, Some(new_parent))
                };

                match result {
                    Ok((old_parent, _)) => {
                        let instance = {
                            let dm = data_model.read().await;
                            dm.get_instance(&uuid).cloned()
                        };
                        if let Some(instance) = instance {
                            // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                            // Studio'ya REPARENT gönder
                            studio_outbox.push(Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "REPARENT",
                                        "data": {
                                            "syncix_id": uuid,
                                            "parent": new_parent.to_string()
                                        }
                                    }]
                                }),
                            });
                            // VS Code Explorer'a yansıt
                            let ws_msg = serde_json::json!({
                                "event_type": "INSTANCE_MOVED",
                                "data": {
                                    "id": uuid,
                                    "oldParentId": old_parent.map(|u| u.to_string()),
                                    "newParentId": new_parent.to_string()
                                }
                            });
                            let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                            tracing::info!("REPARENT handled: {} -> {}", instance.name, new_parent);
                        }
                    }
                    Err(e) => tracing::warn!("REPARENT_INSTANCE failed: {}", e),
                }
            } else {
                tracing::warn!("REPARENT_INSTANCE target or parent not found (id={}, parent={})", id, new_parent_field);
            }
        } else if payload.event_type == EventType::SetProperty {
            // VS Code/CLI -> Studio: Bir özelliği ayarla
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let property = payload
                .data
                .get("property")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // Değer hem CLI'dan string ("0.5", "true", "1,2,3") hem de Inspector'dan
            // tiplenmiş ({"Color3":{...}}, sayı, bool) gelebilir.
            let value_json = payload
                .data
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let value_str = value_json
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| value_json.to_string());

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if property.is_empty() {
                tracing::warn!("SET_PROPERTY: the property name was empty.");
            } else if let Some(uuid) = resolved {
                // Terminalden gelen değer düz metindir; tip bilgisi taşımaz.
                // O yüzden önce property'nin MEVCUT değerinin tipine uydurulmaya
                // çalışılır: `set X BrickColor "Really red"` metin değil BrickColor,
                // `set X CFrame 0,10,0` metin değil CFrame olur.
                // Tip kararı yine değere göre veriliyor — sadece kıyaslanan değer
                // modelde zaten duran değer.
                let mevcut = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid)
                        .and_then(|i| i.properties.get(&property).cloned())
                };
                let mut pv = match &value_json {
                    serde_json::Value::String(s) => mevcut
                        .as_ref()
                        .and_then(|m| mevcut_tipe_uydur(m, s))
                        .unwrap_or_else(|| parse_property_value(s)),
                    other => parse_wire_value(other)
                        .unwrap_or_else(|| model::PropertyValue::String(value_str.clone())),
                };

                // Referans hedefi burada tam UUID'ye çözülür. Kullanıcı kısa
                // tutamaç ya da isim yazabiliyor ("syncix set door Part0 hinge"),
                // ama Studio'daki önbellek yalnızca tam UUID ile aranıyor;
                // çözülmeden gönderilirse referans sessizce nil kalırdı.
                if let model::PropertyValue::Ref(hedef) = &pv {
                    if !hedef.is_empty() {
                        let cozulen = {
                            let dm = data_model.read().await;
                            resolve_id(&dm, hedef)
                        };
                        match cozulen {
                            Some(u) => pv = model::PropertyValue::Ref(u.to_string()),
                            None => {
                                tracing::warn!(
                                    "SET_PROPERTY: reference target '{}' was not found; the property was left unchanged.",
                                    hedef
                                );
                                return;
                            }
                        }
                    }
                }
                let mut ok = false;
                {
                    let mut dm = data_model.write().await;
                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                        if property == "Name" {
                            inst.name = value_str.clone();
                        } else {
                            inst.properties.insert(property.clone(), pv.clone());
                        }
                        inst.last_updated = chrono::Utc::now().timestamp_millis();
                        ok = true;
                    }
                }

                if ok {
                    // Studio'ya uygula
                    let wire_value = if property == "Name" {
                        serde_json::json!(value_str)
                    } else {
                        pv_to_wire(&pv)
                    };
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "PROPERTY_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "property": property,
                                    "value": wire_value
                                }
                            }]
                        }),
                    });
                    // VS Code Explorer'a bildir (isim değişmiş olabilir)
                    let instance = {
                        let dm = data_model.read().await;
                        dm.get_instance(&uuid).cloned()
                    };
                    if let Some(instance) = instance {
                        let ws_msg = serde_json::json!({
                            "event_type": "INSTANCE_UPDATED",
                            "data": {
                                "id": instance.syncix_id,
                                "name": instance.name,
                                "className": instance.class_name,
                                "parentId": instance.parent.map(|u| u.to_string()),
                                "childrenIds": instance.children,
                                "isExpanded": false
                            }
                        });
                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    }
                    tracing::info!("SET_PROPERTY handled: {} .{} = {}", id, property, value_str);
                } else {
                    tracing::warn!("SET_PROPERTY bilinmeyen UUID: {}", id);
                }
            } else {
                tracing::warn!("SET_PROPERTY target not found: {}", id);
            }
        } else if payload.event_type == EventType::Selection {
            // Secim iki yonlu bir AYNA: Studio'da tiklanan obje editorde,
            // editorde tiklanan obje Studio'da secilir.
            //
            // Model'e yazilmiyor cunku secim projenin icerigi degil, anlik bir
            // durum. Diske yazilsaydi her tiklama bir dosya degisikligi olur,
            // surum kontrolu gurultuye bogulurdu.
            let kimlikler: Vec<String> = payload
                .data
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // Kaynak, mesajin geri donmesini onlemek icin tasiniyor: Studio'dan
            // gelen secimi Studio'ya geri gondermek sonsuz bir ping-pong olurdu.
            let kaynak = payload
                .data
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("studio");

            if kaynak == "studio" {
                let ws = serde_json::json!({
                    "event_type": "SELECTION",
                    "data": { "ids": kimlikler }
                });
                let _ = app_state.tx_to_vscode.send(ws.to_string());
            } else {
                // Editorden geldi: Studio'ya uygula.
                let cozulmus: Vec<String> = {
                    let dm = data_model.read().await;
                    kimlikler
                        .iter()
                        .filter_map(|h| resolve_id(&dm, h).map(|u| u.to_string()))
                        .collect()
                };
                studio_outbox.push(Payload {
                    version: "v1".to_string(),
                    event_type: EventType::CompositeUpdate,
                    data: serde_json::json!({
                        "patches": [{
                            "event_type": "SELECTION_UPDATE",
                            "data": { "ids": cozulmus }
                        }]
                    }),
                });
            }
        } else if payload.event_type == EventType::SetTags {
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let mut etiketler: Vec<String> = payload
                .data
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            // Siralanmis ve tekrarsiz: iki taraf ayni listeyi ayni sirada gorsun,
            // yoksa her karsilastirma "degisti" der ve gereksiz yama uretilir.
            etiketler.sort();
            etiketler.dedup();

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if let Some(uuid) = resolved {
                let uygulandi = {
                    let mut dm = data_model.write().await;
                    match dm.get_mut_instance(&uuid) {
                        Some(inst) => {
                            inst.tags = etiketler.clone();
                            inst.last_updated = chrono::Utc::now().timestamp_millis();
                            true
                        }
                        None => false,
                    }
                };
                if uygulandi {
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "TAGS_UPDATE",
                                "data": { "syncix_id": uuid, "tags": etiketler }
                            }]
                        }),
                    });
                    tracing::info!("SET_TAGS handled: {} -> {:?}", id, etiketler);
                }
            } else {
                tracing::warn!("SET_TAGS target not found: {}", id);
            }
        } else if payload.event_type == EventType::SetAttribute {
            // VS Code/CLI -> Studio: Bir Attribute ayarla
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = payload
                .data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // value alani JSON null ise bu bir SILME istegidir.
            // Studio tarafinda SetAttribute(ad, nil) attribute'u kaldirir; ayni
            // anlami tel uzerinde null ile tasiyoruz, ayri bir event tipi gerekmiyor.
            let silme = payload
                .data
                .get("value")
                .map(|v| v.is_null())
                .unwrap_or(true);
            let value_str = payload
                .data
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if name.is_empty() {
                tracing::warn!("SET_ATTRIBUTE: the attribute name was empty.");
            } else if let Some(uuid) = resolved {
                let pv = parse_property_value(&value_str);
                let mut ok = false;
                {
                    let mut dm = data_model.write().await;
                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                        if silme {
                            inst.attributes.remove(&name);
                        } else {
                            inst.attributes.insert(name.clone(), pv.clone());
                        }
                        inst.last_updated = chrono::Utc::now().timestamp_millis();
                        ok = true;
                    }
                }
                if ok {
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "ATTRIBUTE_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "name": name,
                                    "value": if silme { serde_json::Value::Null } else { pv_to_wire(&pv) }
                                }
                            }]
                        }),
                    });
                    tracing::info!("SET_ATTRIBUTE handled: {} @{} = {}", name, id, value_str);
                } else {
                    tracing::warn!("SET_ATTRIBUTE for an unknown UUID: {}", id);
                }
            } else {
                tracing::warn!("SET_ATTRIBUTE target not found: {}", id);
            }
        } else if payload.event_type == EventType::CompositeUpdate {
            if let Some(patches) = payload.data.get("patches").and_then(|p| p.as_array()) {
                for patch in patches {
                    let p_type = patch.get("event_type").and_then(|e| e.as_str()).unwrap_or("");
                    if p_type == "CREATE" {
                        // Extract basic info
                        if let (Some(_data), Some(class_name), Some(name), Some(syncix_id)) = (
                            patch.get("data"),
                            patch.get("data").and_then(|d| d.get("class_name")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("name")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                        ) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let mut instance = InstanceNode::new(class_name, name);
                                instance.syncix_id = uuid;

                                // Parent UUID (hiyerarşi için)
                                if let Some(parent_str) = patch
                                    .get("data")
                                    .and_then(|d| d.get("parent"))
                                    .and_then(|v| v.as_str())
                                {
                                    if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                        instance.parent = Some(parent_uuid);
                                    }
                                }

                                // Script kaynak kodu
                                if let Some(src) = patch
                                    .get("data")
                                    .and_then(|d| d.get("source"))
                                    .and_then(|v| v.as_str())
                                {
                                    instance.source = Some(src.to_string());
                                }

                                // Attribute'lar
                                if let Some(attrs) = patch
                                    .get("data")
                                    .and_then(|d| d.get("attributes"))
                                    .and_then(|v| v.as_object())
                                {
                                    for (k, val) in attrs {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.attributes.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // Property'ler (geniş kapsam)
                                if let Some(props) = patch
                                    .get("data")
                                    .and_then(|d| d.get("properties"))
                                    .and_then(|v| v.as_object())
                                {
                                    for (k, val) in props {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.properties.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // Save to DataModel
                                {
                                    let mut dm = data_model.write().await;
                                    if let Err(e) = dm.upsert_instance(instance.clone()) {
                                        tracing::warn!("CREATE upsert failed ({}): {}", instance.name, e);
                                    }
                                }
                                
                                // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.

                                // Notify VS Code (her sınıf için, serializer olsun olmasın)
                                let ws_msg = serde_json::json!({
                                    "event_type": "INSTANCE_CREATED",
                                    "data": {
                                        "id": instance.syncix_id,
                                        "name": instance.name,
                                        "className": instance.class_name,
                                        "parentId": instance.parent.map(|u| u.to_string()),
                                        "childrenIds": instance.children,
                                        "isExpanded": false
                                    }
                                });
                                let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                tracing::info!(
                                    "Studio CREATE işlendi: {} ({})",
                                    instance.name,
                                    instance.class_name
                                );
                            }
                        }
                    } else if p_type == "PROPERTY_UPDATE" {
                        if let (Some(_data), Some(syncix_id), Some(property), Some(value)) = (
                            patch.get("data"),
                            patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("property")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("value")),
                        ) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                // Echo döngüsü tespiti: aynı property kısa sürede defalarca
                                // gidip geliyorsa echo engelleme kaçırmış demektir. Eskiden bu
                                // tamamen sessizdi; artık hangi property olduğu loga düşer.
                                if health_monitor
                                    .loop_detector
                                    .kaydet(syncix_id, property)
                                {
                                    tracing::warn!(
                                        "Suspected echo loop: {} .{} was updated many times in a short window. Studio and the core may be writing the same value back and forth.",
                                        syncix_id,
                                        property
                                    );
                                }

                                let mut updated_instance = None;
                                let mut old_name = None;

                                {
                                    let mut dm = data_model.write().await;
                                    if let Some(instance) = dm.get_mut_instance(&uuid) {
                                        old_name = Some(instance.name.clone());
                                        
                                        if property == "Name" {
                                            if let Some(new_name) = value.as_str() {
                                                instance.name = new_name.to_string();
                                            }
                                        } else if property == "Source" {
                                            if let Some(src) = value.as_str() {
                                                instance.source = Some(src.to_string());
                                            }
                                        } else if let Some(pv) = parse_wire_value(value) {
                                            instance.properties.insert(property.to_string(), pv);
                                        }
                                        updated_instance = Some(instance.clone());
                                    }
                                }
                                
                                if let Some(instance) = updated_instance {
                                    let _ = &old_name;
                                    // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.

                                    // Notify VS Code (her sınıf için)
                                    let ws_msg = serde_json::json!({
                                        "event_type": "INSTANCE_UPDATED",
                                        "data": {
                                            "id": instance.syncix_id,
                                            "name": instance.name,
                                            "className": instance.class_name,
                                            "parentId": instance.parent.map(|u| u.to_string()),
                                            "childrenIds": instance.children,
                                            "isExpanded": false
                                        }
                                    });
                                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                    tracing::info!(
                                        "Studio PROPERTY_UPDATE işlendi: {} -> {}",
                                        instance.name,
                                        property
                                    );
                                } else {
                                    tracing::warn!(
                                        "PROPERTY_UPDATE ignored for an unknown UUID: {} (property: {}). FULL_SYNC may not have arrived yet.",
                                        uuid,
                                        property
                                    );
                                }
                            }
                        }
                    } else if p_type == "DESTROY" {
                        if let Some(syncix_id) = patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let removed_instance = {
                                    let mut dm = data_model.write().await;
                                    dm.remove_instance(&uuid)
                                };
                                
                                if let Some(instance) = removed_instance {
                                    // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                                    let ws_msg = serde_json::json!({
                                        "event_type": "INSTANCE_REMOVED",
                                        "data": {
                                            "id": uuid,
                                            "parentId": instance.parent.map(|u| u.to_string())
                                        }
                                    });
                                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                    tracing::info!("Studio DESTROY handled: {}", instance.name);
                                } else {
                                    tracing::warn!(
                                        "DESTROY bilinmeyen UUID için yok sayıldı: {}",
                                        uuid
                                    );
                                }
                            }
                        }
                    } else if p_type == "REPARENT" {
                        // Studio'da obje başka bir ebeveyne taşındı.
                        let syncix_id = patch
                            .get("data")
                            .and_then(|d| d.get("syncix_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let new_parent_str = patch
                            .get("data")
                            .and_then(|d| d.get("parent"))
                            .and_then(|v| v.as_str());

                        if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                            let new_parent = new_parent_str
                                .and_then(|s| uuid::Uuid::parse_str(s).ok());

                            let result = {
                                let mut dm = data_model.write().await;
                                dm.reparent(&uuid, new_parent)
                            };

                            match result {
                                Ok((old_parent, _)) => {
                                    let instance = {
                                        let dm = data_model.read().await;
                                        dm.get_instance(&uuid).cloned()
                                    };
                                    if let Some(instance) = instance {
                                        // Disk yazımı merkezi debounced yazıcı (layout) tarafından yapılır.
                                        // VS Code Explorer'a taşımayı bildir
                                        let ws_msg = serde_json::json!({
                                            "event_type": "INSTANCE_MOVED",
                                            "data": {
                                                "id": uuid,
                                                "oldParentId": old_parent.map(|u| u.to_string()),
                                                "newParentId": new_parent.map(|u| u.to_string())
                                            }
                                        });
                                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                        tracing::info!(
                                            "Studio REPARENT işlendi: {} -> parent {}",
                                            instance.name,
                                            new_parent.map(|u| u.to_string()).unwrap_or_else(|| "yok".to_string())
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("REPARENT failed ({}): {}", uuid, e);
                                }
                            }
                        }
                    } else if p_type == "ATTRIBUTE_UPDATE" {
                        // Studio'da bir attribute değişti.
                        let syncix_id = patch
                            .get("data")
                            .and_then(|d| d.get("syncix_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = patch
                            .get("data")
                            .and_then(|d| d.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let value = patch.get("data").and_then(|d| d.get("value"));

                        if let (Ok(uuid), false, Some(val)) =
                            (uuid::Uuid::parse_str(syncix_id), name.is_empty(), value)
                        {
                            if let Some(pv) = parse_wire_value(val) {
                                let mut applied = false;
                                {
                                    let mut dm = data_model.write().await;
                                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                                        inst.attributes.insert(name.to_string(), pv);
                                        applied = true;
                                    }
                                }
                                if applied {
                                    tracing::info!("Studio ATTRIBUTE_UPDATE handled: {} @{}", name, syncix_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Her olaydan sonra model değişmiş olabilir; diski (Studio Explorer aynası)
        // güncellemek için debounced yazıcıyı uyar.
        disk_notify.notify_one();
    }
}

// ===========================================================================
// PROPERTY TİP DÖNÜŞÜMÜ TESTLERİ
//
// Neden burası: projedeki iki gerçek hata da tam olarak bu katmandan geçti.
//   1. Position hatası — Vector3 değerler Studio'ya ulaşıyordu ama tel formatı
//      yanlış yorumlanınca sessizce düşüyordu, tüm objeler 0,0,0'da kaldı.
//   2. Renk hatası — "#5aa832" düz String olarak gidiyordu, Studio reddediyordu.
// İkisi de model testleriyle yakalanamazdı; ağaç mantığı kusursuzdu.
// Buradaki testler tel formatını KİLİTLER: biçim değişirse test kırılır.
// ===========================================================================
#[cfg(test)]
mod property_tests {
    use super::*;
    use model::PropertyValue;

    /// Tel formatı gidiş-dönüşü: pv -> wire -> pv aynı değeri vermeli.
    /// Bir tip bu döngüde kaybolursa senkron sessizce veri kaybeder.
    #[test]
    fn tel_formati_gidis_donus_tum_tipler() {
        let ornekler = vec![
            PropertyValue::String("Merhaba".into()),
            PropertyValue::Number(42.5),
            PropertyValue::Boolean(true),
            PropertyValue::Boolean(false),
            PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 },
            PropertyValue::Color3 { r: 0.35, g: 0.66, b: 0.2 },
            PropertyValue::UDim2 { xs: 0.5, xo: 10.0, ys: 1.0, yo: -4.0 },
        ];

        for ornek in ornekler {
            let wire = pv_to_wire(&ornek);
            let geri = parse_wire_value(&wire)
                .unwrap_or_else(|| panic!("tel degeri cozulemedi: {:?} -> {}", ornek, wire));
            assert_eq!(geri, ornek, "gidis-donus bozuldu: {}", wire);
        }
    }

    /// Vector3 tel üzerinde ASLA düz metin olmamalı.
    /// Düz metin gönderilirse Studio "Vector3 expected" diyerek reddeder ve
    /// obje 0,0,0'da kalır — Position hatasının tam olarak yaptığı şey.
    #[test]
    fn vector3_tel_uzerinde_tablo_olarak_gider() {
        let wire = pv_to_wire(&PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 });
        assert!(wire.is_object(), "Vector3 tablo olmali, duz deger degil: {}", wire);
        assert!(wire.get("Vector3").is_some(), "Vector3 anahtari bulunmali: {}", wire);
        assert_eq!(wire["Vector3"]["y"].as_f64().unwrap(), 2.0);
    }

    #[test]
    fn color3_tel_uzerinde_tablo_olarak_gider() {
        let wire = pv_to_wire(&PropertyValue::Color3 { r: 1.0, g: 0.0, b: 0.5 });
        assert!(wire.get("Color3").is_some(), "Color3 anahtari bulunmali: {}", wire);
    }

    /// CLI ve HTTP'den gelen metinlerin doğru tipe çevrilmesi.
    /// "0,0.5,-60" String kalırsa konum uygulanmaz; hatanın girdi tarafı budur.
    #[test]
    fn metin_vector3_olarak_cozulur() {
        assert_eq!(
            parse_property_value("0,0.5,-60"),
            PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 }
        );
        // Haritada gerçekten kullanılan negatif değerler
        assert_eq!(
            parse_property_value("-20, 0.5, -6"),
            PropertyValue::Vector3 { x: -20.0, y: 0.5, z: -6.0 }
        );
        // Sıfır vektörü Number olarak yorumlanmamalı
        assert_eq!(
            parse_property_value("0,0,0"),
            PropertyValue::Vector3 { x: 0.0, y: 0.0, z: 0.0 }
        );
    }

    /// Hex renk hatası: "#5aa832" String kalırsa Studio reddeder.
    #[test]
    fn hex_renk_color3_olarak_cozulur() {
        match parse_property_value("#5aa832") {
            PropertyValue::Color3 { r, g, b } => {
                assert_eq!((r * 255.0).round() as u8, 0x5a);
                assert_eq!((g * 255.0).round() as u8, 0xa8);
                assert_eq!((b * 255.0).round() as u8, 0x32);
            }
            other => panic!("hex renk Color3 olmaliydi, gelen: {:?}", other),
        }
    }

    /// Diyez olmadan hex sayılmaz: "abcdef" bir isim olabilir, "123456" bir sayıdır.
    /// Bu ayrım olmazsa isim alanları yanlışlıkla renge dönüşür.
    #[test]
    fn diyezsiz_metin_renge_donusmez() {
        assert_eq!(parse_property_value("abcdef"), PropertyValue::String("abcdef".into()));
        assert_eq!(parse_property_value("123456"), PropertyValue::Number(123456.0));
    }

    #[test]
    fn skaler_tipler_dogru_cozulur() {
        assert_eq!(parse_property_value("true"), PropertyValue::Boolean(true));
        assert_eq!(parse_property_value("False"), PropertyValue::Boolean(false));
        assert_eq!(parse_property_value("5"), PropertyValue::Number(5.0));
        assert_eq!(parse_property_value("-3.5"), PropertyValue::Number(-3.5));
        assert_eq!(parse_property_value("Kutu"), PropertyValue::String("Kutu".into()));
    }

    /// Enum değerleri metin olarak geçmeli; çözümlemeyi Studio tarafı yapar.
    /// Burada sayıya ya da başka bir tipe dönüşürse Material/Font gibi
    /// property'ler sessizce uygulanmaz.
    #[test]
    fn enum_metni_oldugu_gibi_gecer() {
        assert_eq!(
            parse_property_value("Enum.Material.Neon"),
            PropertyValue::String("Enum.Material.Neon".into())
        );
        let wire = pv_to_wire(&PropertyValue::String("Enum.Material.Neon".into()));
        assert_eq!(wire.as_str(), Some("Enum.Material.Neon"));
    }

    /// Studio'dan ham skaler de gelebilir, serde enum biçimi de.
    /// İkisi de kabul edilmezse Studio kaynaklı güncellemeler düşer.
    #[test]
    fn ham_ve_serde_bicimleri_kabul_edilir() {
        assert_eq!(
            parse_wire_value(&serde_json::json!(7)),
            Some(PropertyValue::Number(7.0))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!("metin")),
            Some(PropertyValue::String("metin".into()))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!(true)),
            Some(PropertyValue::Boolean(true))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!({"Number": 3.0})),
            Some(PropertyValue::Number(3.0))
        );
    }
}

#[cfg(test)]
mod tip_uydurma_testleri {
    use super::*;
    use model::PropertyValue as P;

    /// Asıl hata buydu: `set <part> BrickColor "Really red"` düz String üretiyor,
    /// Studio'da atama sessizce başarısız oluyordu.
    #[test]
    fn brickcolor_metni_brickcolor_kalir() {
        let mevcut = P::BrickColor("Medium stone grey".into());
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "Really red"),
            Some(P::BrickColor("Really red".into()))
        );
    }

    /// Üç sayı verildiğinde dönme korunmalı: kullanıcı sadece taşımak istiyor.
    #[test]
    fn cframe_uc_sayi_donmeyi_korur() {
        let rot = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let mevcut = P::CFrame {
            pos: [1.0, 2.0, 3.0],
            rot,
        };
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "10, 0, -5"),
            Some(P::CFrame {
                pos: [10.0, 0.0, -5.0],
                rot
            })
        );
    }

    #[test]
    fn udim2_dort_sayi() {
        let mevcut = P::UDim2 {
            xs: 0.0,
            xo: 0.0,
            ys: 0.0,
            yo: 0.0,
        };
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "0.5,10,0.25,-4"),
            Some(P::UDim2 {
                xs: 0.5,
                xo: 10.0,
                ys: 0.25,
                yo: -4.0
            })
        );
    }

    /// Metin property'sine sayı benzeri bir değer yazılabilmeli:
    /// genel ayrıştırıcı "5"i Number yapardı ve Studio metni reddederdi.
    #[test]
    fn metin_propertysi_sayiya_donusmez() {
        let mevcut = P::String("merhaba".into());
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "5"),
            Some(P::String("5".into()))
        );
    }

    /// Uydurulamayan girdi None dönmeli ki çağıran genel ayrıştırıcıya düşsün.
    #[test]
    fn bozuk_girdi_none_doner() {
        let mevcut = P::Vector2 { x: 0.0, y: 0.0 };
        assert_eq!(mevcut_tipe_uydur(&mevcut, "abc"), None);
        assert_eq!(mevcut_tipe_uydur(&mevcut, "1,2,3"), None);
    }

    #[test]
    fn color3_hem_hex_hem_ucluyu_kabul_eder() {
        let mevcut = P::Color3 {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "#ff0000"),
            Some(P::Color3 {
                r: 1.0,
                g: 0.0,
                b: 0.0
            })
        );
        assert_eq!(
            mevcut_tipe_uydur(&mevcut, "0,0.5,1"),
            Some(P::Color3 {
                r: 0.0,
                g: 0.5,
                b: 1.0
            })
        );
    }
}

#[cfg(test)]
mod tel_formati_testleri {
    use super::*;
    use model::{ColorKeypoint, NumberKeypoint, PropertyValue as P};

    /// Bir tip yalnızca yazılıp okunabildiğinde gerçekten taşınmış olur.
    /// Tek yönlü eklenen tipler "gönderildi ama karşı tarafta kayboldu"
    /// durumunun kaynağıydı.
    fn gidis_donus(pv: P) {
        let tel = pv_to_wire(&pv);
        let geri = parse_wire_value(&tel);
        assert_eq!(geri, Some(pv.clone()), "tel formatı: {:?}", tel);
    }

    #[test]
    fn asset_id_gidis_donus() {
        gidis_donus(P::Content("rbxassetid://123456".into()));
    }

    #[test]
    fn renk_egrisi_gidis_donus() {
        gidis_donus(P::ColorSequence(vec![
            ColorKeypoint { t: 0.0, r: 1.0, g: 0.0, b: 0.0 },
            ColorKeypoint { t: 1.0, r: 0.0, g: 0.0, b: 1.0 },
        ]));
    }

    /// Envelope Roblox'un rastgelelik payı; düşerse parçacık efekti düzleşir.
    #[test]
    fn sayi_egrisi_envelope_korunur() {
        let pv = P::NumberSequence(vec![
            NumberKeypoint { t: 0.0, v: 1.0, envelope: 0.25 },
            NumberKeypoint { t: 1.0, v: 0.0, envelope: 0.0 },
        ]);
        let geri = parse_wire_value(&pv_to_wire(&pv));
        match geri {
            Some(P::NumberSequence(k)) => assert_eq!(k[0].envelope, 0.25),
            other => panic!("beklenmeyen: {:?}", other),
        }
    }

    #[test]
    fn dikdortgen_ve_yazi_tipi_gidis_donus() {
        gidis_donus(P::Rect { min: [4.0, 4.0], max: [12.0, 12.0] });
        gidis_donus(P::Font {
            family: "rbxasset://fonts/families/SourceSansPro.json".into(),
            weight: "Enum.FontWeight.Bold".into(),
            style: "Enum.FontStyle.Normal".into(),
        });
    }

    #[test]
    fn fizik_ozellikleri_gidis_donus() {
        gidis_donus(P::PhysicalProperties {
            density: 0.7,
            friction: 0.3,
            elasticity: 0.5,
            friction_weight: 1.0,
            elasticity_weight: 2.0,
        });
    }

    /// Studio'ya giden değer serde'nin etiketli biçiminde OLMAMALI.
    ///
    /// Bu bir regresyon testi: disk tarafı bir süre `PropertyValue`'yu doğrudan
    /// JSON'a koyuyordu, serde de onu {"Number":0.5} diye yazıyordu. Eklenti düz
    /// biçimi beklediği için "unsupported table value for property: Transparency"
    /// diyerek reddediyordu — canlı kullanımda yakalandı.
    #[test]
    fn tel_formati_serde_etiketi_uretmez() {
        for pv in [
            P::Number(0.5),
            P::String("merhaba".into()),
            P::Boolean(true),
        ] {
            let tel = pv_to_wire(&pv);
            assert!(
                !tel.is_object(),
                "ilkel değer düz gitmeli, tablo değil: {:?} -> {}",
                pv,
                tel
            );
            // Serde'nin hali gerçekten farklı olmalı; test kendi varsayımını doğrulasın.
            let serde_hali = serde_json::to_value(&pv).unwrap();
            assert!(serde_hali.is_object(), "serde etiketli yazmalı: {}", serde_hali);
            assert_ne!(tel, serde_hali);
        }
    }

    /// Daha önce eklenen tipler de kırılmamalı: yeni dallar sıralı if/else
    /// zincirine giriyor ve yanlış sırada eklenen bir dal öncekini gölgeleyebilir.
    #[test]
    fn onceki_tipler_hala_calisiyor() {
        gidis_donus(P::BrickColor("Really red".into()));
        gidis_donus(P::Ref("11111111-2222-4333-8444-555555555555".into()));
        gidis_donus(P::CFrame {
            pos: [1.0, 2.0, 3.0],
            rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        });
        gidis_donus(P::NumberRange { min: 1.0, max: 5.0 });
        gidis_donus(P::UDim { scale: 0.5, offset: 10.0 });
        gidis_donus(P::Vector2 { x: 1.0, y: 2.0 });
    }
}
