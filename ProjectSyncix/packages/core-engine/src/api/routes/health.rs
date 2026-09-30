use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::health::HealthStatus;
use crate::server::AppState;

/// What the core is, and the token the rest of the API needs.
///
/// This is the only call that needs no token: the Studio plugin cannot read files, so
/// it scans the local ports, finds its project here and takes the token from the same
/// reply. A web page cannot read this body -- the core sends no CORS header, so the
/// browser refuses to hand a cross-origin response to page scripts.
///
/// It no longer reports the project's path. The name is enough to tell two projects
/// apart, and the path carried the user's account name.
#[utoipa::path(
    get,
    path = "/health",
    tag = "core",
    responses((status = 200, description = "Identity, state and the access token"))
)]
pub async fn status(State(state): State<Arc<AppState>>) -> Json<HealthStatus> {
    crate::health::health_handler(State(state)).await
}
