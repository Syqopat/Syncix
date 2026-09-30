//! file name tests.

use crate::layout::*;

/// What the editor sees in a file name is what the watcher must read out of it: the
/// object's name, the identity duplicates carry, and the class.
#[test]
fn a_data_file_name_gives_name_identity_and_class() {
    assert_eq!(instance_name_in("Box.part.json"), "Box");
    assert_eq!(instance_name_in("Box_1a2b3c4d.part.json"), "Box");
    assert_eq!(instance_name_in("Ore Shop_1a2b3c4d.model.json"), "Ore Shop");
    assert_eq!(short_id_in("Box.part.json"), None);
    assert_eq!(short_id_in("Box_1a2b3c4d.part.json"), Some("1a2b3c4d".to_string()));
    // A name that merely ends in _something is not an identity.
    assert_eq!(short_id_in("Box_left.part.json"), None);
    assert_eq!(instance_name_in("Box_left.part.json"), "Box_left");
    assert_eq!(class_in_file_name("Box.part.json"), Some("Part"));
    assert_eq!(class_in_file_name("Rock_1a2b3c4d.meshpart.json"), Some("MeshPart"));
    assert_eq!(class_in_file_name("Box.meta.json"), None);
}
