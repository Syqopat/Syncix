use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Syncix public API contract
/// This module defines the strict message schema between the core, Studio, the CLI and AI extensions.
/// Changes must follow backward-compatibility rules.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub event_type: String,
    pub api_version: u32,
    pub request_id: Option<String>,
    pub transaction_id: Option<String>,
    pub retry_count: u32,
    pub created_at: i64,
    pub data: Value,
}

impl MessageEnvelope {
    pub fn new(event_type: &str, data: Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            api_version: 1, // v1 API for now
            request_id: None,
            transaction_id: None,
            retry_count: 0,
            created_at: chrono::Utc::now().timestamp_millis(),
            data,
        }
    }

    pub fn with_request_id(mut self, id: String) -> Self {
        self.request_id = Some(id);
        self
    }

    pub fn with_transaction_id(mut self, id: String) -> Self {
        self.transaction_id = Some(id);
        self
    }
}

/// DTO (data transfer object) definitions
pub mod dto {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PropertyUpdateDto {
        pub syncix_id: String,
        pub property: String,
        pub value: Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CompositePatchDto {
        pub patches: Vec<PatchEntryDto>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum PatchEntryDto {
        UpdateProperty(PropertyUpdateDto),
        CreateInstance {
            syncix_id: String,
            class_name: String,
        },
        DeleteInstance {
            syncix_id: String,
        },
        ReparentInstance {
            syncix_id: String,
            new_parent_id: String,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_envelope_serialization() {
        let envelope = MessageEnvelope::new("TEST_EVENT", json!({"key": "value"}));
        let serialized = serde_json::to_string(&envelope).unwrap();
        assert!(serialized.contains(r#""api_version":1"#));
        assert!(serialized.contains(r#""event_type":"TEST_EVENT""#));
    }
}
