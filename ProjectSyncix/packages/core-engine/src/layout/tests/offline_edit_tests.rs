//! What happens to an edit made on disk while the core was not running.

use crate::layout::*;
use crate::model::{DataModel, InstanceNode};
use std::fs;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("syncix-offline-{}", name));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).unwrap();
    root
}

/// One script, parented to nothing, so its file lands straight in the sync folder.
fn model_with_script(source: &str) -> DataModel {
    let mut dm = DataModel::new();
    let mut node = InstanceNode::new("Script", "Main");
    node.parent = None;
    node.source = Some(source.to_string());
    dm.upsert_instance(node).unwrap();
    dm
}

/// The incident this prevents: the core was down, the file was edited in the editor,
/// Studio then connected and its own (older) version of the script was written over the
/// file. The edit was gone, with nothing in the log and nothing in the trash.
#[test]
fn an_edit_made_while_the_core_was_down_is_kept_in_the_trash() {
    let root = scratch("overwrite");
    let sync = root.join("src");
    let s = sync.to_str().unwrap();
    let file = sync.join("Main.server.lua");
    fs::write(&file, "print('edited while the core was down')").unwrap();

    write_full_tree(&model_with_script("print('from studio')"), s, &[], true);

    let written = fs::read_to_string(&file).unwrap();
    assert!(
        written.contains("from studio"),
        "Studio stays the authority for the session: {}",
        written
    );
    let rescued = trash_entries(s)
        .iter()
        .any(|entry| entry.rel.contains("Main"));
    assert!(rescued, "the version that was on disk has to be in the trash");
}

/// The counterpart: a file the core itself wrote must not be copied to the trash on every
/// pass. Otherwise one ordinary edit in Studio would fill the trash with copies.
#[test]
fn our_own_write_is_not_backed_up_again() {
    let root = scratch("own-write");
    let sync = root.join("src");
    let s = sync.to_str().unwrap();

    // First pass: nothing on disk yet, so there is nothing to rescue.
    write_full_tree(&model_with_script("print('one')"), s, &[], true);
    assert!(trash_entries(s).is_empty(), "a fresh write is not a rescue");

    // Second pass with new content: the file on disk is ours, so it is simply replaced.
    write_full_tree(&model_with_script("print('two')"), s, &[], true);
    assert!(
        trash_entries(s).is_empty(),
        "replacing our own write must not go through the trash"
    );
    let written = fs::read_to_string(sync.join("Main.server.lua")).unwrap();
    assert!(written.contains("two"), "{}", written);
}
