//! Reads a folder written in Syncix's own disk layout back into a tree that can be
//! created in Studio: `syncix import <folder>`.
//!
//! The layout is the one layout.rs writes, and the one the assetkit tool produces when it
//! pulls models out of a place:
//!
//! ```text
//! Tree/
//!   init.model.json        an instance with children: a folder plus init.<class>.json
//!   Trunk.part.json        an instance without children: <name>.<class>.json
//!   Spin.server.lua        a script's source; its properties in Spin.meta.json
//!   Label.txt              a StringValue's Value
//!   Words.csv              a LocalizationTable's Contents
//! ```
//!
//! Copying such a folder into a sync folder does not work and must not: the watcher reads
//! one file at a time, in whatever order the file system reports, and the identities in
//! the files belong to the place they came from — importing the same model twice would
//! collide. So the tree is read here in one go, every instance is given a NEW identity,
//! and references inside the tree are moved to the new identities (references pointing
//! out of it are cleared, as they point at objects this place may not have).
//!
//! The shape of the tree comes from the FOLDERS, not from the parent/children fields in
//! the files: the folders are what the user sees and edits, and a hand-moved file should
//! land where it was moved to.

use std::collections::BTreeMap;
use std::path::Path;

use uuid::Uuid;

use crate::model::PropertyValue;

/// An instance read from disk, with its children, ready to be created.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// The identity it will be created with (fresh, so a second import is a second copy).
    pub id: Uuid,
    /// The identity the file carried, only used to resolve references and child order.
    pub old_id: Option<Uuid>,
    pub class_name: String,
    pub name: String,
    pub properties: BTreeMap<String, PropertyValue>,
    pub attributes: BTreeMap<String, PropertyValue>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub children: Vec<TreeNode>,
}

/// What a folder held, and what could not be brought along.
#[derive(Debug, Default)]
pub struct Tree {
    pub roots: Vec<TreeNode>,
    /// Files that look like part of the layout but could not be read, with the reason.
    pub unreadable: Vec<String>,
    /// Unions left out: Studio does not let a plugin rebuild their geometry.
    pub unions: Vec<String>,
    /// References that pointed outside the folder and were cleared.
    pub outside_refs: usize,
}

impl Tree {
    pub fn count(&self) -> usize {
        fn walk(node: &TreeNode) -> usize {
            1 + node.children.iter().map(walk).sum::<usize>()
        }
        self.roots.iter().map(walk).sum()
    }
}

/// A class whose geometry only Studio can build; a plugin cannot recreate it from a file.
fn is_union(class_name: &str) -> bool {
    matches!(
        class_name,
        "UnionOperation" | "NegateOperation" | "IntersectOperation" | "PartOperation"
    )
}

/// Reads a folder (or a single data file) written in the Syncix layout.
pub fn read(path: &Path) -> Result<Tree, String> {
    let mut tree = Tree::default();
    if path.is_file() {
        match node_from_file(path, &mut tree) {
            Some(node) => tree.roots.push(node),
            None => return Err(format!("{} is not a Syncix data file.", path.display())),
        }
    } else if path.is_dir() {
        match read_folder(path, &mut tree)? {
            Folder::Instance(node) => tree.roots.push(node),
            // A folder that is not an instance of its own (no init file) is just a place
            // to keep things: what it holds arrives side by side.
            Folder::Contents(children) => tree.roots.extend(children),
        }
    } else {
        return Err(format!("{} was not found.", path.display()));
    }

    remap_references(&mut tree);
    Ok(tree)
}

enum Folder {
    Instance(TreeNode),
    Contents(Vec<TreeNode>),
}

fn read_folder(dir: &Path, tree: &mut Tree) -> Result<Folder, String> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{} could not be read: {}", dir.display(), e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();

    let folder_name = dir
        .file_name()
        .and_then(|f| f.to_str())
        .map(crate::layout::instance_name_in)
        .unwrap_or_default();

    // The folder's own data file, if it has one: init.<class>.json, or a container
    // script (init.server.lua and friends) whose settings live in init.meta.json.
    let mut own: Option<TreeNode> = None;
    let mut used: Vec<std::path::PathBuf> = Vec::new();
    for path in &entries {
        let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !file_name.starts_with("init.") || path.is_dir() {
            continue;
        }
        if file_name == "init.meta.json" {
            continue; // read below, with whatever it belongs to
        }
        if let Some(mut node) = node_from_file(path, tree) {
            if node.name.is_empty() {
                node.name = folder_name.clone();
            }
            own = Some(node);
            used.push(path.clone());
            break;
        }
    }
    if let (Some(node), Some(meta_path)) = (
        own.as_mut(),
        entries.iter().find(|p| p.file_name().and_then(|f| f.to_str()) == Some("init.meta.json")),
    ) {
        apply_meta(node, meta_path, tree);
        used.push(meta_path.clone());
    }

    // Everything else in the folder is a child.
    let mut children: Vec<TreeNode> = Vec::new();
    let mut metas: Vec<std::path::PathBuf> = Vec::new();
    for path in &entries {
        if used.contains(path) {
            continue;
        }
        if path.is_dir() {
            match read_folder(path, tree)? {
                Folder::Instance(node) => children.push(node),
                Folder::Contents(inner) => children.extend(inner),
            }
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if file_name.ends_with(".meta.json") {
            metas.push(path.clone());
            continue;
        }
        if file_name.starts_with("init.") {
            continue; // a second init file: the first one won
        }
        if let Some(node) = node_from_file(path, tree) {
            children.push(node);
        }
    }

    // A .meta.json carries the properties, attributes and tags of the file beside it.
    for meta_path in &metas {
        let Some(file_name) = meta_path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let owner_name = crate::layout::instance_name_in(file_name);
        let short = crate::layout::short_id_in(file_name);
        let owner = children.iter_mut().find(|c| {
            c.name == owner_name
                && short
                    .as_ref()
                    .map(|s| c.old_id.map(|u| u.to_string().starts_with(s)).unwrap_or(false))
                    .unwrap_or(true)
        });
        match owner {
            Some(node) => apply_meta(node, meta_path, tree),
            None => tree.unreadable.push(format!(
                "{}: nothing beside it is called {}",
                meta_path.display(),
                owner_name
            )),
        }
    }

    order_children(&mut children, own.as_ref());
    match own {
        Some(mut node) => {
            node.children = children;
            Ok(Folder::Instance(node))
        }
        None => Ok(Folder::Contents(children)),
    }
}

/// The order the tree had: the identities listed in the folder's own file. Anything the
/// list does not mention keeps the order the file names gave it.
fn order_children(children: &mut [TreeNode], own: Option<&TreeNode>) {
    let Some(order) = own.and_then(|n| n.properties.get("__children_order")) else {
        return;
    };
    let PropertyValue::String(order) = order else {
        return;
    };
    let wanted: Vec<&str> = order.split(',').collect();
    children.sort_by_key(|c| {
        c.old_id
            .and_then(|u| wanted.iter().position(|w| *w == u.to_string()))
            .unwrap_or(usize::MAX)
    });
}

/// One file -> one instance. None for a file that is not part of the layout.
fn node_from_file(path: &Path, tree: &mut Tree) -> Option<TreeNode> {
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

fn new_node(class: &str, name: &str, old_id: Option<Uuid>, source: Option<String>) -> TreeNode {
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

fn read_text(path: &Path, tree: &mut Tree) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) => {
            tree.unreadable.push(format!("{}: {}", path.display(), e));
            None
        }
    }
}

fn apply_meta(node: &mut TreeNode, meta_path: &Path, tree: &mut Tree) {
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
fn remap_references(tree: &mut Tree) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), text).unwrap();
    }

    fn temp(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("syncix_tree_import_{}_{}", label, Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A model folder as layout writes it: a folder with its own file, a leaf, a script
    /// with settings beside it, a StringValue, attributes and tags.
    fn write_model(root: &Path) {
        let model = root.join("Apple");
        write(
            &model,
            "init.model.json",
            r#"{ "class_name": "Model", "name": "Apple",
                 "syncix_id": "11111111-1111-4111-8111-111111111111",
                 "properties": { "PrimaryPart": { "Ref": "22222222-2222-4222-8222-222222222222" } },
                 "children": ["33333333-3333-4333-8333-333333333333",
                              "22222222-2222-4222-8222-222222222222"],
                 "tags": ["Fruit"] }"#,
        );
        write(
            &model,
            "Trunk.part.json",
            r#"{ "class_name": "Part", "name": "Trunk",
                 "syncix_id": "22222222-2222-4222-8222-222222222222",
                 "properties": { "Anchored": { "Boolean": true },
                                 "Nowhere": { "Ref": "99999999-9999-4999-8999-999999999999" } },
                 "attributes": { "Weight": { "Number": 2.0 } } }"#,
        );
        write(
            &model,
            "Leaf.part.json",
            r#"{ "class_name": "Part", "name": "Leaf",
                 "syncix_id": "33333333-3333-4333-8333-333333333333" }"#,
        );
        write(&model, "Spin.server.lua", "print('spin')\n");
        write(&model, "Spin.meta.json", r#"{ "properties": { "Disabled": { "Boolean": true } }, "tags": ["Spinner"] }"#);
        write(&model, "Label.txt", "Apple");
    }

    #[test]
    fn a_model_folder_is_read_with_its_scripts_settings_and_order() {
        let root = temp("model");
        write_model(&root);
        let tree = read(&root.join("Apple")).unwrap();

        assert_eq!(tree.roots.len(), 1);
        let apple = &tree.roots[0];
        assert_eq!((apple.class_name.as_str(), apple.name.as_str()), ("Model", "Apple"));
        assert_eq!(apple.tags, vec!["Fruit"]);
        assert_eq!(apple.children.len(), 4, "leaf, trunk, script and the StringValue");

        // The order the tree had wins over the alphabet: Leaf was listed first.
        let names: Vec<&str> = apple.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names[0], "Leaf");
        assert_eq!(names[1], "Trunk");

        let script = apple.children.iter().find(|c| c.name == "Spin").unwrap();
        assert_eq!(script.class_name, "Script");
        assert_eq!(script.source.as_deref(), Some("print('spin')\n"));
        assert_eq!(script.properties.get("Disabled"), Some(&PropertyValue::Boolean(true)));
        assert_eq!(script.tags, vec!["Spinner"]);

        let label = apple.children.iter().find(|c| c.name == "Label").unwrap();
        assert_eq!(label.class_name, "StringValue");
        assert_eq!(label.properties.get("Value"), Some(&PropertyValue::String("Apple".to_string())));

        let trunk = apple.children.iter().find(|c| c.name == "Trunk").unwrap();
        assert_eq!(trunk.attributes.get("Weight"), Some(&PropertyValue::Number(2.0)));

        // A reference inside the tree follows it; one pointing out of it is cleared.
        assert_eq!(
            apple.properties.get("PrimaryPart"),
            Some(&PropertyValue::Ref(trunk.id.to_string())),
            "PrimaryPart must point at the Trunk that was just read"
        );
        assert_eq!(trunk.properties.get("Nowhere"), Some(&PropertyValue::Ref(String::new())));
        assert_eq!(tree.outside_refs, 1);
        // The child order is bookkeeping, not a property Studio should hear about.
        assert!(!apple.properties.contains_key("__children_order"));
    }

    /// The same folder twice must be two separate copies; identities are never reused.
    #[test]
    fn importing_the_same_folder_twice_gives_two_copies() {
        let root = temp("twice");
        write_model(&root);
        let first = read(&root.join("Apple")).unwrap();
        let second = read(&root.join("Apple")).unwrap();

        let ids = |tree: &Tree| {
            let mut out = Vec::new();
            fn walk(node: &TreeNode, out: &mut Vec<Uuid>) {
                out.push(node.id);
                for child in &node.children {
                    walk(child, out);
                }
            }
            for root in &tree.roots {
                walk(root, &mut out);
            }
            out
        };
        let (a, b) = (ids(&first), ids(&second));
        assert_eq!(a.len(), b.len());
        assert!(a.iter().all(|id| !b.contains(id)), "the two imports must not share an identity");
        assert!(a.iter().all(|id| Some(*id) != first.roots[0].old_id), "the identity in the file is not reused");
    }

    /// A union cannot be rebuilt from a file; it is left out and named, not turned into a box.
    #[test]
    fn unions_are_left_out_and_named() {
        let root = temp("union");
        let model = root.join("Door");
        write(&model, "init.model.json", r#"{ "class_name": "Model", "name": "Door" }"#);
        write(&model, "Frame.unionoperation.json", r#"{ "class_name": "UnionOperation", "name": "Frame" }"#);
        write(&model, "Handle.part.json", r#"{ "class_name": "Part", "name": "Handle" }"#);

        let tree = read(&model).unwrap();
        assert_eq!(tree.unions, vec!["Frame"]);
        let door = &tree.roots[0];
        assert_eq!(door.children.len(), 1);
        assert_eq!(door.children[0].name, "Handle");
        assert_eq!(tree.count(), 2);
    }

    /// A folder with no file of its own is not an instance: what it holds arrives as roots.
    #[test]
    fn a_folder_without_its_own_file_yields_its_contents() {
        let root = temp("bare");
        let bare = root.join("Models");
        write(&bare, "Rock.meshpart.json", r#"{ "class_name": "MeshPart", "name": "Rock" }"#);
        write(&bare, "Stick.part.json", r#"{ "class_name": "Part", "name": "Stick" }"#);

        let tree = read(&bare).unwrap();
        assert_eq!(tree.roots.len(), 2);
        assert_eq!(tree.count(), 2);
    }
}
