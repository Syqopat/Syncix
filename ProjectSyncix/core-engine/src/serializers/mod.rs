pub mod part;

use crate::model::InstanceNode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Interface every serializer must implement.
pub trait Serializer: Send + Sync {
    fn serialize(&self, instance: &InstanceNode) -> Result<String, String>;
    fn deserialize(&self, data: &str) -> Result<InstanceNode, String>;
    fn get_class_name(&self) -> &'static str;
}

/// General serializer that does not distinguish classes.
/// EVERY class without a registered custom serializer (Folder, Script, Model, GUI, services...)
/// is written to disk through it. That way every object in Studio appears as a file.
pub struct GenericSerializer;

impl Serializer for GenericSerializer {
    fn serialize(&self, instance: &InstanceNode) -> Result<String, String> {
        serde_json::to_string_pretty(instance).map_err(|e| e.to_string())
    }

    fn deserialize(&self, data: &str) -> Result<InstanceNode, String> {
        serde_json::from_str(data).map_err(|e| e.to_string())
    }

    fn get_class_name(&self) -> &'static str {
        "Instance"
    }
}

/// Central registry holding the serializers (registry & factory).
/// Performance and extensibility: the core engine does not know which instance type it handles.
/// It looks at the incoming data's `class_name` and takes the matching serializer from this registry.
/// Even if 50 new types were added, `main.rs` and `model.rs` would not change (open/closed principle).
pub struct SerializerRegistry {
    serializers: HashMap<String, Box<dyn Serializer>>,
    /// General fallback for classes without a registered custom serializer.
    fallback: Box<dyn Serializer>,
}

impl SerializerRegistry {
    pub fn new() -> Self {
        Self {
            serializers: HashMap::new(),
            fallback: Box::new(GenericSerializer),
        }
    }

    /// Registers a new serializer.
    pub fn register(&mut self, serializer: Box<dyn Serializer>) {
        let class_name = serializer.get_class_name().to_string();
        self.serializers.insert(class_name, serializer);
    }

    /// Returns the serializer for the given class.
    /// Without a custom serializer it falls back to the general one, so EVERY class can be written.
    pub fn get(&self, class_name: &str) -> Option<&dyn Serializer> {
        Some(
            self.serializers
                .get(class_name)
                .map(|b| b.as_ref())
                .unwrap_or_else(|| self.fallback.as_ref()),
        )
    }
}

/// Thread-safe registry shared by the whole system.
pub type SharedRegistry = Arc<RwLock<SerializerRegistry>>;

/// Creates the registry with the default serializers (Part, ...) loaded.
pub fn create_default_registry() -> SharedRegistry {
    let mut registry = SerializerRegistry::new();

    // All existing serializers are registered here.
    registry.register(Box::new(part::PartSerializer));

    Arc::new(RwLock::new(registry))
}
