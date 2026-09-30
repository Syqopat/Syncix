//! DataModel methods split out of model.rs.

#[allow(unused_imports)]
use super::*;

impl DataModel {
    /// Adds a new instance. Performs a conflict check.
    pub fn upsert_instance(&mut self, mut incoming: InstanceNode) -> Result<(), ModelError> {
        let id = incoming.syncix_id;
        let mut previous_parent = None;
        if let Some(existing) = self.instances.get(&id) {
            // Conflict resolution: reject incoming data that is older
            if incoming.last_updated < existing.last_updated {
                return Err(ModelError::VersionConflict {
                    current: existing.last_updated,
                    incoming: incoming.last_updated,
                });
            }
            previous_parent = existing.parent;
            // The model keeps children lists current as children come and go; a stored
            // copy of the list may be older, so the children known here are kept.
            for c in &existing.children {
                if !incoming.children.contains(c) {
                    incoming.children.push(*c);
                }
            }
        }
        if let Some(parent_id) = incoming.parent {
            if !self.instances.contains_key(&parent_id) {
                return Err(ModelError::InvalidParent(parent_id));
            }
        }

        // Only children that exist and point back here are kept. A folder restored from
        // the trash listed children that were never restored, and verify then reported
        // children that do not exist. A child arriving later adds itself below.
        incoming
            .children
            .retain(|c| self.instances.get(c).is_some_and(|n| n.parent == Some(id)));

        // Moved: leave the old parent's list.
        if previous_parent != incoming.parent {
            if let Some(old) = previous_parent.and_then(|p| self.instances.get_mut(&p)) {
                old.children.retain(|c| *c != id);
            }
        }
        // If it has a parent, add it to the parent's children list
        if let Some(parent) = incoming.parent.and_then(|p| self.instances.get_mut(&p)) {
            if !parent.children.contains(&id) {
                parent.children.push(id);
            }
        }

        self.instances.insert(id, incoming);
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

    /// Puts back what the core created but Studio had not applied when its tree arrived
    /// (FULL_SYNC); `previous` is the model before the rebuild. An instance comes back only
    /// under a parent the new model has, parents before children, so a whole subtree on
    /// its way survives. For a property change on its way the core's value wins over the
    /// tree's older one. Returns the instances put back.
    pub fn carry_over(&mut self, previous: &DataModel, in_flight: &crate::transport::InFlight) -> Vec<Uuid> {
        let mut kept = Vec::new();
        let mut pending: Vec<Uuid> = in_flight
            .creates
            .iter()
            .copied()
            .filter(|id| !self.instances.contains_key(id))
            .collect();
        loop {
            let mut progressed = false;
            let mut waiting = Vec::new();
            for id in pending {
                let Some(node) = previous.instances.get(&id) else { continue };
                if node.parent.is_some_and(|p| !self.instances.contains_key(&p)) {
                    waiting.push(id);
                    continue;
                }
                let mut copy = node.clone();
                copy.children.clear();
                if self.upsert_instance(copy).is_ok() {
                    kept.push(id);
                    progressed = true;
                }
            }
            pending = waiting;
            if !progressed || pending.is_empty() {
                break;
            }
        }

        // A move on its way: the core's parent wins over the tree's older one.
        for id in &in_flight.reparents {
            let Some(parent) = previous.instances.get(id).map(|n| n.parent) else { continue };
            let Some(current) = self.instances.get(id).map(|n| n.parent) else { continue };
            let parent_exists = match parent {
                Some(p) => self.instances.contains_key(&p),
                None => true,
            };
            if current != parent && parent_exists {
                let _ = self.reparent(id, parent);
            }
        }

        for (id, property) in &in_flight.properties {
            let (Some(old), Some(node)) = (previous.instances.get(id), self.instances.get_mut(id)) else {
                continue;
            };
            match property.as_str() {
                // Name and a script's source live outside the property map; carrying only
                // the map let a rename or an edit on its way be undone.
                "Name" => node.name = old.name.clone(),
                "Source" => {
                    node.source = old.source.clone();
                    if let Some(value) = old.properties.get(property) {
                        node.properties.insert(property.clone(), value.clone());
                    }
                }
                _ => {
                    if let Some(value) = old.properties.get(property) {
                        node.properties.insert(property.clone(), value.clone());
                    }
                }
            }
        }

        for (id, name) in &in_flight.attributes {
            let (Some(old), Some(node)) = (previous.instances.get(id), self.instances.get_mut(id)) else {
                continue;
            };
            match old.attributes.get(name) {
                Some(value) => {
                    node.attributes.insert(name.clone(), value.clone());
                }
                // Deleting the attribute is what is on its way.
                None => {
                    node.attributes.remove(name);
                }
            }
        }

        for id in &in_flight.tags {
            if let (Some(old), Some(node)) = (previous.instances.get(id), self.instances.get_mut(id)) {
                node.tags = old.tags.clone();
            }
        }

        // A deletion on its way: the core already removed the instance, the tree still
        // has it. Kept, it would come back into the model and onto disk.
        for id in &in_flight.destroys {
            if !previous.instances.contains_key(id) && self.instances.contains_key(id) {
                self.remove_instance(id);
            }
        }
        kept
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

    /// Gives an object a new identity. In Team Create two people's plugins can name the
    /// same new object differently; the plugins settle on one and the core follows. The
    /// parent's children list, the children's parent and every reference property that
    /// pointed at the old identity are carried over. Fails if the new identity is taken.
    pub fn rekey(&mut self, old: &Uuid, new: &Uuid) -> Result<(), ModelError> {
        if old == new {
            return Ok(());
        }
        if self.instances.contains_key(new) {
            return Err(ModelError::IdentityTaken(*new));
        }
        let mut node = self.instances.remove(old).ok_or(ModelError::InstanceNotFound(*old))?;
        node.syncix_id = *new;
        if let Some(parent) = node.parent.and_then(|p| self.instances.get_mut(&p)) {
            for c in parent.children.iter_mut() {
                if c == old {
                    *c = *new;
                }
            }
        }
        for child in &node.children {
            if let Some(c) = self.instances.get_mut(child) {
                c.parent = Some(*new);
            }
        }
        let old_text = old.to_string();
        for other in self.instances.values_mut() {
            for value in other.properties.values_mut() {
                if matches!(value, PropertyValue::Ref(r) if *r == old_text) {
                    *value = PropertyValue::Ref(new.to_string());
                }
            }
        }
        for value in node.properties.values_mut() {
            if matches!(value, PropertyValue::Ref(r) if *r == old_text) {
                *value = PropertyValue::Ref(new.to_string());
            }
        }
        if self.root_id == *old {
            self.root_id = *new;
        }
        self.instances.insert(*new, node);
        Ok(())
    }
}
