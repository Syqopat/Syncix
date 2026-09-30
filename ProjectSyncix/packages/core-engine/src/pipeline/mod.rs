use crate::model::InstanceNode;
use serde_json::Value;

/// Export layer (export pipeline)
/// Converts the current DataModel tree into other formats.
pub struct ExportPipeline;

impl ExportPipeline {
    /// Exports the DataModel as one large JSON dump
    pub fn export_to_json(root: &InstanceNode) -> String {
        serde_json::to_string_pretty(root).unwrap()
    }

    /// To be added: export in rbxlx (XML) format
    pub fn export_to_rbxlx(_root: &InstanceNode) -> String {
        unimplemented!("RBXLX export is not supported yet.")
    }
}

/// Import layer (import pipeline)
/// Converts data from other formats into an InstanceNode tree.
pub struct ImportPipeline;

impl ImportPipeline {
    /// Loads the DataModel from a JSON dump file
    pub fn import_from_json(json_str: &str) -> Result<InstanceNode, String> {
        let root: InstanceNode = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
        Ok(root)
    }
}

/// Schema migration layer (migration pipeline)
/// Converts old formats to the new one when the version changes.
pub struct MigrationPipeline;

impl MigrationPipeline {
    /// Migrates from the v1 schema to the v2 schema.
    /// E.g. if the "Color" property changed from a Color3 object to a String, it is converted here.
    pub fn migrate_v1_to_v2(old_json: &str) -> Result<String, String> {
        let mut v: Value = serde_json::from_str(old_json).map_err(|e| e.to_string())?;

        // Example migration: if "class_name" is being capitalised to "ClassName":
        if let Some(class_name) = v.get("class_name").cloned() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("ClassName".to_string(), class_name);
                obj.remove("class_name");
            }
        }

        serde_json::to_string(&v).map_err(|e| e.to_string())
    }
}
