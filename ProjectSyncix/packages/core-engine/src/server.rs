use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::health::HealthMonitor;
use crate::transport::{Payload, StudioOutbox};

/// Everything a request handler is allowed to reach.
pub struct AppState {
    pub studio_outbox: Arc<StudioOutbox>,
    pub tx_to_core: tokio::sync::mpsc::Sender<Payload>,
    pub tx_to_vscode: broadcast::Sender<String>,
    pub health_monitor: Arc<HealthMonitor>,
    pub data_model: crate::model::SharedDataModel,
    /// Failure injection, used by the resilience tests.
    pub chaos_mode_enabled: bool,
    /// Project identity, reported through /health so a client can tell which project
    /// this core serves.
    pub project: Arc<crate::project::ProjectConfig>,
    /// The port actually bound; the requested one may have been taken.
    pub actual_port: u16,
    /// The token every call except /health has to carry.
    pub access_token: String,
    /// Set when ANOTHER place tried to connect to this folder.
    ///
    /// A sync folder belongs to one place. Merging two places silently duplicated
    /// singleton services such as StarterPlayerScripts, because service UUIDs are the
    /// same in every place. Now sync stops and the user decides.
    pub place_clash_state: Arc<std::sync::Mutex<Option<PlaceConflict>>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlaceConflict {
    /// The place this folder is bound to.
    pub folder_place: String,
    /// The place trying to connect.
    pub incoming_place: String,
    pub incoming_name: String,
    pub incoming_place_id: String,
}

/// Tries the requested port, then the ones after it.
///
/// The port used to be fixed at 8080: with a second project open, or another program
/// holding the port, the core died without saying why.
pub fn bind_with_fallback(
    cfg: &crate::project::ProjectConfig,
) -> Option<(std::net::TcpListener, u16)> {
    let first = cfg.wanted_port;
    // An explicitly requested port is the only one tried.
    let upper = if cfg.port_fixed {
        first.saturating_add(1)
    } else {
        first.saturating_add(crate::project::PORT_SCAN_SPAN)
    };

    for port in first..upper {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match std::net::TcpListener::bind(addr) {
            Ok(listener) => {
                if port != first {
                    warn!(
                        "Port {} is taken, using {} instead. The Studio plugin scans this range and will find it.",
                        first, port
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
            first
        );
    } else {
        error!(
            "All ports in the range {}-{} are taken. Change the 'port' value in syncix.toml.",
            first,
            upper - 1
        );
    }
    None
}

/// Serves the API. The listener is opened outside because the real port has to be in
/// AppState before the first request arrives.
pub async fn start_server(state: Arc<AppState>, listener: std::net::TcpListener) {
    let app = crate::api::router(state.clone());

    info!(
        "Syncix transport and RPC layer started: http://127.0.0.1:{} (project: {}) — docs at /docs",
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
