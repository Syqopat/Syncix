use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{stream::StreamExt, SinkExt};
use tracing::{error, info, warn};

use crate::server::AppState;
use crate::transport::Payload;

/// WebSocket the editor extension talks over.
pub async fn upgrade(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| serve(socket, state))
}

fn connection_count(state: &AppState) -> usize {
    state
        .health_monitor
        .get_status(&state.project, state.actual_port)
        .active_connections
}

async fn serve(socket: WebSocket, state: Arc<AppState>) {
    state.health_monitor.add_connection();
    let (mut sender, mut receiver) = socket.split();
    let mut from_core = state.tx_to_vscode.subscribe();

    info!("Editor RPC client connected. Active connections: {}", connection_count(&state));

    let mut to_editor = tokio::spawn(async move {
        while let Ok(message) = from_core.recv().await {
            if sender.send(Message::Text(message)).await.is_err() {
                break;
            }
        }
    });

    let reply_channel = state.tx_to_vscode.clone();
    let to_core = state.tx_to_core.clone();
    let mut from_editor = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            info!("RPC command received: {}", text);
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(request_id) = value.get("request_id") else {
                continue;
            };

            // The acknowledgement resolves the extension's promise; without it every
            // call waits for the RPC timeout.
            let ack = serde_json::json!({ "request_id": request_id, "data": { "status": "ok" } });
            let _ = reply_channel.send(ack.to_string());

            let event_type = value.get("event_type").and_then(|e| e.as_str()).unwrap_or("");
            if let Some(event) = super::commands::map_command(event_type) {
                let payload = Payload {
                    version: "v1".to_string(),
                    event_type: event,
                    data: value.get("data").cloned().unwrap_or(serde_json::json!({})),
                };
                if let Err(e) = to_core.send(payload).await {
                    error!("Could not forward command to the core ({}): {}", event_type, e);
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut to_editor) => from_editor.abort(),
        _ = (&mut from_editor) => to_editor.abort(),
    };

    state.health_monitor.remove_connection();
    warn!("Editor RPC client disconnected. Active connections: {}", connection_count(&state));
}
