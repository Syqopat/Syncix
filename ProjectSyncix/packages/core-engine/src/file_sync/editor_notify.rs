//! Split out of file_sync.rs.

#[allow(unused_imports)]
use super::*;

/// Reports the current state of an instance to the VS Code Explorer.
pub(crate) fn notify_vscode_updated(
    dm: &crate::model::DataModel,
    uuid: &Uuid,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
) {
    if let Some(inst) = dm.get_instance(uuid) {
        let msg = serde_json::json!({
            "event_type": event_type,
            "data": {
                "id": inst.syncix_id,
                "name": inst.name,
                "className": inst.class_name,
                "parentId": inst.parent.map(|u| u.to_string()),
                "childrenIds": inst.children,
                "isExpanded": false
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());
    }
}
