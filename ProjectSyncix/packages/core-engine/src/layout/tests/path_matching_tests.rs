//! path matching tests.

use crate::layout::*;
use crate::model::{DataModel, InstanceNode};

/// The watcher reports absolute paths, data_file produces relative paths.
/// With plain equality a disk deletion never matched.
#[test]
fn absolute_path_matches_relative_expectation() {
let mut dm = DataModel::new();
let mut service_name = InstanceNode::new("ServerScriptService", "ServerScriptService");
let service_id = service_name.syncix_id;
service_name.parent = None;

let mut script_node = InstanceNode::new("Script", "DiskDeleteTest");
let script_id = script_node.syncix_id;
script_node.parent = Some(service_id);
service_name.children.push(script_id);

dm.upsert_instance(service_name).unwrap();
dm.upsert_instance(script_node).unwrap();

let absolute = Path::new(r"C:\project\src_workspace\ServerScriptService\DiskDeleteTest.server.lua");
assert_eq!(
    uuid_for_path(&dm, "src_workspace", absolute),
    Some(script_id)
);
}

/// Deleting the meta file is not deleting the instance.
#[test]
fn deleting_meta_file_is_not_a_delete() {
let mut dm = DataModel::new();
let mut script_node = InstanceNode::new("Script", "Code");
script_node.parent = None;
dm.upsert_instance(script_node).unwrap();

let meta = Path::new(r"C:\project\src_workspace\Code.meta.json");
assert_eq!(uuid_for_path(&dm, "src_workspace", meta), None);
}
