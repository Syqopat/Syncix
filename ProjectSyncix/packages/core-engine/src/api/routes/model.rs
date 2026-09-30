use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use tracing::warn;

use crate::api::errors::{ApiError, ApiResult};
use crate::api::resolve::resolve_one;
use crate::server::AppState;

/// Every object in the tree, flat: id, name, class and parent.
#[utoipa::path(
    get,
    path = "/model/tree",
    tag = "model",
    responses((status = 200, description = "The tree as a flat list")),
    security(("token" = []))
)]
pub async fn tree(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
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

/// One object with its properties.
///
/// This used to answer `200 OK` with an `{"error": ...}` body for a target that does
/// not exist, so a client had to parse the body to learn the call failed.
#[utoipa::path(
    get,
    path = "/model/object",
    tag = "model",
    params(("target" = String, Query, description = "Name, UUID or short UUID")),
    responses(
        (status = 200, description = "The object and its properties"),
        (status = 400, description = "No target given"),
        (status = 404, description = "No such target"),
        (status = 409, description = "The target matches more than one object")
    ),
    security(("token" = []))
)]
pub async fn object(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let target = params.get("target").cloned().unwrap_or_default();
    let dm = state.data_model.read().await;
    let uuid = resolve_one(&dm, "Target", &target)?;
    let node = dm
        .get_instance(&uuid)
        .ok_or_else(|| ApiError::NotFound(format!("Target not found: {}", target)))?;
    Ok(Json(
        serde_json::to_value(node).unwrap_or(serde_json::json!(null)),
    ))
}

/// Model integrity: orphaned references and the class distribution.
#[utoipa::path(
    get,
    path = "/model/verify",
    tag = "model",
    responses((status = 200, description = "Report; `ok` is false when the model is inconsistent")),
    security(("token" = []))
)]
pub async fn verify(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let dm = state.data_model.read().await;
    let instances = dm.get_all_instances();

    let mut by_class: std::collections::BTreeMap<String, usize> = Default::default();
    let mut scripts_with_source = 0usize;
    let mut roots = 0usize;
    for node in instances.values() {
        if node.class_name == "DataModel" {
            continue;
        }
        *by_class.entry(node.class_name.clone()).or_insert(0) += 1;
        if node.parent.is_none() {
            roots += 1;
        }
        if matches!(node.class_name.as_str(), "Script" | "LocalScript" | "ModuleScript")
            && node.source.is_some()
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
        "totalObjects": instances.len().saturating_sub(1),
        "rootServices": roots,
        "scriptsWithSource": scripts_with_source,
        "byClass": by_class,
    }))
}

/// sourcemap.json, in the shape luau-lsp reads.
#[utoipa::path(
    get,
    path = "/model/sourcemap",
    tag = "model",
    responses((status = 200, description = "sourcemap.json")),
    security(("token" = []))
)]
pub async fn sourcemap(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dm = state.data_model.read().await;
    let body = crate::sourcemap::json(&dm, &state.project.sync_dir, &state.project.root);
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
}

/// The tree as Roblox XML. With `?target=` only that subtree.
#[utoipa::path(
    get,
    path = "/model/export",
    tag = "model",
    params(("target" = Option<String>, Query, description = "Only this subtree")),
    responses(
        (status = 200, description = "Roblox XML; x-syncix-skipped-enums counts values left out"),
        (status = 404, description = "No such target"),
        (status = 409, description = "The target matches more than one object")
    ),
    security(("token" = []))
)]
pub async fn export(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Response> {
    let dm = state.data_model.read().await;
    let root = match params.get("target").filter(|t| !t.is_empty()) {
        None => None,
        Some(target) => Some(resolve_one(&dm, "Target", target)?),
    };

    let (xml, skipped) = crate::rbxmx::export_rbxmx(&dm, root.as_ref());
    if skipped > 0 {
        warn!("export: skipped {} unrecognised enum value(s).", skipped);
    }

    let mut reply = xml.into_response();
    let headers = reply.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/xml"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-syncix-skipped-enums"),
        axum::http::HeaderValue::from_str(&skipped.to_string())
            .unwrap_or(axum::http::HeaderValue::from_static("0")),
    );
    Ok(reply)
}
