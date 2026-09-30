pub mod commands;
pub mod core;
pub mod health;
pub mod model;
pub mod rpc;
pub mod sync;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::auth;
use crate::api::docs::ApiDoc;
use crate::server::AppState;

/// Every request body the API accepts, largest first: a full sync of a big place.
const MAX_BODY_BYTES: usize = 250 * 1024 * 1024;

/// The HTTP surface.
///
/// Paths say what they act on -- /model/... reads the tree, /sync/... is the plugin's
/// channel, /core/stop ends the process -- because the old names (/object, /build,
/// /shutdown) did not say which part of the system they belonged to.
pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/sync/poll", get(sync::poll))
        .route("/sync/push", post(sync::push))
        .route("/rpc", get(rpc::upgrade))
        .route("/commands", post(commands::run))
        .route("/model/tree", get(model::tree))
        .route("/model/object", get(model::object))
        .route("/model/verify", get(model::verify))
        .route("/model/sourcemap", get(model::sourcemap))
        .route("/model/export", get(model::export))
        .route("/core/stop", post(core::stop))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ));

    Router::new()
        .route("/health", get(health::status))
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .merge(protected)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}
