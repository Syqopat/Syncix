mod edit;
mod lookup;
mod verify;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(Uuid),
    #[error("Stale version (conflict): current {current}, incoming {incoming}")]
    VersionConflict { current: i64, incoming: i64 },
    #[error("Invalid parent: {0}")]
    InvalidParent(Uuid),
    #[error("Identity already in use: {0}")]
    IdentityTaken(Uuid),
}

/// Syncix's own, self-contained data model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceNode {
    pub class_name: String,
    pub name: String,
    pub syncix_id: Uuid,
    /// Model version (for migrations)
    pub schema_version: u32,
    /// For conflict resolution. Unix timestamp in milliseconds.
    pub last_updated: i64,
    pub properties: BTreeMap<String, PropertyValue>,
    pub children: Vec<Uuid>,
    pub parent: Option<Uuid>,
    /// Source code for script classes (Script/LocalScript/ModuleScript).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Roblox attributes (custom values added with SetAttribute).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, PropertyValue>,
    /// CollectionService tags.
    ///
    /// A separate channel, not a property: in Roblox tags do not live on the instance
    /// as a field; CollectionService keeps them. In a game that relies on tags a large
    /// share of the logic runs through them, so while tags were not carried the
    /// editor could not see half the game.
    ///
    /// Must be kept sorted and without duplicates; a Vec is used instead of a set and
    /// sorted when written, so both sides see the same list in the same order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl InstanceNode {
    pub fn new(class_name: &str, name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
            name: name.to_string(),
            syncix_id: Uuid::new_v4(),
            schema_version: 1,
            last_updated: Utc::now().timestamp_millis(),
            properties: BTreeMap::new(),
            children: Vec::new(),
            parent: None,
            source: None,
            attributes: BTreeMap::new(),
            tags: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child_id: Uuid) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }
}

impl Default for InstanceNode {
    fn default() -> Self {
        Self {
            class_name: String::new(),
            name: String::new(),
            syncix_id: Uuid::nil(),
            schema_version: 1,
            last_updated: 0,
            properties: BTreeMap::new(),
            children: Vec::new(),
            parent: None,
            source: None,
            attributes: BTreeMap::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Vector3 { x: f32, y: f32, z: f32 },
    Color3 { r: f32, g: f32, b: f32 },
    /// For GUI: UDim2 = (X: scale/offset, Y: scale/offset)
    UDim2 {
        xs: f32,
        xo: f32,
        ys: f32,
        yo: f32,
    },
    Vector2 {
        x: f32,
        y: f32,
    },
    UDim {
        scale: f32,
        offset: f32,
    },
    /// Position + 3x3 rotation matrix. Orientation is not enough: a part's
    /// real orientation cannot be fully expressed with Euler angles.
    CFrame {
        pos: [f32; 3],
        rot: [f32; 9],
    },
    NumberRange {
        min: f32,
        max: f32,
    },
    /// Reference to another instance (ObjectValue.Value, Motor6D.Part0,
    /// Model.PrimaryPart, ...). The value is the target's UUID; an empty string = nil.
    ///
    /// It must be a separate type: carried as text, neither side could tell it
    /// from plain text and turn it into an instance.
    Ref(String),
    /// Roblox's named colour palette ("Really red", "Deep orange").
    ///
    /// Same reasoning as Ref: for a while it was carried as a plain String and
    /// `part.BrickColor = "Really red"` silently failed on the Studio side
    /// — text does not convert to BrickColor implicitly. Types are decided from the
    /// value, so the value itself has to carry its type.
    BrickColor(String),
    /// Asset reference: "rbxassetid://123". MeshId, SoundId, Image, Texture.
    ///
    /// Kept apart from String because Roblox's newer Content type does not accept a plain
    /// text assignment; we need to know which way to write it.
    Content(String),
    /// Colour curve: ParticleEmitter.Color, UIGradient.Color, Beam.Color.
    /// Each point is (time, colour); Roblox computes the values in between itself.
    ColorSequence(Vec<ColorKeypoint>),
    /// Number sequence curve: transparency, size, UIGradient.Transparency.
    /// envelope is Roblox's random variance; cannot be left zero, carries info.
    NumberSequence(Vec<NumberKeypoint>),
    /// Rectangle for 9-slice UI (ImageLabel.SliceCenter).
    Rect {
        min: [f32; 2],
        max: [f32; 2],
    },
    /// Font. Not an enum but a compound value: family + weight + style.
    Font {
        family: String,
        weight: String,
        style: String,
    },
    /// Custom physics: density, friction, elasticity and their weights.
    PhysicalProperties {
        density: f32,
        friction: f32,
        elasticity: f32,
        friction_weight: f32,
        elasticity_weight: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorKeypoint {
    pub t: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NumberKeypoint {
    pub t: f32,
    pub v: f32,
    pub envelope: f32,
}

/// Patch structure holding the differences between two objects.
/// Instead of the whole object, only this patch goes over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancePatch {
    pub syncix_id: Uuid,
    pub changed_properties: BTreeMap<String, PropertyValue>,
}

impl InstanceNode {
    /// Compares an old node with this one and returns only the changed properties as a patch.
    /// Performance: instead of pushing megabytes, only the changed Color3 or Position is sent.
    pub fn diff(&self, old_node: &InstanceNode) -> Option<InstancePatch> {
        if self.syncix_id != old_node.syncix_id {
            return None; // different objects cannot be compared
        }

        let mut changed_properties = BTreeMap::new();

        // Did the name change? (Name is treated as a special property)
        if self.name != old_node.name {
            changed_properties.insert("Name".to_string(), PropertyValue::String(self.name.clone()));
        }

        // Did the parent change?
        if self.parent != old_node.parent {
            if let Some(parent_uuid) = self.parent {
                changed_properties.insert("Parent".to_string(), PropertyValue::String(parent_uuid.to_string()));
            } else {
                changed_properties.insert("Parent".to_string(), PropertyValue::String("Workspace".to_string()));
            }
        }

        for (key, new_val) in &self.properties {
            if let Some(old_val) = old_node.properties.get(key) {
                if new_val != old_val {
                    changed_properties.insert(key.clone(), new_val.clone());
                }
            } else {
                // A new property was added
                changed_properties.insert(key.clone(), new_val.clone());
            }
        }

        if changed_properties.is_empty() {
            None
        } else {
            Some(InstancePatch {
                syncix_id: self.syncix_id,
                changed_properties,
            })
        }
    }
}
/// Result of target resolution (search by UUID / short UUID / name).
pub enum ResolveResult {
    One(Uuid),
    NotFound,
    /// List of (name, class_name, uuid)
    Ambiguous(Vec<(String, String, Uuid)>),
}

pub struct DataModel {
    instances: HashMap<Uuid, InstanceNode>,
    root_id: Uuid,
}

impl DataModel {
    pub fn new() -> Self {
        let root = InstanceNode::new("DataModel", "Game");
        let root_id = root.syncix_id;

        let mut instances = HashMap::new();
        instances.insert(root_id, root);

        Self { instances, root_id }
    }

    pub fn get_all_instances(&self) -> &HashMap<Uuid, InstanceNode> {
        &self.instances
    }


    pub fn get_instance(&self, id: &Uuid) -> Option<&InstanceNode> {
        self.instances.get(id)
    }

    pub fn get_mut_instance(&mut self, id: &Uuid) -> Option<&mut InstanceNode> {
        self.instances.get_mut(id)
    }


}

/// Thread-safe DataModel wrapper.
/// Every core service (watcher, HTTP server, ...) shares this structure.
pub type SharedDataModel = Arc<RwLock<DataModel>>;

pub fn create_shared_model() -> SharedDataModel {
    Arc::new(RwLock::new(DataModel::new()))
}

#[cfg(test)]
mod tests;
