use axum::response::IntoResponse;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::{stream::StreamExt, SinkExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::health::{health_handler, HealthMonitor};
use crate::transport::{EventType, Payload, StudioOutbox};
use rand::Rng;

/// Sunucu durumu. Kanalları (channels) barındırır.
pub struct AppState {
    pub studio_outbox: Arc<StudioOutbox>,
    pub tx_to_core: tokio::sync::mpsc::Sender<Payload>,
    pub tx_to_vscode: broadcast::Sender<String>,
    pub health_monitor: Arc<HealthMonitor>,
    pub data_model: crate::model::SharedDataModel,
    pub chaos_mode_enabled: bool, // Failure Injection feature flag
    /// Proje kimliği: /health ile dışarı verilir, Studio eklentisi hangi projeye
    /// bağlandığını kullanıcıya gösterebilsin diye gerekli.
    pub project: Arc<crate::project::ProjectConfig>,
    /// Gerçekte bağlanılan port (istenen port dolu olabilir).
    pub actual_port: u16,
    /// Bu klasore BASKA bir place baglanmaya calistiysa burada durur.
    ///
    /// Bir sync klasoru single bir place'e aittir. Baska bir place baglandiginda
    /// iki tree sessizce birlestiriliyordu: service_name UUID'leri butun place'lerde
    /// is_same oldugu icin StarterPlayerScripts gibi TEKIL objeler ikiser tane
    /// oluyordu. Artik birlestirmek yerine duruyoruz ve karari kullaniciya
    /// birakiyoruz.
    pub place_clash_state: Arc<std::sync::Mutex<Option<PlaceConflict>>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlaceConflict {
    /// Klasorun is_bound oldugu place.
    pub folder_place: String,
    /// Baglanmaya calisan place.
    pub incoming_place: String,
    pub incoming_name: String,
    pub incoming_place_id: String,
}

/// İstenen porta bağlanmayı dener, doluysa sıradakileri dener.
///
/// Eskiden port sabit 8080'di: ikinci bir proje açıldığında ya da 8080'i başka bir
/// program tuttuğunda core sessizce çöküyor, kullanıcı sebebini göremiyordu.
pub fn bind_with_fallback(
    cfg: &crate::project::ProjectConfig,
) -> Option<(std::net::TcpListener, u16)> {
    let first_item = cfg.wanted_port;
    // Port açıkça istendiyse devretme yok: yalnızca o port denenir.
    let upper = if cfg.port_fixed {
        first_item.saturating_add(1)
    } else {
        first_item.saturating_add(crate::project::PORT_SCAN_SPAN)
    };
    for port in first_item..upper {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match std::net::TcpListener::bind(addr) {
            Ok(listener) => {
                if port != first_item {
                    warn!(
                        "Port {} is taken, using {} instead. The Studio plugin scans this range and will find it.",
                        first_item, port
                    );
                }
                return Some((listener, port));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => {
                error!("Could not bind port {}: {}", port, e);
                continue;
            }
        }
    }
    if cfg.port_fixed {
        error!(
            "Port {} is taken. It was requested explicitly, so no fallback was used. Free that port or pick another: syncix serve <port>",
            first_item
        );
    } else {
        error!(
            "All ports in the range {}-{} are taken. Change the 'port' value in syncix.toml.",
            first_item,
            upper - 1
        );
    }
    None
}

/// Dış istemcilerden (VS Code RPC, CLI) received command_name adlarını çekirdek event'lerine çevirir.
fn map_command(event_type: &str) -> Option<EventType> {
    match event_type {
        "GET_TREE" => Some(EventType::GetTree),
        "CREATE_INSTANCE" => Some(EventType::CreateInstance),
        "RENAME_INSTANCE" => Some(EventType::RenameInstance),
        "DELETE_INSTANCE" => Some(EventType::DeleteInstance),
        "REPARENT_INSTANCE" => Some(EventType::ReparentInstance),
        "SET_PROPERTY" => Some(EventType::SetProperty),
        "SET_ATTRIBUTE" => Some(EventType::SetAttribute),
        "SET_TAGS" => Some(EventType::SetTags),
        "SELECTION" => Some(EventType::Selection),
        _ => None,
    }
}

/// HTTP Sunucusunu başlatır. Roblox Studio eklentisi ve VS Code uzantısı bu sunucu ile konuşur.
/// Dinleyici dışarıda açılır: gerçek portun AppState'e girmesi gerektiği için
/// bağlanma işi start_server'dan önce yapılmak zorunda.
pub async fn start_server(state: Arc<AppState>, listener: std::net::TcpListener) {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/sync/poll", get(poll_handler))
        .route("/sync/push", post(push_handler))
        .route("/rpc", get(rpc_ws_handler))
        .route("/command", post(command_handler))
        .route("/tree", get(tree_handler))
        .route("/object", get(object_handler))
        .route("/verify", get(verify_handler))
        .route("/shutdown", post(shutdown_handler))
        .route("/sourcemap", get(sourcemap_handler))
        .route("/build", get(build_handler))
        .with_state(state.clone());

    info!(
        "Syncix transport and RPC layer started: http://127.0.0.1:{} (project: {})",
        state.actual_port, state.project.name
    );

    if let Err(e) = axum::Server::from_tcp(listener)
        .expect("could not take over the listener")
        .serve(app.into_make_service())
        .await
    {
        error!("HTTP server stopped: {}", e);
    }
}

/// Düzgün kapanma. `syncix down` bunu çağırır.
///
/// Eskiden core'u durdurmanın single yolu süreç adından öldürmekti (taskkill /IM),
/// bu da diğer projelerin core'unu da kapatıyordu ve yalnızca Windows'ta çalışıyordu.
/// Sunucu 127.0.0.1'e bağlı olduğu için bu uç nokta yalnızca yerel makineden erişilebilir.
async fn shutdown_handler(State(state): State<Arc<AppState>>) -> axum::response::Response {
    info!("Shutdown requested, Syncix Core is stopping.");
    state.project.clear_port_file();
    tokio::spawn(async {
        // Cevabın istemciye ulaşması için kısa bir gecikme.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        std::process::exit(0);
    });
    (axum::http::StatusCode::OK, "OK").into_response()
}

/// sourcemap.json içeriğini döndürür (luau-lsp uyumlu).
async fn sourcemap_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dm = state.data_model.read().await;
    let file_content = crate::sourcemap::json(&dm, &state.project.sync_dir, &state.project.root);
    ([(axum::http::header::CONTENT_TYPE, "application/json")], file_content)
}

/// Ağacı Roblox XML (.rbxmx / .rbxlx) olarak döndürür.
/// `?target=<hedef>` verilirse yalnızca o sub ağaç yazılır.
async fn build_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    use crate::model::ResolveResult;
    let dm = state.data_model.read().await;

    let root_dir = match q.get("target").filter(|t| !t.is_empty()) {
        None => None,
        Some(t) => match dm.resolve_target(t) {
            ResolveResult::One(u) => Some(u),
            ResolveResult::NotFound => {
                return (
                    axum::http::StatusCode::NOT_FOUND,
                    format!("Target not found: {}", t),
                )
                    .into_response()
            }
            ResolveResult::Ambiguous(c) => {
                return (
                    axum::http::StatusCode::CONFLICT,
                    format!("'{}' is ambiguous: {} matches.", t, c.len()),
                )
                    .into_response()
            }
        },
    };

    let (xml, skipped) = crate::rbxmx::export_rbxmx(&dm, root_dir.as_ref());
    if skipped > 0 {
        warn!("build: skipped {} unrecognised enum value(s).", skipped);
    }
    let mut reply = xml.into_response();
    let headers = reply.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/xml"),
    );
    // Atlanan Enum sayısı başlıkla bildirilir ki CLI kullanıcıyı uyarabilsin.
    headers.insert(
        axum::http::HeaderName::from_static("x-syncix-skipped-enums"),
        axum::http::HeaderValue::from_str(&skipped.to_string())
            .unwrap_or(axum::http::HeaderValue::from_static("0")),
    );
    reply
}

async fn poll_handler(State(state): State<Arc<AppState>>) -> Json<Option<Payload>> {
    state.health_monitor.inc_messages();
    // Poll gelmesi Studio'nun ayakta olduğunun single kanıtı; bağlantı durumunu buradan biliyoruz.
    state.health_monitor.touch_studio();

    if state.chaos_mode_enabled {
        let should_delay = {
            let mut rng = rand::thread_rng();
            rng.gen_bool(0.1)
        };
        if should_delay {
            let delay = { rand::thread_rng().gen_range(500..2000) };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            warn!("Chaos Engineering: Network Latency Injected (Poll)");
        }
    }

    // Long-poll: kuyrukta message varsa hemen döner, yoksa 10 saniyeye kadar bekler.
    // 10 saniye sınırı önemli: Studio'nun HTTP istemcisi 30 saniyede timeout'a düşer ve
    // bağlantı kopmuş sanılarak gereksiz reconnect + FULL_SYNC döngüsü oluşur.
    // Outbox kuyruğu sayesinde iki poll arasında gönderilen mesajlar kaybolmaz.
    let message = state
        .studio_outbox
        .pop_or_wait(std::time::Duration::from_secs(10))
        .await;
    if message.is_some() {
        state.health_monitor.inc_outbound();
    }
    Json(message)
}

async fn push_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Payload>,
) -> axum::response::Response {
    state.health_monitor.inc_messages();

    if state.chaos_mode_enabled {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.05) {
            // 5% şansla Packet Drop (Crash Simulation)
            warn!("Chaos Engineering: Payload Dropped (Push)");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Simulated Failure",
            )
                .into_response();
        }
    }

    state.health_monitor.touch_studio();

    // Eklentinin BatchQueue sayaçları. Birleştirmenin gerçekten çalışıp çalışmadığı
    // ancak bu iki sayı ile ölçülebilir; elle sürükleyerek doğrulamaya gerek kalmaz.
    if payload.event_type == EventType::PluginMetrics {
        let queued = payload
            .data
            .get("queued")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as usize;
        let coalesced = payload
            .data
            .get("coalesced")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as usize;
        state.health_monitor.set_plugin_metrics(queued, coalesced);

        let number_value = |item_name: &str| {
            payload
                .data
                .get(item_name)
                .and_then(|x| x.as_u64())
                .unwrap_or(0) as usize
        };
        // Eklenti sürümü denetimi.
        //
        // Sürüm kapısı şimdiye kadar yalnızca eklentinin kendisindeydi. Sürüm
        // kontrolü YAPMAYAN previous_text bir eklenti bağlanırsa core sessizce kabul
        // ediyordu ve uyumsuzluk garip davranış olarak ortaya çıkıyordu.
        if let Some(plugin_version) = payload.data.get("plugin_version").and_then(|x| x.as_str()) {
            if !crate::project::versions_compatible(plugin_version, crate::project::VERSION) {
                warn!(
                    "Version mismatch: Studio plugin {}, core {}. The same major.minor is required; update the plugin.",
                    plugin_version,
                    crate::project::VERSION
                );
            }
        }

        state.health_monitor.set_activity(
            number_value("activity_total"),
            number_value("activity_in"),
            number_value("activity_out"),
            number_value("conflicts"),
        );
        return (axum::http::StatusCode::OK, "OK").into_response();
    }

    // Bu liste, eklentiden GELEN mesajlarin gecebildigi single kapi. Listede
    // olmayan bir message sessizce dusuyor — fresh bir message tipi eklerken buraya
    // eklemeyi unutmak, "gonderiyorum ama hicbir sey olmuyor" demek.
    if payload.event_type == EventType::ClientUpdate
        || payload.event_type == EventType::CompositeUpdate
        || payload.event_type == EventType::FullSync
        || payload.event_type == EventType::Selection
    {
        state.health_monitor.inc_inbound();
        if let Err(e) = state.tx_to_core.send(payload).await {
            error!("Could not forward message to the core: {}", e);
        }
    }
    (axum::http::StatusCode::OK, "OK").into_response()
}

/// CLI ve diğer HTTP istemcileri için command_name endpoint'i.
/// Body: { "event_type": "CREATE_INSTANCE" | "RENAME_INSTANCE" | "DELETE_INSTANCE" | "GET_TREE", "data": {...} }
async fn command_handler(
    State(state): State<Arc<AppState>>,
    Json(v): Json<serde_json::Value>,
) -> axum::response::Response {
    use crate::model::ResolveResult;

    let event_type = v.get("event_type").and_then(|e| e.as_str()).unwrap_or("");

    // FULL_SYNC / PULL: Studio'ya "ağacı yeniden gönder" isteği.
    // Bu command_name çekirdeğe DEĞİL doğrudan Studio'ya gider; cevabı normal FULL_SYNC
    // yolundan işlenir ve modeli sıfırdan kurar.
    // BIND: place catismasini cozer. Iki secenek exists_flag ve ikisi de veri kaybettirir,
    // o yuzden karar kullanicinin.
    //   studio -> bu place dogru kabul edilir, folder_path onun uzerine yazilir
    //   disk   -> folder_path dogru kabul edilir, icerigi bu place'e yuklenir
    if event_type == "BIND" {
        let direction = v
            .get("data")
            .and_then(|d| d.get("side"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let place_clash = state.place_clash_state.lock().ok().and_then(|c| c.clone());
        let Some(c) = place_clash else {
            return (axum::http::StatusCode::BAD_REQUEST, "There is no place conflict to resolve.")
                .into_response();
        };

        match direction {
            "studio" => {
                // Klasoru received place'e bagla ve senkronu ac. Bir sonraki
                // FULL_SYNC'te tree Studio'dan yeniden kurulur; uzlastirici
                // klasoru ona uydurur (silinenler cop kutusuna gider).
                state.project.bind_place(&c.incoming_place);
                crate::project::set_sync_suspended(false);
                if let Ok(mut g) = state.place_clash_state.lock() {
                    *g = None;
                }
                state.studio_outbox.push(Payload {
                    version: "v1".to_string(),
                    event_type: EventType::FullSyncRequest,
                    data: serde_json::json!({}),
                });
                state.health_monitor.inc_outbound();
                return (axum::http::StatusCode::OK, "OK").into_response();
            }
            "disk" => {
                // Klasor dogru kabul edildi: kimligi received place'e ceviriyoruz
                // ki bundan sonra is_same sey again place_clash saymasin, ama agaci
                // Studio'dan ISTEMIYORUZ; diskteki file_list izleyici uzerinden
                // Studio'ya akacak.
                state.project.bind_place(&c.incoming_place);
                crate::project::set_sync_suspended(false);
                if let Ok(mut g) = state.place_clash_state.lock() {
                    *g = None;
                }
                return (axum::http::StatusCode::OK, "OK").into_response();
            }
            _ => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    "side must be 'studio' or 'disk'",
                )
                    .into_response();
            }
        }
    }

    if event_type == "FULL_SYNC" || event_type == "PULL" {
        state.studio_outbox.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::FullSyncRequest,
            data: serde_json::json!({}),
        });
        state.health_monitor.inc_outbound();
        return (axum::http::StatusCode::OK, "OK").into_response();
    }

    let Some(core_event) = map_command(event_type) else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Unknown event_type",
        )
            .into_response();
    };

    let mut data = v.get("data").cloned().unwrap_or(serde_json::json!({}));
    if !data.is_object() {
        return (axum::http::StatusCode::BAD_REQUEST, "data must be an object").into_response();
    }

    // CREATE_INSTANCE: UUID'yi BURADA uretip cevapta donduruyoruz.
    // Boylece cagiran, olusturdugu objeyi ismiyle degil kimligiyle hedefleyebilir.
    let mut generated_id: Option<String> = None;
    if core_event == EventType::CreateInstance {
        let id = data
            .get("id")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        data["id"] = serde_json::json!(id);
        generated_id = Some(id);
    }

    // Hedef doğrulama burada syncing yapılır ki CLI'ya anlamlı sonuç dönebilelim.
    // Çözümlenen UUID data'ya restored_count yazılır; çekirdek her zaman kesin UUID alır.
    match core_event {
        EventType::RenameInstance
        | EventType::DeleteInstance
        | EventType::SetProperty
        | EventType::SetAttribute
        | EventType::SetTags => {
            let target = data
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if target.is_empty() {
                return (axum::http::StatusCode::BAD_REQUEST, "the id field is required").into_response();
            }
            let dm = state.data_model.read().await;
            match dm.resolve_target(&target) {
                ResolveResult::One(u) => {
                    data["id"] = serde_json::json!(u.to_string());
                }
                ResolveResult::NotFound => {
                    return (
                        axum::http::StatusCode::NOT_FOUND,
                        format!("Target not found: {}", target),
                    )
                        .into_response();
                }
                ResolveResult::Ambiguous(candidates) => {
                    let list = candidates
                        .iter()
                        .map(|(name, class_name, id)| {
                            format!("  {} ({}) -> {}", name, class_name, &id.to_string()[0..8])
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    return (
                        axum::http::StatusCode::CONFLICT,
                        format!(
                            "'{}' is ambiguous: {} matches. Target it with the short UUID:\n{}",
                            target,
                            candidates.len(),
                            list
                        ),
                    )
                        .into_response();
                }
            }
        }
        EventType::CreateInstance => {
            let parent = data
                .get("parentId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !parent.is_empty() {
                let dm = state.data_model.read().await;
                match dm.resolve_target(&parent) {
                    ResolveResult::One(u) => {
                        data["parentId"] = serde_json::json!(u.to_string());
                    }
                    ResolveResult::NotFound => {
                        return (
                            axum::http::StatusCode::NOT_FOUND,
                            format!("Parent not found: {}", parent),
                        )
                            .into_response();
                    }
                    ResolveResult::Ambiguous(candidates) => {
                        let list = candidates
                            .iter()
                            .map(|(name, class_name, id)| {
                                format!("  {} ({}) -> {}", name, class_name, &id.to_string()[0..8])
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        return (
                            axum::http::StatusCode::CONFLICT,
                            format!(
                                "Parent '{}' belirsiz: {} eşleşme var. Kısa UUID kullanın:\n{}",
                                parent,
                                candidates.len(),
                                list
                            ),
                        )
                            .into_response();
                    }
                }
            }
        }
        EventType::ReparentInstance => {
            // Hem taşınacak obje (id) hem de fresh parent_ref (newParentId) çözümlenir.
            let dm = state.data_model.read().await;
            for (field, label) in [("id", "Target"), ("newParentId", "New parent")] {
                let target = data
                    .get(field)
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if target.is_empty() {
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        format!("the {} ({}) field is required", label, field),
                    )
                        .into_response();
                }
                match dm.resolve_target(&target) {
                    ResolveResult::One(u) => {
                        data[field] = serde_json::json!(u.to_string());
                    }
                    ResolveResult::NotFound => {
                        return (
                            axum::http::StatusCode::NOT_FOUND,
                            format!("{} not found: {}", label, target),
                        )
                            .into_response();
                    }
                    ResolveResult::Ambiguous(candidates) => {
                        let list = candidates
                            .iter()
                            .map(|(name, class_name, id)| {
                                format!("  {} ({}) -> {}", name, class_name, &id.to_string()[0..8])
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        return (
                            axum::http::StatusCode::CONFLICT,
                            format!(
                                "{} '{}' belirsiz: {} eşleşme var. Kısa UUID kullanın:\n{}",
                                label,
                                target,
                                candidates.len(),
                                list
                            ),
                        )
                            .into_response();
                    }
                }
            }
        }
        _ => {}
    }

    let payload = Payload {
        version: "v1".to_string(),
        event_type: core_event,
        data,
    };
    if let Err(e) = state.tx_to_core.send(payload).await {
        error!("Could not forward CLI command to the core ({}): {}", event_type, e);
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Core'a iletilemedi",
        )
            .into_response();
    }
    info!("CLI command received: {}", event_type);
    match generated_id {
        Some(id) => (axum::http::StatusCode::OK, id).into_response(),
        None => (axum::http::StatusCode::OK, "OK").into_response(),
    }
}

/// Tek bir objenin tam verisini (özellikler dahil) döndürür (CLI 'props' için).
/// ?target=<isim|uuid|kısa-uuid>
async fn object_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    use crate::model::ResolveResult;
    let target = params.get("target").cloned().unwrap_or_default();
    let dm = state.data_model.read().await;
    match dm.resolve_target(&target) {
        ResolveResult::One(uuid) => {
            if let Some(node) = dm.get_instance(&uuid) {
                Json(serde_json::to_value(node).unwrap_or(serde_json::json!(null)))
            } else {
                Json(serde_json::json!({ "error": "bulunamadı" }))
            }
        }
        ResolveResult::NotFound => Json(serde_json::json!({ "error": format!("Target not found: {}", target) })),
        ResolveResult::Ambiguous(c) => Json(serde_json::json!({
            "error": format!("'{}' is ambiguous: {} matches", target, c.len()),
            "candidates": c.iter().map(|(n, cl, id)| serde_json::json!({
                "name": n, "className": cl, "shortId": id.to_string()[0..8].to_string()
            })).collect::<Vec<_>>()
        })),
    }
}

/// Model bütünlüğünü denetler (CLI 'check' için).
/// Öksüz parent/child referanslarını ve sınıf dağılımını raporlar.
async fn verify_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let dm = state.data_model.read().await;
    let instances = dm.get_all_instances();

    let mut by_class: std::collections::BTreeMap<String, usize> = Default::default();
    let mut scripts_with_source = 0usize;
    let mut roots = 0usize;
    for n in instances.values() {
        if n.class_name == "DataModel" {
            continue;
        }
        *by_class.entry(n.class_name.clone()).or_insert(0) += 1;
        if n.parent.is_none() {
            roots += 1;
        }
        if matches!(n.class_name.as_str(), "Script" | "LocalScript" | "ModuleScript")
            && n.source.is_some()
        {
            scripts_with_source += 1;
        }
    }

    let (ok, errors) = match dm.verify_consistency() {
        Ok(()) => (true, Vec::new()),
        Err(list) => (false, list),
    };

    Json(serde_json::json!({
        "ok": ok,
        "errors": errors,
        "totalObjects": instances.len().saturating_sub(1), // iç kök hariç
        "rootServices": roots,
        "scriptsWithSource": scripts_with_source,
        "byClass": by_class,
    }))
}

/// Mevcut DataModel ağacını JSON olarak döndürür (CLI 'list' ve debugging için).
async fn tree_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let dm = state.data_model.read().await;
    let mut nodes = Vec::new();
    for instance in dm.get_all_instances().values() {
        if instance.class_name == "DataModel" {
            continue;
        }
        nodes.push(serde_json::json!({
            "id": instance.syncix_id,
            "name": instance.name,
            "className": instance.class_name,
            "parentId": instance.parent
        }));
    }
    Json(serde_json::json!(nodes))
}

/// VS Code Extension için WebSocket tabanlı RPC Handler
async fn rpc_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    ws.on_upgrade(|socket| handle_rpc_socket(socket, state))
}

async fn handle_rpc_socket(socket: WebSocket, state: Arc<AppState>) {
    state.health_monitor.add_connection();
    let (mut sender, mut receiver) = socket.split();
    let mut rx_from_core = state.tx_to_vscode.subscribe();

    info!(
        "VS Code RPC client connected. Active connections: {}",
        state
            .health_monitor
            .get_status(&state.project, state.actual_port)
            .active_connections
    );

    // Core'dan VS Code'a (Push Events)
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx_from_core.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let tx_vscode_in = state.tx_to_vscode.clone();
    let _tx_to_core = state.tx_to_core.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            info!("RPC command received: {}", text);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(req_id) = v.get("request_id") {
                    // Send ACK to resolve the Promise and avoid RPC Timeout
                    let ack = serde_json::json!({
                        "request_id": req_id,
                        "data": { "status": "ok" }
                    });
                    let _ = tx_vscode_in.send(ack.to_string());

                    let event_type = v.get("event_type").and_then(|e| e.as_str()).unwrap_or("");
                    if let Some(core_event) = map_command(event_type) {
                        let payload = Payload {
                            version: "v1".to_string(),
                            event_type: core_event,
                            data: v.get("data").cloned().unwrap_or(serde_json::json!({})),
                        };
                        if let Err(e) = _tx_to_core.send(payload).await {
                            error!("Could not forward command to the core ({}): {}", event_type, e);
                        }
                    }
                }
            }
        }
    });

    // Herhangi bir bağlantı koptuğunda ikisini de kapat
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    state.health_monitor.remove_connection();
    warn!(
        "VS Code RPC İstemcisi koptu. Kalan Aktif: {}",
        state
            .health_monitor
            .get_status(&state.project, state.actual_port)
            .active_connections
    );
}
