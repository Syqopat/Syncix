//! config tests.

use crate::project::*;

fn resolve_arg(toml_text: &str) -> ProjectConfig {
    ProjectConfig::resolve_arg(&toml_text.parse::<toml::Value>().unwrap(), ".")
}

#[test]
fn empty_file_gives_defaults() {
    let c = resolve_arg("");
    assert_eq!(c.mode_value, SyncMode::TwoWay);
    assert_eq!(c.wanted_port, DEFAULT_PORT);
    assert!(c.safety_settings.trash_enabled);
    assert!(c.safety_settings.confirm_delete);
    assert!(c.restore_cmd);
}

/// The setting that was actually asked for: working one-way, like Rojo.
#[test]
fn one_way_mode() {
    let c = resolve_arg("[sync]\nmode = \"disk_to_studio\"\n");
    assert_eq!(c.mode_value, SyncMode::DiskToStudio);
    assert!(c.mode_value.accepts_from_disk(), "disk -> Studio must be on");
    assert!(!c.mode_value.accepts_from_studio(), "Studio -> disk must be off");

    let t = resolve_arg("[sync]\nmode = \"studio_to_disk\"\n");
    assert!(t.mode_value.accepts_from_studio());
    assert!(!t.mode_value.accepts_from_disk());
}

/// Aliases such as "rojo" and "push" must give the same mode: users should be able
/// to type whichever word they remember.
#[test]
fn mode_aliases() {
    assert_eq!(SyncMode::resolve_arg("rojo"), Some(SyncMode::DiskToStudio));
    assert_eq!(SyncMode::resolve_arg("push"), Some(SyncMode::DiskToStudio));
    assert_eq!(SyncMode::resolve_arg("PULL"), Some(SyncMode::StudioToDisk));
    assert_eq!(SyncMode::resolve_arg("Two-Way"), Some(SyncMode::TwoWay));
    assert_eq!(SyncMode::resolve_arg("off"), Some(SyncMode::Manual));
}

/// In manual mode no direction may run automatically.
#[test]
fn manual_mode_disables_both_directions() {
    let c = resolve_arg("[sync]\nmode = \"manual\"\n");
    assert!(!c.mode_value.accepts_from_studio());
    assert!(!c.mode_value.accepts_from_disk());
}

#[test]
fn config_typos_are_reported_with_the_closest_spelling() {
    let value: toml::Value = "[sync]\nmoed = \"manual\"\nmode = \"to_way\"\n[scoep]\nx = 1\n[scope]\nservices = [\"ReplicatedStorge\"]\n"
        .parse()
        .unwrap();
    let warnings = config_warnings(&value);
    let has = |a: &str, b: &str| warnings.iter().any(|w| w.contains(a) && w.contains(b));
    assert!(has("moed", "Did you mean mode?"), "{:?}", warnings);
    assert!(has("to_way", "Did you mean two_way?"), "{:?}", warnings);
    assert!(has("[scoep]", "Did you mean scope?"), "{:?}", warnings);
    assert!(has("ReplicatedStorge", "Did you mean ReplicatedStorage?"), "{:?}", warnings);

    let clean: toml::Value = "[sync]\nmode = \"two_way\"\n[scope]\nservices = [\"Workspace\"]\n"
        .parse()
        .unwrap();
    assert!(config_warnings(&clean).is_empty(), "{:?}", config_warnings(&clean));

    // A key that was understood once is named as retired, not guessed at as a typo.
    let retired: toml::Value = "[sync]\nplay_mode = \"queue\"\n".parse().unwrap();
    let said = config_warnings(&retired);
    assert!(
        said.iter().any(|w| w.contains("play_mode") && w.contains("no longer used")),
        "{:?}",
        said
    );
}

/// A typo must not break sync; it falls back to the default and warns.
#[test]
fn unknown_mode_falls_back_to_default() {
    let c = resolve_arg("[sync]\nmode = \"disk-to-studioo\"\n");
    assert_eq!(c.mode_value, SyncMode::TwoWay);
}

#[test]
fn safety_and_scope_are_read() {
    let c = resolve_arg(
        "[safety]\ntrash = false\ntrash_keep = 3\ndelete_grace_ms = 1500\nconfirm_delete = false\n\
         \n[scope]\nservices = [\"Workspace\", \"Lighting\"]\nignore_classes = [\"Camera\"]\n\
         ignore_properties = [\"Transparency\"]\n",
    );
    assert!(!c.safety_settings.trash_enabled);
    assert_eq!(c.safety_settings.trash_keep_runs, 3);
    assert_eq!(c.safety_settings.delete_grace_ms, 1500);
    assert!(!c.safety_settings.confirm_delete);
    assert_eq!(c.scope_settings.service_list, vec!["Workspace", "Lighting"]);
    assert!(!c.class_allowed("Camera"));
    assert!(c.class_allowed("Part"));
    assert!(!c.property_allowed("Transparency"));
    assert!(c.property_allowed("Anchored"));
}

/// Old syncix.toml files were flat (no sections). An upgrade must not break
/// anyone's file.
#[test]
fn flat_legacy_format_still_parses() {
    let c = resolve_arg("sync_dir = \"source\"\nport = 25565\n");
    assert_eq!(c.wanted_port, 25565);
    assert!(c.sync_dir.ends_with("source"), "sync_dir: {}", c.sync_dir);
}

/// Nonsense values must be rejected: a 0 ms debounce means writing forever.
#[test]
fn out_of_range_values_are_clamped() {
    let c = resolve_arg("[sync]\ndebounce_ms = 0\n\n[safety]\ntrash_keep = 0\n");
    assert!(c.debounce_ms >= 10);
    assert!(c.safety_settings.trash_keep_runs >= 1);
}
