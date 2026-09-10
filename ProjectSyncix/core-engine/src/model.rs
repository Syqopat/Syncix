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

    /// Adds a new instance. Performs a conflict check.
    pub fn upsert_instance(&mut self, incoming: InstanceNode) -> Result<(), ModelError> {
        if let Some(existing) = self.instances.get(&incoming.syncix_id) {
            // Conflict resolution: reject incoming data that is older
            if incoming.last_updated < existing.last_updated {
                return Err(ModelError::VersionConflict {
                    current: existing.last_updated,
                    incoming: incoming.last_updated,
                });
            }
        }

        // If it has a parent, add it to the parent's children list
        if let Some(parent_id) = incoming.parent {
            if let Some(parent) = self.instances.get_mut(&parent_id) {
                if !parent.children.contains(&incoming.syncix_id) {
                    parent.children.push(incoming.syncix_id);
                }
            } else {
                return Err(ModelError::InvalidParent(parent_id));
            }
        }

        self.instances.insert(incoming.syncix_id, incoming);
        Ok(())
    }

    /// Collects every descendant of a node (the node itself excluded).
    fn collect_descendants(&self, id: &Uuid) -> Vec<Uuid> {
        let mut result = Vec::new();
        let mut stack: Vec<Uuid> = self
            .instances
            .get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        // Cycle protection
        let mut guard = 0;
        while let Some(cur) = stack.pop() {
            result.push(cur);
            if let Some(node) = self.instances.get(&cur) {
                stack.extend(node.children.iter().copied());
            }
            guard += 1;
            if guard > 100_000 {
                break;
            }
        }
        result
    }

    /// Deletes an object and ALL of its descendants (cascade). Destroy() in Studio removes
    /// the subtree too, so the core model has to do the same; otherwise
    /// orphaned (dangling) children stay in the model and the state drifts apart.
    pub fn remove_instance(&mut self, id: &Uuid) -> Option<InstanceNode> {
        // Delete the descendants first
        for d in self.collect_descendants(id) {
            self.instances.remove(&d);
        }
        // Then delete the node itself and remove it from its parent's children list
        if let Some(instance) = self.instances.remove(id) {
            if let Some(parent_id) = instance.parent {
                if let Some(parent) = self.instances.get_mut(&parent_id) {
                    parent.children.retain(|&child_id| child_id != *id);
                }
            }
            Some(instance)
        } else {
            None
        }
    }

    /// Moves an object to a new parent. Removes it from the old parent's children list,
    /// adds it to the new parent's list and updates the node's parent field.
    /// Returns: (old_parent, new_parent) — for the VS Code notification.
    pub fn reparent(
        &mut self,
        id: &Uuid,
        new_parent: Option<Uuid>,
    ) -> Result<(Option<Uuid>, Option<Uuid>), ModelError> {
        let old_parent = self
            .instances
            .get(id)
            .ok_or(ModelError::InstanceNotFound(*id))?
            .parent;

        if old_parent == new_parent {
            return Ok((old_parent, new_parent));
        }

        // Remove from the old parent's children list
        if let Some(op) = old_parent {
            if let Some(parent) = self.instances.get_mut(&op) {
                parent.children.retain(|c| c != id);
            }
        }

        // Update the node's parent field
        if let Some(inst) = self.instances.get_mut(id) {
            inst.parent = new_parent;
            inst.last_updated = Utc::now().timestamp_millis();
        }

        // Add to the new parent's children list
        if let Some(np) = new_parent {
            if let Some(parent) = self.instances.get_mut(&np) {
                if !parent.children.contains(id) {
                    parent.children.push(*id);
                }
            }
        }

        Ok((old_parent, new_parent))
    }

    pub fn get_instance(&self, id: &Uuid) -> Option<&InstanceNode> {
        self.instances.get(id)
    }

    pub fn get_mut_instance(&mut self, id: &Uuid) -> Option<&mut InstanceNode> {
        self.instances.get_mut(id)
    }

    /// Finds instances by name (case-insensitive, exact match).
    /// So the CLI and commands can use a name instead of a UUID.
    pub fn find_by_name(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, node)| node.class_name != "DataModel" && node.name.to_lowercase() == lower)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Returns the children of a given parent with the given name (case-insensitive).
    fn children_named(&self, parent: &Uuid, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        if let Some(p) = self.instances.get(parent) {
            p.children
                .iter()
                .filter(|cid| {
                    self.instances
                        .get(cid)
                        .map(|c| c.name.to_lowercase() == lower)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the root (service) nodes with the given name.
    fn roots_named(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, n)| {
                n.parent.is_none()
                    && n.class_name != "DataModel"
                    && n.name.to_lowercase() == lower
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Resolves a dotted path: "Workspace.Model.Part" or "game.Workspace.Baseplate".
    pub fn resolve_path(&self, path: &str) -> ResolveResult {
        let mut segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }
        // Optional "game" prefix
        if segments[0].eq_ignore_ascii_case("game") {
            segments.remove(0);
        }
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }

        // First segment: the root service
        let roots = self.roots_named(segments[0]);
        let mut current = match roots.len() {
            1 => roots[0],
            0 => return ResolveResult::NotFound,
            _ => {
                return ResolveResult::Ambiguous(
                    roots
                        .iter()
                        .filter_map(|id| {
                            self.instances
                                .get(id)
                                .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                        })
                        .collect(),
                )
            }
        };

        // Walk the remaining segments through the children
        for seg in &segments[1..] {
            let matches = self.children_named(&current, seg);
            current = match matches.len() {
                1 => matches[0],
                0 => return ResolveResult::NotFound,
                _ => {
                    return ResolveResult::Ambiguous(
                        matches
                            .iter()
                            .filter_map(|id| {
                                self.instances
                                    .get(id)
                                    .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                            })
                            .collect(),
                    )
                }
            };
        }

        ResolveResult::One(current)
    }

    /// Resolves a command target. Tries, in order:
    /// 1. A dotted path (Workspace.Model.Part)
    /// 2. A full UUID
    /// 3. A short UUID prefix (at least 6 characters, e.g. "d8d0cf78")
    /// 4. A name (exact match; if there is a single result)
    ///
    /// On ambiguity the candidates are returned so the client can give a meaningful error.
    pub fn resolve_target(&self, target: &str) -> ResolveResult {
        // Treat it as a path if it contains a dot (a UUID contains '-', never '.')
        if target.contains('.') {
            return self.resolve_path(target);
        }
        if let Ok(u) = Uuid::parse_str(target) {
            return if self.instances.contains_key(&u) {
                ResolveResult::One(u)
            } else {
                ResolveResult::NotFound
            };
        }

        let is_hexish = target.len() >= 6
            && target.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if is_hexish {
            if let Some(u) = self.find_by_short_uuid(&target.to_lowercase()) {
                return ResolveResult::One(u);
            }
        }

        let matches = self.find_by_name(target);
        match matches.len() {
            0 => ResolveResult::NotFound,
            1 => ResolveResult::One(matches[0]),
            _ => ResolveResult::Ambiguous(
                matches
                    .iter()
                    .filter_map(|id| {
                        self.instances
                            .get(id)
                            .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                    })
                    .collect(),
            ),
        }
    }

    pub fn find_by_short_uuid(&self, short_uuid: &str) -> Option<Uuid> {
        for id in self.instances.keys() {
            if id.to_string().starts_with(short_uuid) {
                return Some(*id);
            }
        }
        None
    }

    /// Data integrity and resync (consistency check)
    /// Scans the tree for integrity. Detects orphaned (dangling) parent or child references.
    pub fn verify_consistency(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (id, node) in &self.instances {
            // Check Parent
            if let Some(parent_id) = node.parent {
                if !self.instances.contains_key(&parent_id) {
                    errors.push(format!(
                        "Node {} ({}) points at parent {}, but that parent does not exist.",
                        node.name, id, parent_id
                    ));
                } else {
                    let parent_node = self.instances.get(&parent_id).unwrap();
                    if !parent_node.children.contains(id) {
                        errors.push(format!("Node {} points at a parent, but parent {} does not list this id among its children.", id, parent_id));
                    }
                }
            }

            // Check Children
            for child_id in &node.children {
                if !self.instances.contains_key(child_id) {
                    errors.push(format!(
                        "Node {} ({}) lists child {}, but that child does not exist.",
                        node.name, id, child_id
                    ));
                } else {
                    let child_node = self.instances.get(child_id).unwrap();
                    if child_node.parent != Some(*id) {
                        errors.push(format!(
                            "Child {} ({}) points at a different parent.",
                            child_node.name, child_id
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Thread-safe DataModel wrapper.
/// Every core service (watcher, HTTP server, ...) shares this structure.
pub type SharedDataModel = Arc<RwLock<DataModel>>;

pub fn create_shared_model() -> SharedDataModel {
    Arc::new(RwLock::new(DataModel::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: adds a named node under the parent and returns its id.
    fn add(model: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut node = InstanceNode::new(class, name);
        node.parent = parent;
        let id = node.syncix_id;
        model.upsert_instance(node).expect("upsert basarisiz");
        id
    }

    /// When a node is deleted its WHOLE subtree must go too (Destroy() behaviour in Studio).
    #[test]
    fn test_cascade_delete_removes_descendants() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let folder = add(&mut m, "Folder", "Container", Some(ws));
        let part = add(&mut m, "Part", "Box", Some(folder));
        let decal = add(&mut m, "Decal", "Doku", Some(part));

        assert!(m.remove_instance(&folder).is_some());

        assert!(m.get_instance(&folder).is_none(), "the folder must be deleted");
        assert!(m.get_instance(&part).is_none(), "the child must be deleted too");
        assert!(m.get_instance(&decal).is_none(), "the grandchild must be deleted too");
        assert!(m.get_instance(&ws).is_some(), "ebeveyn durmali");
        assert!(
            !m.get_instance(&ws).unwrap().children.contains(&folder),
            "ebeveynin children listesi temizlenmeli"
        );
    }

    /// Move: it must leave the old parent, join the new one, and the model must stay consistent.
    #[test]
    fn test_reparent_updates_both_parents() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let a = add(&mut m, "Folder", "A", Some(ws));
        let b = add(&mut m, "Folder", "B", Some(ws));
        let part = add(&mut m, "Part", "Box", Some(a));

        let (old, new) = m.reparent(&part, Some(b)).expect("reparent basarisiz");
        assert_eq!(old, Some(a));
        assert_eq!(new, Some(b));
        assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
        assert!(!m.get_instance(&a).unwrap().children.contains(&part));
        assert!(m.get_instance(&b).unwrap().children.contains(&part));
        assert!(m.verify_consistency().is_ok(), "model tutarli kalmali");
    }

    /// IDENTITY RULE: a UUID is generated only at CREATE time. A rename,
    /// a move or a repeated FULL_SYNC must NEVER change it.
    /// If this rule breaks, the two sides take the same object for two different objects and
    /// sync silently duplicates it; that is why each case is tested separately.
    #[test]
    fn uuid_survives_rename() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let part = add(&mut m, "Part", "OldName", Some(ws));

        m.get_mut_instance(&part).unwrap().name = "NewName".to_string();

        assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
        match m.resolve_target("NewName") {
            ResolveResult::One(u) => assert_eq!(u, part, "the new name must resolve to the same UUID"),
            _ => panic!("renamed object not found"),
        }
        assert!(matches!(m.resolve_target("OldName"), ResolveResult::NotFound));
    }

    #[test]
    fn uuid_survives_move() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let a = add(&mut m, "Folder", "A", Some(ws));
        let b = add(&mut m, "Folder", "B", Some(ws));
        let part = add(&mut m, "Part", "Box", Some(a));

        m.reparent(&part, Some(b)).expect("reparent failed");

        assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
        assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
    }

    /// FULL_SYNC resends the whole tree on every reconnect.
    /// A node arriving with the same UUID must update the existing object, not create a new one.
    #[test]
    fn repeated_full_sync_does_not_duplicate() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let part = add(&mut m, "Part", "Box", Some(ws));
        let prior_count = m.get_instance(&ws).unwrap().children.len();

        // Studio reconnected: same UUID, arriving again with an updated name.
        let mut again = InstanceNode::new("Part", "BoxNewName");
        again.syncix_id = part;
        again.parent = Some(ws);
        m.upsert_instance(again).expect("repeat upsert failed");

        assert_eq!(
            m.get_instance(&ws).unwrap().children.len(),
            prior_count,
            "the same UUID must not create a second child"
        );
        assert_eq!(m.get_instance(&part).unwrap().name, "BoxNewName");
        assert!(m.verify_consistency().is_ok());
    }

    /// Target resolution: dotted path, short UUID and name.
    #[test]
    fn test_resolve_target_path_shortuuid_and_name() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let folder = add(&mut m, "Folder", "Decor", Some(ws));
        let part = add(&mut m, "Part", "Pillar", Some(folder));

        // By path
        match m.resolve_target("Workspace.Decor.Pillar") {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("path could not be resolved"),
        }
        // By short UUID
        let short = &part.to_string()[0..8];
        match m.resolve_target(short) {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("short uuid could not be resolved"),
        }
        // By name (single match)
        match m.resolve_target("Pillar") {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("the name could not be resolved"),
        }
        // Non-existent target
        assert!(matches!(m.resolve_target("NonExistent"), ResolveResult::NotFound));
    }

    /// With two siblings of the same name, name resolution must be ambiguous (never pick the wrong one).
    #[test]
    fn test_resolve_target_ambiguous_name() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let _p1 = add(&mut m, "Part", "Box", Some(ws));
        let _p2 = add(&mut m, "Part", "Box", Some(ws));

        match m.resolve_target("Box") {
            ResolveResult::Ambiguous(list) => assert_eq!(list.len(), 2),
            _ => panic!("ambiguity should have been detected"),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_conflict_resolution() {
        let mut model = DataModel::new();
        let mut node = InstanceNode::new("Part", "TestPart");
        let id = node.syncix_id;

        // The first insert must succeed
        assert!(model.upsert_instance(node.clone()).is_ok());

        // Try an update with an older version (conflict)
        node.last_updated -= 1000;
        node.name = "OldName".to_string();
        let result = model.upsert_instance(node.clone());
        assert!(matches!(result, Err(ModelError::VersionConflict { .. })));

        // Check that the name did not change
        assert_eq!(model.get_instance(&id).unwrap().name, "TestPart");

        // Update with a newer version
        node.last_updated += 2000;
        node.name = "NewName".to_string();
        assert!(model.upsert_instance(node).is_ok());
        assert_eq!(model.get_instance(&id).unwrap().name, "NewName");
    }

    #[tokio::test]
    async fn test_thread_safety() {
        let shared_model = create_shared_model();
        let node = InstanceNode::new("Part", "ConcurrentPart");
        let id = node.syncix_id;

        // Thread 1: inserts
        let model_clone1 = shared_model.clone();
        let node_clone = node.clone();
        let t1 = tokio::spawn(async move {
            let mut lock = model_clone1.write().await;
            lock.upsert_instance(node_clone).unwrap();
        });

        // Thread 2: reads (after waiting for thread 1 to finish)
        let model_clone2 = shared_model.clone();
        let t2 = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let lock = model_clone2.read().await;
            assert!(lock.get_instance(&id).is_some());
        });

        let _ = tokio::join!(t1, t2);
    }
}
