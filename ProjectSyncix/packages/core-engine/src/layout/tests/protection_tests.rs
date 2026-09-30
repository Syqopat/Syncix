//! protection tests.

use crate::layout::*;

/// Files we do not recognise are NEVER touched.
/// Regression test for README/note files placed in the sync folder being deleted.
#[test]
fn foreign_files_are_not_managed() {
    assert!(!is_managed_file(Path::new("src/NOTLAR.md")));
    assert!(!is_managed_file(Path::new("src/.gitkeep")));
    assert!(!is_managed_file(Path::new("src/resim.png")));
    assert!(!is_managed_file(Path::new("src/rapor.pdf")));
}

/// Formats we could produce ourselves are managed.
#[test]
fn our_formats_are_managed() {
    assert!(is_managed_file(Path::new("src/Box.json")));
    assert!(is_managed_file(Path::new("src/Main.server.lua")));
    assert!(is_managed_file(Path::new("src/Hud.client.lua")));
    assert!(is_managed_file(Path::new("src/Modul.lua")));
    assert!(is_managed_file(Path::new("src/Modul.luau")));
    assert!(is_managed_file(Path::new("src/Message.txt")));
    assert!(is_managed_file(Path::new("src/Main.meta.json")));
}

#[test]
fn ignore_patterns_match() {
    let patterns = vec!["*.md".to_string(), "notlar/**".to_string()];
    assert!(is_ignored(Path::new("src/OKU.md"), "src", &patterns));
    assert!(is_ignored(Path::new("src/notlar/a/b.lua"), "src", &patterns));
    assert!(!is_ignored(Path::new("src/Box.json"), "src", &patterns));
}

#[test]
fn empty_pattern_list_ignores_nothing() {
    assert!(!is_ignored(Path::new("src/OKU.md"), "src", &[]));
}

/// Windows backslashes must match too.
#[test]
fn backslashes_are_normalised() {
    let patterns = vec!["notlar/**".to_string()];
    let p = PathBuf::from("src").join("notlar").join("gizli.lua");
    assert!(is_ignored(&p, "src", &patterns));
}

/// A broken pattern must not cause a crash.
#[test]
fn broken_pattern_does_not_crash() {
    let patterns = vec!["[".to_string()];
    assert!(!is_ignored(Path::new("src/Box.json"), "src", &patterns));
}
