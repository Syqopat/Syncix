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

mod folders;
mod nodes;

pub(crate) use folders::*;
pub(crate) use nodes::*;

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

