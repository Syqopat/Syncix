use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use tracing::{error, info};

use crate::api::errors::{accepted, ApiError, ApiResult};
use crate::api::resolve::{resolve_one, resolve_optional};
use crate::server::AppState;
use crate::transport::{EventType, Payload};

/// Maps a command name from a client (editor RPC, CLI) to a core event.
pub fn map_command(event_type: &str) -> Option<EventType> {
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

/// One command from the CLI or another local client.
#[utoipa::path(
    post,
    path = "/commands",
    tag = "commands",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Accepted; a create also returns the new object's id"),
        (status = 400, description = "Unknown command, or a field is missing"),
        (status = 404, description = "A target does not exist"),
        (status = 409, description = "A target matches more than one object"),
        (status = 500, description = "Could not be forwarded to the core")
    ),
    security(("token" = []))
)]
pub async fn run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Response> {
    let event_type = body.get("event_type").and_then(|e| e.as_str()).unwrap_or("");

    if event_type == "BIND" {
        return bind(&state, &body).await;
    }

    // FULL_SYNC / PULL go straight to Studio, not to the core: the reply arrives on the
    // normal full-sync path and rebuilds the model.
    if event_type == "FULL_SYNC" || event_type == "PULL" {
        state.studio_outbox.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::FullSyncRequest,
            data: serde_json::json!({}),
        });
        state.health_monitor.inc_outbound();
        return Ok(accepted());
    }

    let Some(core_event) = map_command(event_type) else {
        return Err(ApiError::BadRequest(format!(
            "Unknown event_type: {}",
            event_type
        )));
    };

    let mut data = body.get("data").cloned().unwrap_or(serde_json::json!({}));
    if !data.is_object() {
        return Err(ApiError::BadRequest("data must be an object.".to_string()));
    }

    // A create gets its UUID here and hands it back, so the caller can address what it
    // just made by identity instead of by name.
    let mut created_id: Option<String> = None;
    if core_event == EventType::CreateInstance {
        let id = data
            .get("id")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        data["id"] = serde_json::json!(id);
        created_id = Some(id);
    }

    resolve_targets(&state, &core_event, &mut data).await?;

    let payload = Payload {
        version: "v1".to_string(),
        event_type: core_event,
        data,
    };
    if let Err(e) = state.tx_to_core.send(payload).await {
        error!("Could not forward the command to the core ({}): {}", event_type, e);
        return Err(ApiError::Internal(
            "The command could not be forwarded to the core.".to_string(),
        ));
    }
    info!("Command received: {}", event_type);

    Ok(match created_id {
        Some(id) => (StatusCode::OK, Json(serde_json::json!({ "id": id }))).into_response(),
        None => accepted(),
    })
}

/// Turns every target a command carries into an exact UUID, so the core never has to
/// guess what a name meant.
async fn resolve_targets(
    state: &Arc<AppState>,
    event: &EventType,
    data: &mut serde_json::Value,
) -> ApiResult<()> {
    let field = |data: &serde_json::Value, name: &str| {
        data.get(name)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };

    match event {
        EventType::RenameInstance
        | EventType::DeleteInstance
        | EventType::SetProperty
        | EventType::SetAttribute
        | EventType::SetTags => {
            let dm = state.data_model.read().await;
            let uuid = resolve_one(&dm, "Target", &field(data, "id"))?;
            data["id"] = serde_json::json!(uuid.to_string());
        }
        EventType::CreateInstance => {
            let dm = state.data_model.read().await;
            if let Some(uuid) = resolve_optional(&dm, "Parent", &field(data, "parentId"))? {
                data["parentId"] = serde_json::json!(uuid.to_string());
            }
        }
        EventType::ReparentInstance => {
            let dm = state.data_model.read().await;
            for (name, label) in [("id", "Target"), ("newParentId", "New parent")] {
                let uuid = resolve_one(&dm, label, &field(data, name))?;
                data[name] = serde_json::json!(uuid.to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

/// Resolves a place conflict. Both answers can lose work, so the user decides:
///   studio -> this place is right, the folder is rewritten from it
///   disk   -> the folder is right, its files flow into the place
async fn bind(state: &Arc<AppState>, body: &serde_json::Value) -> ApiResult<Response> {
    let side = body
        .get("data")
        .and_then(|d| d.get("side"))
        .and_then(|x| x.as_str())
        .unwrap_or("");

    let conflict = state
        .place_clash_state
        .lock()
        .ok()
        .and_then(|c| c.clone())
        .ok_or_else(|| ApiError::Conflict("There is no place conflict to resolve.".to_string()))?;

    match side {
        "studio" => {
            state.project.bind_place(&conflict.incoming_place);
            crate::project::set_sync_suspended(false);
            if let Ok(mut guard) = state.place_clash_state.lock() {
                *guard = None;
            }
            // The next full sync rebuilds the tree from Studio; the reconciler brings
            // the folder in line and deleted files go to the trash.
            state.studio_outbox.push(Payload {
                version: "v1".to_string(),
                event_type: EventType::FullSyncRequest,
                data: serde_json::json!({}),
            });
            state.health_monitor.inc_outbound();
            Ok(accepted())
        }
        "disk" => {
            // The folder wins: its identity switches to the incoming place so this stops
            // counting as a conflict, and the watcher pushes the files to Studio.
            state.project.bind_place(&conflict.incoming_place);
            crate::project::set_sync_suspended(false);
            if let Ok(mut guard) = state.place_clash_state.lock() {
                *guard = None;
            }
            Ok(accepted())
        }
        _ => Err(ApiError::BadRequest(
            "side must be 'studio' or 'disk'.".to_string(),
        )),
    }
}
