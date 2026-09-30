//! Bringing a folder, a tree or Roblox XML into the place.

use std::io::Read;
use std::path::PathBuf;

use super::*;

/// Brings a .rbxmx / .rbxlx file into the tree (the last item Rojo had and Syncix lacked).
///
/// For every node a CREATE_INSTANCE is sent first, then its properties. Top-down
/// creation order matters: a child cannot be sent before its parent exists.
/// `syncix import <file.rbxmx | folder | file.json> [parent]`.
///
/// A folder (or a single data file) is read as Syncix's own layout — the shape assetkit
/// writes models in, and the shape the sync folder itself has. Anything else is read as
/// Roblox XML.
pub(crate) fn import_target(cli_args: &[String]) -> i32 {
    let Some(target) = cli_args.get(1) else {
        report_error("Usage: syncix import <file.rbxmx | folder | file.json> [parent]");
        return 1;
    };
    let path = PathBuf::from(target);
    if path.is_dir() || target.to_ascii_lowercase().ends_with(".json") {
        import_tree(cli_args)
    } else {
        import_rbxmx(cli_args)
    }
}

/// Creates a tree written in Syncix's layout in the place. Every instance is created with
/// a NEW identity, so the same folder can be imported twice and gives two copies.
pub(crate) fn import_tree(cli_args: &[String]) -> i32 {
    let target = cli_args.get(1).cloned().unwrap_or_default();
    let Some(port) = require_core() else { return 1 };
    let parent_ref = cli_args.get(2).cloned().unwrap_or_else(|| "Workspace".to_string());

    let tree = match crate::tree_import::read(&PathBuf::from(&target)) {
        Ok(tree) => tree,
        Err(e) => {
            report_error(&e);
            return 1;
        }
    };
    let total = tree.count();
    if total == 0 {
        report_error(&format!("{} holds no instances Syncix can read.", target));
        for line in tree.unreadable.iter().take(5) {
            print_dim(&format!("  {}", line));
        }
        return 1;
    }

    // Resolved ONCE, before anything is created: looked up again for every root it would
    // become ambiguous as soon as the import created a second object of that name.
    let parent_id = match fetch_json(port, &format!("/model/object?target={}", url_encode(&parent_ref))) {
        Some(o) => match o.get("syncix_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                let reason = o.get("error").and_then(|e| e.as_str()).unwrap_or("not found");
                report_error(&format!("Parent {}: {}", parent_ref, reason));
                return 1;
            }
        },
        None => return 1,
    };

    print_info(&format!("Importing {} instance(s) into {}", total, parent_ref));

    fn create(port: u16, node: &crate::tree_import::TreeNode, parent: &str, created: &mut usize, failed: &mut usize) {
        let properties: serde_json::Map<String, serde_json::Value> = node
            .properties
            .iter()
            .map(|(name, value)| (name.clone(), crate::values::pv_to_wire(value)))
            .collect();
        let attributes: serde_json::Map<String, serde_json::Value> = node
            .attributes
            .iter()
            .map(|(name, value)| (name.clone(), crate::values::pv_to_wire(value)))
            .collect();
        let mut data = serde_json::json!({
            "id": node.id.to_string(),
            "className": node.class_name,
            "name": node.name,
            "parentId": parent,
            "properties": properties,
            "attributes": attributes,
            "tags": node.tags
        });
        if let Some(source) = &node.source {
            data["source"] = serde_json::json!(source);
        }
        if !send_command(port, "CREATE_INSTANCE", data) {
            // Its children have nowhere to go, so they are counted with it.
            *failed += 1 + node.children.iter().map(count_of).sum::<usize>();
            return;
        }
        *created += 1;
        let id = node.id.to_string();
        for child in &node.children {
            create(port, child, &id, created, failed);
        }
    }
    fn count_of(node: &crate::tree_import::TreeNode) -> usize {
        1 + node.children.iter().map(count_of).sum::<usize>()
    }

    let (mut created, mut failed) = (0, 0);
    for root in &tree.roots {
        create(port, root, &parent_id, &mut created, &mut failed);
    }

    if tree.outside_refs > 0 {
        print_dim(&format!(
            "  {} reference(s) pointed outside the folder and were left empty.",
            tree.outside_refs
        ));
    }
    for line in tree.unreadable.iter().take(10) {
        print_dim(&format!("  {}", line));
    }
    if !tree.unions.is_empty() {
        print_info(&format!(
            "{} union(s) were not created: {}",
            tree.unions.len(),
            tree.unions.join(", ")
        ));
        print_dim("  Studio does not let a plugin rebuild a union's shape. Drag the model's");
        print_dim("  .rbxm into Studio for those, or rebuild them there by hand.");
    }
    if failed > 0 {
        report_error(&format!("{} instance(s) could not be created.", failed));
        print_dim("  Check the result with syncix tree.");
        return 1;
    }

    print_ok(&format!("Imported {} instance(s).", created));
    print_dim("  Run syncix pull to confirm the result from Studio.");
    0
}

pub(crate) fn import_rbxmx(cli_args: &[String]) -> i32 {
    let Some(file_path) = cli_args.get(1) else {
        report_error("Usage: syncix import <file.rbxmx> [parent]");
        return 1;
    };
    let Some(port) = require_core() else { return 1 };
    let parent_ref = cli_args.get(2).cloned().unwrap_or_else(|| "Workspace".to_string());

    let xml = match std::fs::read_to_string(file_path) {
        Ok(x) => x,
        Err(e) => {
            report_error(&format!("Could not read {}: {}", file_path, e));
            if e.kind() == std::io::ErrorKind::NotFound {
                if let Some(hint) = file_hint(file_path) {
                    print_hint(&hint);
                }
            }
            return 1;
        }
    };

    let (root_list, skipped) = match crate::rbxmx_import::parse_text(&xml) {
        Ok(v) => v,
        Err(e) => {
            report_error(&format!("Could not parse {}: {}", file_path, e));
            return 1;
        }
    };

    let total_count = crate::rbxmx_import::tally(&root_list);
    if total_count == 0 {
        report_error("The file contains no instances.");
        return 1;
    }

    // The parent is resolved to an identity ONCE, before anything is created. Looked up by
    // name for every root, it became ambiguous as soon as the import itself created a
    // second instance with that name (a place file carrying its own ReplicatedStorage).
    let parent_id = match fetch_json(port, &format!("/model/object?target={}", url_encode(&parent_ref))) {
        Some(o) => match o.get("syncix_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                let reason = o.get("error").and_then(|e| e.as_str()).unwrap_or("not found");
                report_error(&format!("Parent {}: {}", parent_ref, reason));
                return 1;
            }
        },
        None => return 1,
    };
    // Snapshot of the tree, used to find the existing services a place file merges into.
    let tree = fetch_tree(port).unwrap_or_default();

    print_info(&format!("Importing {} instance(s) into {}", total_count, parent_ref));
    if root_list.iter().any(|n| crate::rbxmx_import::is_service(&n.class_name)) {
        print_dim("  The file holds services (a whole place): their contents merge into the");
        print_dim("  matching services of this place; the parent applies only to the rest.");
    }
    if skipped > 0 {
        print_dim(&format!(
            "  {} property value(s) use types Syncix does not model and were skipped.",
            skipped
        ));
    }

    #[derive(Default)]
    struct Outcome {
        created: usize,
        failed: usize,
        merged: usize,
        settings_kept: usize,
        out_of_scope: usize,
    }

    /// The existing instance a singleton maps onto. A service is looked up among the
    /// top-level services, any other singleton among the children of `parent`.
    fn existing_singleton<'a>(tree: &'a [TreeRow], class_name: &str, parent: Option<&str>) -> Option<&'a str> {
        let top_level = |r: &TreeRow| match r.parent_ref.as_deref() {
            None => true,
            Some(p) => tree.iter().any(|q| q.id == p && q.class_str == "DataModel"),
        };
        tree.iter()
            .find(|r| {
                r.class_str == class_name
                    && match parent {
                        None => top_level(r),
                        Some(p) => r.parent_ref.as_deref() == Some(p),
                    }
            })
            .map(|r| r.id.as_str())
    }

    // Recursive creation. Each node is created first, then its properties are written.
    fn generate(
        port: u16,
        tree: &[TreeRow],
        node_entry: &crate::rbxmx_import::ImportedNode,
        parent_ref: &str,
        outcome: &mut Outcome,
    ) {
        // Services and singleton containers are never created: Studio refuses to, and
        // the core used to keep a phantom copy that made names ambiguous. Their
        // contents go into the instance that already exists; their own settings are
        // left as they are, an import must not retune the place's Lighting.
        if crate::rbxmx_import::is_singleton(&node_entry.class_name) {
            let lookup_parent = if crate::rbxmx_import::is_service(&node_entry.class_name) {
                None
            } else {
                Some(parent_ref)
            };
            match existing_singleton(tree, &node_entry.class_name, lookup_parent) {
                Some(existing) => {
                    outcome.merged += 1;
                    outcome.settings_kept += node_entry.properties.len();
                    for child_entry in &node_entry.children {
                        generate(port, tree, child_entry, existing, outcome);
                    }
                }
                None => outcome.out_of_scope += crate::rbxmx_import::tally(std::slice::from_ref(node_entry)),
            }
            return;
        }

        // The identity is generated IN ADVANCE, so the created object can be targeted by
        // identity rather than by name. Targeting by name failed with an ambiguity error
        // when the imported tree repeated an existing name.
        let identity = uuid::Uuid::new_v4().to_string();
        // The properties travel with the create: one command per instance. (One command
        // per property, each followed by a pause, took ~6.5 minutes for 1076 instances.)
        // Values are not turned into text: text loses the type, and types like CFrame,
        // UDim and NumberRange were dropped entirely on import. The wire format carries it.
        let properties: serde_json::Map<String, serde_json::Value> = node_entry
            .properties
            .iter()
            .map(|(name, value)| (name.clone(), crate::values::pv_to_wire(value)))
            .collect();
        let mut data = serde_json::json!({
            "id": identity,
            "className": node_entry.class_name,
            "name": node_entry.name,
            "parentId": parent_ref,
            "properties": properties
        });
        if let Some(source) = &node_entry.source {
            data["source"] = serde_json::json!(source);
        }
        if !send_command(port, "CREATE_INSTANCE", data) {
            outcome.failed += 1;
            return;
        }
        outcome.created += 1;

        for child_entry in &node_entry.children {
            generate(port, tree, child_entry, &identity, outcome);
        }
    }

    let mut outcome = Outcome::default();
    for k in &root_list {
        generate(port, &tree, k, &parent_id, &mut outcome);
    }

    if outcome.merged > 0 {
        print_dim(&format!(
            "  {} service/container item(s) merged into the existing ones; their own settings ({} value(s)) were left unchanged.",
            outcome.merged, outcome.settings_kept
        ));
    }
    if outcome.out_of_scope > 0 {
        print_info(&format!(
            "{} instance(s) skipped: their service is not part of this sync (see scope in syncix.toml).",
            outcome.out_of_scope
        ));
    }
    if outcome.failed > 0 {
        report_error(&format!("{} instance(s) could not be created.", outcome.failed));
        print_dim("  Check the result with syncix tree.");
        return 1;
    }

    print_ok(&format!("Imported {} instance(s).", outcome.created));
    print_dim("  Run syncix pull to confirm the result from Studio.");
    0
}
