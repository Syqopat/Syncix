use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

/// Reference resolver (ReferenceResolver)
/// Manages references between objects such as ObjectValue, Motor and HingeConstraint.
/// If the referenced object (UUID) is not in the Workspace yet, it is parked for lazy resolution.
pub struct ReferenceResolver {
    /// UUID -> objects and property names waiting for this UUID
    /// E.g. when the "Part_B" UUID is created, it is assigned to the "Value" property of "ObjectValue_A".
    pending_references: RwLock<HashMap<String, Vec<PendingRef>>>,

    /// Map of references resolved so far (hard references)
    resolved_references: RwLock<HashSet<String>>,
}

pub struct PendingRef {
    pub source_uuid: String,
    pub property_name: String,
}

impl ReferenceResolver {
    pub fn new() -> Self {
        Self {
            pending_references: RwLock::new(HashMap::new()),
            resolved_references: RwLock::new(HashSet::new()),
        }
    }

    /// Queues a reference request.
    pub fn enqueue_reference(&self, target_uuid: &str, source_uuid: &str, property_name: &str) {
        let mut pending = self.pending_references.write().unwrap();
        let entry = pending.entry(target_uuid.to_string()).or_default();
        entry.push(PendingRef {
            source_uuid: source_uuid.to_string(),
            property_name: property_name.to_string(),
        });
    }

    /// Called when a new UUID joins the system (when an instance is created).
    /// Resolves any references waiting for this UUID.
    pub fn notify_uuid_created(&self, new_uuid: &str) -> Vec<PendingRef> {
        self.resolved_references
            .write()
            .unwrap()
            .insert(new_uuid.to_string());

        let mut pending = self.pending_references.write().unwrap();
        if let Some(waiting_list) = pending.remove(new_uuid) {
            return waiting_list; // routed to the Dispatcher on the Studio side and connected there
        }

        Vec::new()
    }
}

/// Asset registry (AssetRegistry)
/// Will hold local files such as meshes, textures and sounds, or rbxassetid:// links.
pub struct AssetRegistry {
    /// Local file path -> rbxassetid or syncix:// URL
    assets: RwLock<HashMap<String, String>>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self {
            assets: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_asset(&self, local_path: &str, asset_id: &str) {
        self.assets
            .write()
            .unwrap()
            .insert(local_path.to_string(), asset_id.to_string());
    }

    pub fn get_asset_id(&self, local_path: &str) -> Option<String> {
        self.assets.read().unwrap().get(local_path).cloned()
    }
}
