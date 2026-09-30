//! Split out of file_sync.rs.

#[allow(unused_imports)]
use super::*;

/// Actually applies the deletions whose grace period has expired.
///
/// The wait keeps a move from being mistaken for a deletion: the operating system
/// reports a move as "delete + create". If the file comes back within this time,
/// the deletion has been cancelled.
pub(crate) static DELETE_GRACE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(800);

pub fn configure_delete_grace(ms: u64) {
    DELETE_GRACE_MS.store(ms, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn delete_grace_period() -> std::time::Duration {
    std::time::Duration::from_millis(DELETE_GRACE_MS.load(std::sync::atomic::Ordering::Relaxed))
}

pub(crate) fn apply_ripe_deletes(
    pending_item: &mut std::collections::HashMap<Uuid, std::time::Instant>,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
    tx_to_vscode: &tokio::sync::broadcast::Sender<String>,
    disk_notify: &Arc<tokio::sync::Notify>,
) {
    if pending_item.is_empty() {
        return;
    }
    let current_time = std::time::Instant::now();
    let ready_deletes: Vec<Uuid> = pending_item
        .iter()
        .filter(|(_, t)| current_time.duration_since(**t) >= delete_grace_period())
        .map(|(u, _)| *u)
        .collect();

    for uuid in ready_deletes {
        pending_item.remove(&uuid);

        let removed_node = {
            let mut dm = data_model.blocking_write();
            dm.remove_instance(&uuid)
        };
        let Some(instance) = removed_node else {
            continue;
        };

        info!(
            "The file for '{}' was deleted from disk; the instance is being removed.",
            instance.name
        );

        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "DESTROY",
                    "data": { "syncix_id": uuid }
                }]
            }),
        });

        let msg = serde_json::json!({
            "event_type": "INSTANCE_REMOVED",
            "data": {
                "id": uuid,
                "parentId": instance.parent.map(|u| u.to_string())
            }
        });
        let _ = tx_to_vscode.send(msg.to_string());

        // The subtree left the model too; let the disk writer rewrite the tree.
        disk_notify.notify_one();
    }
}
