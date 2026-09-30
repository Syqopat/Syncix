//! trash tests.

use crate::layout::*;
use std::fs;

/// Each test works in its own folder: the trash uses one run name per process,
/// so tests sharing a directory would break each other.
fn scratch_root(item_name: &str) -> PathBuf {
let root_dir = std::env::temp_dir().join(format!("syncix-trash-{}", item_name));
let _ = fs::remove_dir_all(&root_dir);
fs::create_dir_all(root_dir.join("src")).unwrap();
root_dir
}

/// From a real incident: the editor opened before Studio connected, the model was empty and
/// the reconciler moved all 48 files in the sync folder to the trash. While the model
/// is not authoritative, nothing may be removed from disk.
#[test]
fn empty_model_deletes_nothing_before_studio_syncs() {
let root_dir = scratch_root("no-authority");
let sync = root_dir.join("src");
let s = sync.to_str().unwrap();
let game_file = sync.join("Workspace.json");
let script_node = sync.join("ServerScriptService").join("Main.server.lua");
fs::create_dir_all(script_node.parent().unwrap()).unwrap();
fs::write(&game_file, "{}").unwrap();
fs::write(&script_node, "print('game code')").unwrap();

write_full_tree(&DataModel::new(), s, &[], false);

assert!(game_file.exists(), "no file may be deleted while the model is not authoritative");
assert!(script_node.exists(), "the script in the subfolder must stay too");
assert!(trash_runs(s).is_empty(), "nothing may go to the trash");
}

/// With authority the old behaviour is unchanged: a file not in the model goes to the trash.
#[test]
fn authoritative_model_trashes_extras() {
let root_dir = scratch_root("authority");
let sync = root_dir.join("src");
let s = sync.to_str().unwrap();
let extra = sync.join("Deleted.server.lua");
fs::write(&extra, "-- no longer in Studio").unwrap();

write_full_tree(&DataModel::new(), s, &[], true);

assert!(!extra.exists(), "a file missing from the authoritative model must be removed");
assert_eq!(trash_runs(s).len(), 1, "the removed file must be in the trash");
}

/// The point: when the reconciler deletes a file, its content must not be lost.
#[test]
fn deleted_file_stays_in_trash() {
let root_dir = scratch_root("basic");
let sync = root_dir.join("src");
let file_path = sync.join("Important.server.lua");
fs::write(&file_path, "print('must_not_be_lost')").unwrap();

move_to_trash(&file_path, sync.to_str().unwrap()).unwrap();

assert!(!file_path.exists(), "the file must be removed from the sync folder");
let runs = trash_runs(sync.to_str().unwrap());
assert_eq!(runs.len(), 1);
assert_eq!(runs[0].1, 1, "exactly one file must be in the trash");
}

#[test]
fn restore_returns_content_unchanged() {
let root_dir = scratch_root("restore");
let sync = root_dir.join("src");
let s = sync.to_str().unwrap();
let file_path = sync.join("Sub").join("Code.server.lua");
fs::create_dir_all(file_path.parent().unwrap()).unwrap();
fs::write(&file_path, "-- original content").unwrap();

move_to_trash(&file_path, s).unwrap();
let run_name = trash_runs(s)[0].0.clone();
let (restored_count, skipped) = restore_from_trash(s, &run_name);

assert_eq!((restored_count, skipped), (1, 0));
assert!(file_path.exists(), "the file must return to its old place, subfolder included");
assert_eq!(fs::read_to_string(&file_path).unwrap(), "-- original content");
}

/// Restoring must not cause data loss of its own: if a file exists at the same path
/// it is not overwritten but skipped.
fn trash_sample() -> Vec<TrashEntry> {
let e = |run: &str, rel: &str| TrashEntry { run: run.into(), rel: rel.into() };
// Newest run first, as trash_entries returns them.
vec![
    e("20260911-120000", "Workspace/Ramp.part.json"),
    e("20260911-120000", "Workspace/Zones/init.folder.json"),
    e("20260911-120000", "Workspace/Zones/Floor.part.json"),
    e("20260911-120000", "ServerScriptService/Main.server.lua"),
    e("20260911-120000", "ServerScriptService/Main.meta.json"),
    e("20260910-080000", "Workspace/Ramp.part.json"),
    e("20260910-080000", "Lighting/Sky.sky.json"),
]
}

#[test]
fn restore_by_name_takes_newest_copy_only() {
let f = TrashFilter { name: Some("ramp".into()), ..Default::default() };
let picked = select_entries(&trash_sample(), &f);
assert_eq!(picked.len(), 1);
assert_eq!(picked[0].run, "20260911-120000");
}

#[test]
fn restore_by_folder_name_brings_its_contents() {
let f = TrashFilter { name: Some("Zones".into()), ..Default::default() };
let rels: Vec<String> = select_entries(&trash_sample(), &f).into_iter().map(|e| e.rel).collect();
assert_eq!(rels, vec!["Workspace/Zones/init.folder.json", "Workspace/Zones/Floor.part.json"]);
}

#[test]
fn restore_filters_by_scope_class_and_time() {
let by_scope = TrashFilter { scope: Some("Lighting".into()), ..Default::default() };
assert_eq!(select_entries(&trash_sample(), &by_scope).len(), 1);

let by_class = TrashFilter { class_name: Some("part".into()), ..Default::default() };
assert_eq!(select_entries(&trash_sample(), &by_class).len(), 2);

let by_time = TrashFilter { since: Some("20260911-000000".into()), ..Default::default() };
assert!(select_entries(&trash_sample(), &by_time).iter().all(|e| e.run == "20260911-120000"));
}

#[test]
fn same_name_in_two_places_is_ambiguous_until_narrowed() {
let e = |rel: &str| TrashEntry { run: "20260911-120000".into(), rel: rel.into() };
let entries = vec![
    e("Workspace/RampA.part.json"),
    e("Workspace/RampA_149b6fa2.part.json"),
    e("Workspace/Obby/init.folder.json"),
    e("Workspace/Obby/Part1.part.json"),
];
assert_eq!(instances_named(&entries, "RampA"), vec!["Workspace/RampA", "Workspace/RampA_149b6fa2"]);
// A folder with its contents is one instance.
assert_eq!(instances_named(&entries, "Obby"), vec!["Workspace/Obby"]);

// The path instances_named lists picks exactly that one.
let narrowed = TrashFilter {
    name: Some("RampA".into()),
    scope: Some("Workspace/RampA".into()),
    ..Default::default()
};
let picked = select_entries(&entries, &narrowed);
assert_eq!(picked.len(), 1);
assert_eq!(instances_named(&picked, "RampA"), vec!["Workspace/RampA"]);
}

#[test]
fn restore_brings_meta_with_its_script() {
let f = TrashFilter { class_name: Some("script".into()), ..Default::default() };
let rels: Vec<String> = select_entries(&trash_sample(), &f).into_iter().map(|e| e.rel).collect();
assert_eq!(rels, vec!["ServerScriptService/Main.server.lua", "ServerScriptService/Main.meta.json"]);
}

#[test]
fn restore_does_not_overwrite_existing() {
let root_dir = scratch_root("overwrite");
let sync = root_dir.join("src");
let s = sync.to_str().unwrap();
let file_path = sync.join("Code.server.lua");

fs::write(&file_path, "old").unwrap();
move_to_trash(&file_path, s).unwrap();
fs::write(&file_path, "new and valuable").unwrap();

let run_name = trash_runs(s)[0].0.clone();
let (restored_count, skipped) = restore_from_trash(s, &run_name);

assert_eq!((restored_count, skipped), (0, 1));
assert_eq!(fs::read_to_string(&file_path).unwrap(), "new and valuable");
}

/// The trash must be OUTSIDE the sync folder; inside, the watcher would take it for
/// new content and the reconciler would delete it again on the next pass.
#[test]
fn trash_is_outside_sync_folder() {
let root_dir = scratch_root("location");
let sync = root_dir.join("src");
let s = sync.to_str().unwrap();
let file_path = sync.join("Code.server.lua");
fs::write(&file_path, "x").unwrap();
move_to_trash(&file_path, s).unwrap();

let mut remaining_items = Vec::new();
collect_files(&sync, &mut remaining_items);
assert!(remaining_items.is_empty(), "no leftovers may remain in the sync folder");
assert!(root_dir.join(".syncix").join("trash").exists());
}
