//! tests.

use crate::project::*;

#[test]
fn patch_diff_compatible_minor_diff_not() {
    assert!(versions_compatible("0.3.1", "0.3.9"));
    assert!(versions_compatible("1.0.0", "1.0.0"));
    assert!(!versions_compatible("0.3.0", "0.4.0"));
    assert!(!versions_compatible("0.3.0", "1.3.0"));
}

#[test]
fn bad_version_text_does_not_crash() {
    assert!(versions_compatible("abc", "abc"));
    assert!(!versions_compatible("0.3.0", "abc"));
}
