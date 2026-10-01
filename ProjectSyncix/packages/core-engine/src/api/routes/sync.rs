use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand::Rng;
use tracing::{error, warn};

use crate::api::errors::{accepted, ApiError};
use crate::server::AppState;
use crate::transport::{EventType, Payload};

/// The most messages one poll reply carries.
const MAX_POLL_BATCH: usize = 64;

/// Long poll the Studio plugin waits on.
///
/// It returns as soon as the outbox has something and otherwise waits ten seconds.
/// That limit matters: Studio's HTTP client gives up at thirty, and a connection
/// believed lost costs a reconnect and a full resync.
#[utoipa::path(
    get,
    path = "/sync/poll",
    tag = "sync",
    params(("batch" = Option<usize>, Query, description = "How many messages in one reply, up to 64")),
    responses((status = 200, description = "One message, a batch of messages, or null")),
    security(("token" = []))
)]
pub async fn poll(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    state.health_monitor.inc_messages();
    // A poll is the only proof Studio is alive; the connection state comes from here.
    state.health_monitor.touch_studio();

    if state.chaos_mode_enabled {
        inject_latency().await;
    }

    let Some(first) = state
        .studio_outbox
        .pop_or_wait(std::time::Duration::from_secs(10))
        .await
    else {
        return Json(serde_json::Value::Null);
    };

    // One message per request made a large import cost one HTTP request per command,
    // enough to hit Studio's request limit. An older plugin asks for no batch and
    // still gets a single message.
    let batch = params
        .get("batch")
        .and_then(|b| b.parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_POLL_BATCH);

    if batch == 0 {
        state.health_monitor.inc_outbound();
        return Json(serde_json::to_value(first).unwrap_or(serde_json::Value::Null));
    }

    let mut messages = vec![first];
    while messages.len() < batch {
        match state.studio_outbox.try_pop() {
            Some(payload) => messages.push(payload),
            None => break,
        }
    }
    for _ in &messages {
        state.health_monitor.inc_outbound();
    }
    Json(serde_json::to_value(messages).unwrap_or(serde_json::Value::Null))
}

async fn inject_latency() {
    let should_delay = rand::thread_rng().gen_bool(0.1);
    if should_delay {
        let delay = rand::thread_rng().gen_range(500..2000);
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        warn!("Chaos Engineering: Network Latency Injected (Poll)");
    }
}

/// Everything the Studio plugin sends: tree changes, full syncs, selection, metrics.
#[utoipa::path(
    post,
    path = "/sync/push",
    tag = "sync",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Accepted"),
        (status = 500, description = "Could not be forwarded to the core")
    ),
    security(("token" = []))
)]
pub async fn push(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Payload>,
) -> Result<Response, ApiError> {
    state.health_monitor.inc_messages();

    if state.chaos_mode_enabled && rand::thread_rng().gen_bool(0.05) {
        warn!("Chaos Engineering: Payload Dropped (Push)");
        return Err(ApiError::Internal("Simulated Failure".to_string()));
    }

    state.health_monitor.touch_studio();

    if payload.event_type == EventType::PluginMetrics {
        record_metrics(&state, &payload);
        return Ok(accepted());
    }

    // This list is the only gate messages from the plugin pass. A type missing from it
    // is dropped without a word, which reads as "I am sending it but nothing happens".
    let forwarded = matches!(
        payload.event_type,
        EventType::ClientUpdate
            | EventType::CompositeUpdate
            | EventType::FullSync
            | EventType::Selection
    );
    if forwarded {
        state.health_monitor.inc_inbound();
        if let Err(e) = state.tx_to_core.send(payload).await {
            error!("Could not forward message to the core: {}", e);
            return Err(ApiError::Internal(
                "The message could not be forwarded to the core.".to_string(),
            ));
        }
    }
    Ok(accepted().into_response())
}

/// The plugin's own counters. They are the only way to see whether coalescing works.
fn record_metrics(state: &Arc<AppState>, payload: &Payload) {
    let number = |field: &str| {
        payload
            .data
            .get(field)
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as usize
    };

    state
        .health_monitor
        .set_plugin_metrics(number("queued"), number("coalesced"), number("floods"));

    // The plugin used to be the only side checking versions, so an older plugin that
    // did not check was accepted in silence and the mismatch surfaced as odd behaviour.
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
        number("activity_total"),
        number("activity_in"),
        number("activity_out"),
        number("conflicts"),
    );
}
