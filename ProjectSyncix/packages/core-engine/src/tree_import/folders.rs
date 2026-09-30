//! Split out of tree_import.rs.

#[allow(unused_imports)]
use super::*;

pub(crate) enum Folder {
    Instance(TreeNode),
    Contents(Vec<TreeNode>),
}

pub(crate) fn read_folder(dir: &Path, tree: &mut Tree) -> Result<Folder, String> {
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
pub(crate) fn order_children(children: &mut [TreeNode], own: Option<&TreeNode>) {
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
