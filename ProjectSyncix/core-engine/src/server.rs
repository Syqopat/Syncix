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

/// Server state. Holds the channels.
pub struct AppState {
    pub studio_outbox: Arc<StudioOutbox>,
    pub tx_to_core: tokio::sync::mpsc::Sender<Payload>,
    pub tx_to_vscode: broadcast::Sender<String>,
    pub health_monitor: Arc<HealthMonitor>,
    pub data_model: crate::model::SharedDataModel,
    pub chaos_mode_enabled: bool, // Failure Injection feature flag
    /// Project identity: exposed through /health so the Studio plugin can show the user
    /// which project it connected to.
    pub project: Arc<crate::project::ProjectConfig>,
    /// The port actually bound (the requested one may be taken).
    pub actual_port: u16,
    /// Set when ANOTHER place tried to connect to this folder.
    ///
    /// A sync folder belongs to one place. When another place connected, the
    /// two trees were merged silently: service UUIDs are the same in every place,
    /// so SINGLETON objects such as StarterPlayerScripts ended up
    /// duplicated. Now, instead of merging, we stop and leave the decision to the
    /// user.
    pub place_clash_state: Arc<std::sync::Mutex<Option<PlaceConflict>>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlaceConflict {
    /// The place this folder is bound to.
    pub folder_place: String,
    /// Place attempting to connect.
    pub incoming_place: String,
    pub incoming_name: String,
    pub incoming_place_id: String,
}

/// Tries to bind the requested port; if it is taken, tries the following ones.
///
/// The port used to be fixed at 8080: when a second project was opened or another
/// program held 8080, the core silently crashed and the user could not see why.
pub fn bind_with_fallback(
    cfg: &crate::project::ProjectConfig,
) -> Option<(std::net::TcpListener, u16)> {
    let first_item = cfg.wanted_port;
    // If the port was requested explicitly there is no fallback: only that port is tried.
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

/// Maps command names from external clients (VS Code RPC, CLI) to core events.
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

/// Starts the HTTP server. The Roblox Studio plugin and the VS Code extension talk to it.
/// The listener is opened outside: the real port has to go into AppState, so
/// binding has to happen before start_server.
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
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 250)) // 250MB limit
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

/// Clean shutdown. `syncix down` calls this.
///
/// The only way to stop the core used to be killing it by process name (taskkill /IM),
/// which also killed other projects' cores and only worked on Windows.
/// The server is bound to 127.0.0.1, so this endpoint is only reachable from the local machine.
async fn shutdown_handler(State(state): State<Arc<AppState>>) -> axum::response::Response {
    info!("Shutdown requested, Syncix Core is stopping.");
    state.project.clear_port_file();
    tokio::spawn(async {
        // A short delay so the reply reaches the client.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        std::process::exit(0);
    });
    (axum::http::StatusCode::OK, "OK").into_response()
}

/// Returns the contents of sourcemap.json (luau-lsp compatible).
async fn sourcemap_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dm = state.data_model.read().await;
    let file_content = crate::sourcemap::json(&dm, &state.project.sync_dir, &state.project.root);
    ([(axum::http::header::CONTENT_TYPE, "application/json")], file_content)
}

/// Returns the tree as Roblox XML (.rbxmx / .rbxlx).
/// With `?target=<target>` only that subtree is written.
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
    // The number of skipped Enums is reported in a header so the CLI can warn the user.
    headers.insert(
        axum::http::HeaderName::from_static("x-syncix-skipped-enums"),
        axum::http::HeaderValue::from_str(&skipped.to_string())
            .unwrap_or(axum::http::HeaderValue::from_static("0")),
    );
    reply
}

async fn poll_handler(State(state): State<Arc<AppState>>) -> Json<Option<Payload>> {
    state.health_monitor.inc_messages();
    // A poll is the only proof that Studio is up; the connection state comes from here.
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

    // Long-poll: returns at once if the queue has a message, otherwise waits up to 10 seconds.
    // The 10-second limit matters: Studio's HTTP client times out at 30 seconds, and a
    // connection thought to be lost causes a needless reconnect + FULL_SYNC loop.
    // Thanks to the outbox, messages sent between two polls are not lost.
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
            // Packet drop with 5% probability (crash simulation)
            warn!("Chaos Engineering: Payload Dropped (Push)");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Simulated Failure",
            )
                .into_response();
        }
    }

    state.health_monitor.touch_studio();

    // The plugin's BatchQueue counters. Only these two numbers can show whether merging
    // really works; no need to verify it by dragging things by hand.
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
        // Plugin version check.
        //
        // Until now the version gate lived only in the plugin itself. If an older plugin
        // that did NOT check versions connected, the core silently accepted it
        // and the mismatch surfaced as strange behaviour.
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

    // This list is the only gate messages FROM the plugin can pass. A message
    // missing from the list is silently dropped — forgetting to add a new message type
    // here means "I am sending it but nothing happens".
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

/// Command endpoint for the CLI and other HTTP clients.
/// Body: { "event_type": "CREATE_INSTANCE" | "RENAME_INSTANCE" | "DELETE_INSTANCE" | "GET_TREE", "data": {...} }
async fn command_handler(
    State(state): State<Arc<AppState>>,
    Json(v): Json<serde_json::Value>,
) -> axum::response::Response {
    use crate::model::ResolveResult;

    let event_type = v.get("event_type").and_then(|e| e.as_str()).unwrap_or("");

    // FULL_SYNC / PULL: asks Studio to "send the tree again".
    // This command goes straight to Studio, NOT to the core; the reply is handled on the
    // normal FULL_SYNC path and rebuilds the model from scratch.
    // BIND: resolves a place conflict. There are two options and both can lose data,
    // so the decision is the user's.
    //   studio -> this place is taken as correct; the folder is overwritten from it
    //   disk   -> the folder is taken as correct; its contents are loaded into this place
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
                // Bind the folder to the incoming place and resume sync. On the next
                // FULL_SYNC the tree is rebuilt from Studio; the reconciler
                // brings the folder in line with it (deleted files go to the trash).
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
                // The folder was taken as correct: its identity is switched to the incoming place
                // so the same thing does not count as a place conflict again, but the tree
                // is NOT requested from Studio; the files on disk flow to Studio
                // through the watcher.
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

    // CREATE_INSTANCE: the UUID is generated HERE and returned in the reply,
    // so the caller can target the object it created by identity rather than by name.
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

    // Target validation happens here synchronously so the CLI gets a meaningful result.
    // The resolved UUID is written back into data; the core always receives an exact UUID.
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
                                "Parent '{}' is ambiguous: {} matches. Use a short UUID:\n{}",
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
            // Both the object being moved (id) and the new parent (newParentId) are resolved.
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
                                "{} '{}' is ambiguous: {} matches. Use a short UUID:\n{}",
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

/// Returns the full data of a single object, properties included (for the CLI's 'props').
/// ?target=<name|uuid|short-uuid>
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
                Json(serde_json::json!({ "error": "not found" }))
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

/// Checks model integrity (for the CLI's 'check').
/// Reports orphaned parent/child references and the class distribution.
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
        "totalObjects": instances.len().saturating_sub(1), // internal root excluded
        "rootServices": roots,
        "scriptsWithSource": scripts_with_source,
        "byClass": by_class,
    }))
}

/// Returns the current DataModel tree as JSON (for the CLI's 'list' and debugging).
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

/// WebSocket-based RPC handler for the VS Code extension
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

    // Core to VS Code (push events)
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

    // When either connection drops, close both
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    state.health_monitor.remove_connection();
    warn!(
        "VS Code RPC client disconnected. Active connections: {}",
        state
            .health_monitor
            .get_status(&state.project, state.actual_port)
            .active_connections
    );
}
