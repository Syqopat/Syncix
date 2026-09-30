//! Split out of tree_import.rs.

#[allow(unused_imports)]
use super::*;

/// One file -> one instance. None for a file that is not part of the layout.
pub(crate) fn node_from_file(path: &Path, tree: &mut Tree) -> Option<TreeNode> {
    let file_name = path.file_name()?.to_str()?;
    let name = crate::layout::instance_name_in(file_name);

    // A script: its source is the file, its settings the .meta.json beside it.
    if let Some(class) = crate::layout::script_class_of_file(file_name) {
        let source = read_text(path, tree)?;
        return Some(new_node(class, &name, None, Some(source)));
    }
    if file_name.ends_with(".txt") {
        let text = read_text(path, tree)?;
        let mut node = new_node("StringValue", &name, None, None);
        node.properties.insert("Value".to_string(), PropertyValue::String(text));
        return Some(node);
    }
    if file_name.ends_with(".csv") {
        let text = read_text(path, tree)?;
        return match crate::localization::csv_to_json(&text) {
            Ok(json) => {
                let mut node = new_node("LocalizationTable", &name, None, None);
                node.properties.insert("Contents".to_string(), PropertyValue::String(json));
                Some(node)
            }
            Err(e) => {
                tree.unreadable.push(format!("{}: {}", path.display(), e));
                None
            }
        };
    }
    if !file_name.ends_with(".json") || file_name.ends_with(".meta.json") {
        return None;
    }

    let text = read_text(path, tree)?;
    let stored: crate::model::InstanceNode = match serde_json::from_str(&text) {
        Ok(node) => node,
        Err(e) => {
            tree.unreadable.push(format!("{}: {}", path.display(), e));
            return None;
        }
    };
    if is_union(&stored.class_name) {
        tree.unions.push(stored.name.clone());
        return None;
    }

    let file_class = crate::layout::class_in_file_name(file_name);
    let class = if stored.class_name.is_empty() {
        file_class?
    } else {
        // The file name is the one the user sees and can correct.
        file_class.filter(|c| !c.eq_ignore_ascii_case(&stored.class_name)).unwrap_or(&stored.class_name)
    };
    let stored_name = if stored.name.is_empty() { name.clone() } else { stored.name.clone() };
    let mut node = new_node(class, &stored_name, Some(stored.syncix_id), stored.source.clone());
    node.properties = stored.properties.clone();
    node.attributes = stored.attributes.clone();
    node.tags = stored.tags.clone();
    // Kept only to order the children below; never sent to Studio.
    if !stored.children.is_empty() {
        let order = stored.children.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
        node.properties.insert("__children_order".to_string(), PropertyValue::String(order));
    }
    Some(node)
}

pub(crate) fn new_node(class: &str, name: &str, old_id: Option<Uuid>, source: Option<String>) -> TreeNode {
    TreeNode {
        id: Uuid::new_v4(),
        old_id: old_id.filter(|u| !u.is_nil()),
        class_name: class.to_string(),
        name: name.to_string(),
        properties: BTreeMap::new(),
        attributes: BTreeMap::new(),
        tags: Vec::new(),
        source,
        children: Vec::new(),
    }
}

pub(crate) fn read_text(path: &Path, tree: &mut Tree) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) => {
            tree.unreadable.push(format!("{}: {}", path.display(), e));
            None
        }
    }
}

pub(crate) fn apply_meta(node: &mut TreeNode, meta_path: &Path, tree: &mut Tree) {
    let Some(text) = read_text(meta_path, tree) else {
        return;
    };
    match serde_json::from_str::<crate::layout::ScriptMeta>(&text) {
        Ok(meta) => {
            node.properties.extend(meta.properties);
            node.attributes.extend(meta.attributes);
            for tag in meta.tags {
                if !node.tags.contains(&tag) {
                    node.tags.push(tag);
                }
            }
        }
        Err(e) => tree.unreadable.push(format!("{}: {}", meta_path.display(), e)),
    }
}

/// References (PrimaryPart, Motor6D.Part0, an ObjectValue's Value) point at identities
/// from the place the folder came from. Inside the tree they are moved to the new
/// identities; pointing out of it, they are cleared — the object they name is not part of
/// what is being imported, and a stale identity would silently point at nothing.
pub(crate) fn remap_references(tree: &mut Tree) {
    let mut moved: BTreeMap<String, String> = BTreeMap::new();
    fn collect(node: &TreeNode, moved: &mut BTreeMap<String, String>) {
        if let Some(old) = node.old_id {
            moved.insert(old.to_string(), node.id.to_string());
        }
        for child in &node.children {
            collect(child, moved);
        }
    }
    for root in &tree.roots {
        collect(root, &mut moved);
    }

    fn apply(node: &mut TreeNode, moved: &BTreeMap<String, String>, cleared: &mut usize) {
        node.properties.remove("__children_order");
        for values in [&mut node.properties, &mut node.attributes] {
            for value in values.values_mut() {
                if let PropertyValue::Ref(target) = value {
                    if target.is_empty() {
                        continue;
                    }
                    match moved.get(target.as_str()) {
                        Some(new) => *value = PropertyValue::Ref(new.clone()),
                        None => {
                            *value = PropertyValue::Ref(String::new());
                            *cleared += 1;
                        }
                    }
                }
            }
        }
        for child in &mut node.children {
            apply(child, moved, cleared);
        }
    }
    let mut cleared = 0;
    for root in &mut tree.roots {
        apply(root, &moved, &mut cleared);
    }
    tree.outside_refs = cleared;
}
