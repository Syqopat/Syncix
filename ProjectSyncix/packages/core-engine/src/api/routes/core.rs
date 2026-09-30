use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};

use crate::api::errors::accepted;
use crate::server::AppState;

/// Stops the core.
///
/// Killing it by process name (taskkill /IM) also killed other projects' cores and
/// only worked on Windows. The call needs the project token, so a web page the user
/// happens to have open cannot stop their sync.
#[utoipa::path(
    post,
    path = "/core/stop",
    tag = "core",
    responses((status = 200, description = "The core stops right after answering")),
    security(("token" = []))
)]
pub async fn stop(State(state): State<Arc<AppState>>) -> Response {
    tracing::info!("Stop requested, Syncix Core is shutting down.");
    state.project.clear_port_file();
    tokio::spawn(async {
        // A short delay so the reply reaches the client.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        std::process::exit(0);
    });
    accepted().into_response()
}
