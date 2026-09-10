use crate::model::InstanceNode;
use crate::serializers::SharedRegistry;
use crate::vfs::FileSystemProvider;
use std::path::Path;
use std::sync::Arc;

/// TreeBuilder
/// Builds the Roblox tree recursively by reading the folder structure through the VFS.
/// Rules:
/// 1. If `init.class_name.json` exists (e.g. init.model.json), the folder becomes that class.
/// 2. Other `.json` files inside become children of that root class.
pub struct TreeBuilder {
    vfs: Arc<dyn FileSystemProvider>,
    registry: SharedRegistry,
}

impl TreeBuilder {
    pub fn new(vfs: Arc<dyn FileSystemProvider>, registry: SharedRegistry) -> Self {
        Self { vfs, registry }
    }

    /// Builds the DataModel tree starting from a folder
    pub async fn build_from_dir(&self, root_path: &Path) -> Result<Option<InstanceNode>, String> {
        if !self.vfs.is_dir(root_path) {
            return Err("Root path must be a directory".into());
        }

        Box::pin(self.process_directory(root_path)).await
    }

    async fn process_directory(&self, dir_path: &Path) -> Result<Option<InstanceNode>, String> {
        let entries = self.vfs.read_dir(dir_path).map_err(|e| e.to_string())?;

        // 1. First find out whether this folder has a "root file" (init.*.json)
        let mut root_node = None;

        for path in &entries {
            if self.vfs.is_file(path) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.starts_with("init.") && filename.ends_with(".json") {
                        // found a file like init.model.json
                        let data = self.vfs.read_to_string(path).map_err(|e| e.to_string())?;

                        // Take the class name from the file (init.model.json -> Model)
                        // The file will be deserialized to get the real values
                        // For now deserialization goes through the registry
                        // Note: the JSON file already contains class_name. Any serializer could open it,
                        // but to use GenericSerializer we have to fetch the generic serializer from the registry.

                        let node = self.deserialize_file(&data).await?;
                        root_node = Some(node);
                        break; // there can be only one init file
                    }
                }
            }
        }

        // Without an init file this folder does not count as a Roblox object.
        // Could it count as a Folder by default, for tests or Rojo compatibility?
        // User decision: "a plain folder stays just a physical folder"
        // So without a root_node we return `None` (its files are not attached to the DataModel).
        let mut node = match root_node {
            Some(n) => n,
            None => return Ok(None), // this folder does not represent a Roblox object
        };

        // 2. Handle the children
        for path in &entries {
            if self.vfs.is_dir(path) {
                // Subfolders are handled recursively
                if let Ok(Some(child_node)) = Box::pin(self.process_directory(path)).await {
                    node.add_child(child_node.syncix_id);
                }
            } else if self.vfs.is_file(path) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Other .json files besides the init file are child objects
                    if filename.ends_with(".json") && !filename.starts_with("init.") {
                        let data = self.vfs.read_to_string(path).map_err(|e| e.to_string())?;
                        if let Ok(child_node) = self.deserialize_file(&data).await {
                            node.add_child(child_node.syncix_id);
                        }
                    }
                }
            }
        }

        Ok(Some(node))
    }

    async fn deserialize_file(&self, json_data: &str) -> Result<InstanceNode, String> {
        // Temporarily parse the JSON to find class_name
        let v: serde_json::Value = serde_json::from_str(json_data).map_err(|e| e.to_string())?;
        let class_name = v["class_name"]
            .as_str()
            .ok_or("Missing class_name in JSON")?;

        let registry = self.registry.read().await;
        let serializer = registry
            .get(class_name)
            .ok_or(format!("No serializer found for {}", class_name))?;

        serializer.deserialize(json_data)
    }
}
