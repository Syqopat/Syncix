//! repair write tests.

use crate::layout::*;

/// A repair the core makes itself (an identity put back, a file name corrected) must
/// not come back through the watcher as a user's change or a new object.
#[test]
fn a_repair_is_recognised_as_our_own_work() {
    let dir = std::env::temp_dir().join(format!("syncix_repair_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Box.part.json");
    let content = "{ \"name\": \"Box\" }\n";

    write_own(&file, content).unwrap();
    assert!(is_own_write(&file, content), "the write must be recognised as ours");

    let renamed = dir.join("Box_1a2b3c4d.part.json");
    rename_own(&file, &renamed).unwrap();
    assert!(is_own_write(&renamed, content), "the file that appeared is ours");
    assert!(is_own_delete(&file), "the file that went is ours, not a user's deletion");

    // Someone else's content at the same path is NOT ours.
    assert!(!is_own_write(&renamed, "{ \"name\": \"Crate\" }\n"));
    fs::remove_dir_all(&dir).ok();
}
