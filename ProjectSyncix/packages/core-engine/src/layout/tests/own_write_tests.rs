//! own write tests.

use crate::layout::*;

#[test]
fn a_write_is_recognised_however_the_watcher_spells_its_path() {
    let relative = Path::new("./src_own_write_test/Workspace/Box.part.json");
    record_write(relative, "{}");
    let absolute = std::env::current_dir()
        .unwrap()
        .join(".")
        .join("src_own_write_test")
        .join("Workspace")
        .join("Box.part.json");
    assert!(is_own_write(&absolute, "{}"));
    assert!(!is_own_write(&absolute, "{\"changed\":1}"));
    assert!(is_own_write(Path::new("src_own_write_test/Workspace/../Workspace/Box.part.json"), "{}"));

    record_delete(relative);
    assert!(is_own_delete(&absolute));
    assert!(!is_own_delete(&absolute), "the record is single-use");
}
