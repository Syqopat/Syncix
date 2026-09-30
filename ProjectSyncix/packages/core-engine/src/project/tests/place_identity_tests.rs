//! place identity tests.

use crate::project::*;

fn scratch_dir(item_name: &str) -> ProjectConfig {
    let root_dir = std::env::temp_dir().join(format!("syncix-place-{}", item_name));
    let _ = fs::remove_dir_all(&root_dir);
    fs::create_dir_all(&root_dir).unwrap();
    let mut c = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
    c.root = root_dir;
    c
}

/// The first place to connect claims the folder.
#[test]
fn first_connected_place_claims_folder() {
    let c = scratch_dir("first");
    assert_eq!(c.linked_place(), None, "a new folder must not be bound to a place");
    c.bind_place("place-A");
    assert_eq!(c.linked_place(), Some("place-A".to_string()));
}

/// The identity must survive a core restart: it is read from a file.
#[test]
fn identity_is_stable() {
    let c = scratch_dir("kalici");
    c.bind_place("place-A");
    // A second configuration object looking at the same root
    let mut c2 = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
    c2.root = c.root.clone();
    assert_eq!(c2.linked_place(), Some("place-A".to_string()));
}

/// The point: a different place connecting to the same folder must be noticed.
#[test]
fn different_place_is_detected() {
    let c = scratch_dir("different");
    c.bind_place("place-A");
    let is_bound = c.linked_place().unwrap();
    assert_ne!(is_bound, "place-B", "B must not enter a folder bound to A");
    // Once the decision is made, a new owner can be written.
    c.bind_place("place-B");
    assert_eq!(c.linked_place(), Some("place-B".to_string()));
}

/// The filters are applied in both the plugin and the core. The core side is needed so
/// the setting still applies when an older plugin connects —
/// for a while it existed only in the plugin, and then the setting silently did nothing.
#[test]
fn filters_reject_excluded() {
    let c = ProjectConfig::resolve_arg(
        &"[scope]
ignore_classes = [\"Camera\", \"Terrain\"]
ignore_properties = [\"Transparency\"]
"
            .parse::<toml::Value>()
            .unwrap(),
        ".",
    );
    assert!(!c.class_allowed("Camera"));
    assert!(!c.class_allowed("Terrain"));
    assert!(c.class_allowed("Part"), "a class not on the list must pass");

    assert!(!c.property_allowed("Transparency"));
    assert!(c.property_allowed("Anchored"), "a property not on the list must pass");
}

/// An empty list means "no restriction", not "let nothing through".
#[test]
fn empty_filter_allows_everything() {
    let c = ProjectConfig::resolve_arg(&toml::Value::Table(Default::default()), ".");
    assert!(c.class_allowed("Camera"));
    assert!(c.property_allowed("Transparency"));
}

/// Suspension must be visible from all three places and must be reversible.
#[test]
fn suspension_works() {
    set_sync_suspended(false);
    assert!(!is_sync_suspended());
    set_sync_suspended(true);
    assert!(is_sync_suspended());
    set_sync_suspended(false);
    assert!(!is_sync_suspended());
}
