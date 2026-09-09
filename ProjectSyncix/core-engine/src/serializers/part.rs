use super::Serializer;
use crate::model::InstanceNode;

pub struct PartSerializer;

impl Serializer for PartSerializer {
    fn get_class_name(&self) -> &'static str {
        "Part"
    }

    fn serialize(&self, instance: &InstanceNode) -> Result<String, String> {
        // AI-Friendly JSON üretimi
        serde_json::to_string_pretty(instance).map_err(|e| e.to_string())
    }

    fn deserialize(&self, data: &str) -> Result<InstanceNode, String> {
        serde_json::from_str(data).map_err(|e| e.to_string())
    }
}
