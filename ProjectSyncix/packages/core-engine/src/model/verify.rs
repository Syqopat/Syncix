//! DataModel methods split out of model.rs.

#[allow(unused_imports)]
use super::*;

impl DataModel {
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
