use chrono::Utc;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize)]
pub struct AuditLogEntry {
    pub timestamp: String,
    pub transaction_id: Uuid,
    pub session_id: String,
    pub operation_count: usize,
    pub duration_ms: u128,
    pub result: String,
}

pub struct AuditLogger;

impl AuditLogger {
    pub fn log_transaction(
        transaction_id: Uuid,
        session_id: &str,
        operation_count: usize,
        duration: Duration,
        result: &str,
    ) {
        let entry = AuditLogEntry {
            timestamp: Utc::now().to_rfc3339(),
            transaction_id,
            session_id: session_id.to_string(),
            operation_count,
            duration_ms: duration.as_millis(),
            result: result.to_string(),
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("syncix_audit.log");

            if let Ok(mut f) = file {
                let _ = writeln!(f, "{}", json);
            }
        }
    }
}
