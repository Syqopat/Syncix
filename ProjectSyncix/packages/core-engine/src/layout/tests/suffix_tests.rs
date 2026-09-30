//! suffix tests.

use crate::layout::*;

/// The sync folder often arrives as "./src_workspace". The leading "."
/// counts as a separate component, so a plain ends_with does not match and disk
/// deletions were silently dropped.
#[test]
fn dot_slash_prefix_does_not_break_matching() {
assert!(suffix_matches(
    Path::new(r"C:\project\src_workspace\SSS\Code.server.lua"),
    Path::new("./src_workspace/SSS/Code.server.lua"),
));
}

#[test]
fn different_file_does_not_match() {
assert!(!suffix_matches(
    Path::new(r"C:\project\src_workspace\SSS\Other.server.lua"),
    Path::new("./src_workspace/SSS/Code.server.lua"),
));
}

/// Matching only the file name is not enough; the folder path must match too.
#[test]
fn same_name_other_folder_does_not_match() {
assert!(!suffix_matches(
    Path::new(r"C:\project\src_workspace\Other\Code.server.lua"),
    Path::new("./src_workspace/SSS/Code.server.lua"),
));
}
