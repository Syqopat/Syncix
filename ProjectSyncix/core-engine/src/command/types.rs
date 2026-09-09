use crate::model::{PropertyValue, SharedDataModel};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata for collaboration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub operation_id: Uuid,
    pub author_id: String,
    pub timestamp: i64,
    pub transaction_id: Option<Uuid>,
}

impl CommandMetadata {
    pub fn new(author_id: &str) -> Self {
        Self {
            operation_id: Uuid::new_v4(),
            author_id: author_id.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            transaction_id: None,
        }
    }
}

/// A generic interface for all Syncix Operations
#[async_trait::async_trait]
pub trait Command: Send + Sync {
    async fn execute(&mut self, model: &SharedDataModel) -> Result<(), String>;
    async fn undo(&mut self, model: &SharedDataModel) -> Result<(), String>;
    async fn redo(&mut self, model: &SharedDataModel) -> Result<(), String>;
    fn validate(&self, model: &SharedDataModel) -> Result<(), String>;
    fn serialize_cmd(&self) -> serde_json::Value;
    fn metadata(&self) -> &CommandMetadata;
}

// ---------------------------------------------
// Concrete Commands
// ---------------------------------------------

pub struct ModifyPropertyCommand {
    pub metadata: CommandMetadata,
    pub syncix_id: Uuid,
    pub property_name: String,
    pub new_value: PropertyValue,
    // State needed for undo
    pub old_value: Option<PropertyValue>,
}

impl ModifyPropertyCommand {
    pub fn new(author: &str, syncix_id: Uuid, prop: &str, new_val: PropertyValue) -> Self {
        Self {
            metadata: CommandMetadata::new(author),
            syncix_id,
            property_name: prop.to_string(),
            new_value: new_val,
            old_value: None,
        }
    }
}

#[async_trait::async_trait]
impl Command for ModifyPropertyCommand {
    async fn execute(&mut self, model: &SharedDataModel) -> Result<(), String> {
        let mut store = model.write().await;
        if let Some(node) = store.get_mut_instance(&self.syncix_id) {
            // Save old state for undo
            self.old_value = node.properties.get(&self.property_name).cloned();
            node.properties
                .insert(self.property_name.clone(), self.new_value.clone());
            Ok(())
        } else {
            Err("Node not found".to_string())
        }
    }

    async fn undo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if let Some(old) = &self.old_value {
            let mut store = model.write().await;
            if let Some(node) = store.get_mut_instance(&self.syncix_id) {
                node.properties
                    .insert(self.property_name.clone(), old.clone());
                return Ok(());
            }
        }
        Err("Cannot undo property without old value or node".to_string())
    }

    async fn redo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        self.execute(model).await
    }

    fn validate(&self, _model: &SharedDataModel) -> Result<(), String> {
        // Here we could call PropertySchemaProvider to validate `new_value` against the property schema
        Ok(())
    }

    fn serialize_cmd(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "ModifyProperty",
            "syncix_id": self.syncix_id,
            "property": self.property_name,
            "value": self.new_value,
            "meta": self.metadata
        })
    }

    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }
}
