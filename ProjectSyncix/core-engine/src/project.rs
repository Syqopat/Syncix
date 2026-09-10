//! Project configuration and identity.
//!
//! Two things here matter for a release:
//!  1. The port is no longer fixed. It is read from syncix.toml; if taken the next one is
//!     tried and the CHOSEN port is written to disk. The editor and the CLI read that file instead of guessing.
//!  2. The core introduces itself (project name, root directory, version). The Studio plugin
//!     needs this to show the user which project it connected to.

use std::fs;
use std::path::{Path, PathBuf};

/// Sync is suspended when the place identity does not match.
///
/// Why it is global: the three places that must read the flag run independently
/// (the file watcher in its own thread, the disk writer in its own task,
/// the command loop in the main task). Instead of wiring a channel to each,
/// a single atomic flag guarantees that all three go quiet at the same moment.
static SYNC_SUSPENDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_sync_suspended(suspended: bool) {
    SYNC_SUSPENDED.store(suspended, std::sync::atomic::Ordering::SeqCst);
}

/// Is sync suspended? While suspended NO direction runs: nothing is read from disk,
/// nothing is written to disk, no command goes to Studio. The point is to leave
/// both sides as they are until a decision is made.
pub fn is_sync_suspended() -> bool {
    SYNC_SUSPENDED.load(std::sync::atomic::Ordering::SeqCst)
}

/// The core's own version (from Cargo.toml; single source).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Wire protocol version. It is INCREASED when the message format between the Studio
/// plugin and the core becomes incompatible. It is independent of the version number:
/// a patch such as 0.3.1 -> 0.3.2 does not break the protocol, and this number stays the same.
pub const PROTOCOL_VERSION: u32 = 1;

/// Ports are searched in this range. The Studio plugin scans the same range.
pub const PORT_SCAN_SPAN: u16 = 10;

pub const DEFAULT_PORT: u16 = 8080;

/// Which directions sync is active in.
///
/// Rojo works one-way: the file system is the single source of truth and Studio only
/// receives. Syncix is two-way by default, but not everyone wants that —
/// someone on a team may want Studio read-only, while a designer may not want
/// the disk overwritten. So the direction is a setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncMode {
    /// Both directions enabled. The default.
    TwoWay,
    /// Studio -> disk. Changes made in Studio are written to disk; changes on disk
    /// do NOT go to Studio. For people who design scenes in Studio and want the
    /// result in version control.
    StudioToDisk,
    /// Disk -> Studio. Rojo-style workflow: the file system is the source of truth.
    DiskToStudio,
    /// No direction is automatic; only explicitly given commands are carried out
    /// (syncix pull, syncix set, ...). For working under supervision on a risky scene.
    Manual,
}

impl SyncMode {
    fn resolve_arg(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "two_way" | "twoway" | "both" => Some(Self::TwoWay),
            "studio_to_disk" | "studio" | "pull" => Some(Self::StudioToDisk),
            "disk_to_studio" | "disk" | "push" | "rojo" => Some(Self::DiskToStudio),
            "manual" | "off" | "none" => Some(Self::Manual),
            _ => None,
        }
    }

    pub fn name_of(&self) -> &'static str {
        match self {
            Self::TwoWay => "two_way",
            Self::StudioToDisk => "studio_to_disk",
            Self::DiskToStudio => "disk_to_studio",
            Self::Manual => "manual",
        }
    }

    /// Should a change made in Studio be reflected in the model and on disk?
    pub fn accepts_from_studio(&self) -> bool {
        matches!(self, Self::TwoWay | Self::StudioToDisk)
    }

    /// Should a change made on disk be sent to Studio?
    pub fn accepts_from_disk(&self) -> bool {
        matches!(self, Self::TwoWay | Self::DiskToStudio)
    }
}

/// What happens to changes from the editor while the game is running (Play).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayBehavior {
    /// Queued and applied once Play ends. Default.
    Queue,
    /// Dropped. For people who want nothing to happen during Play.
    Ignore,
    /// Applied directly. Studio discards the session when Play ends, so the
    /// change is lost; turn this on only if you mean it.
    Apply,
}

impl PlayBehavior {
    fn resolve_arg(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "queue" => Some(Self::Queue),
            "ignore" | "drop" => Some(Self::Ignore),
            "apply" => Some(Self::Apply),
            _ => None,
        }
    }

    pub fn name_of(&self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Ignore => "ignore",
            Self::Apply => "apply",
        }
    }
}

/// Safety settings regarding deletion.
#[derive(Clone, Debug)]
pub struct SafetySettings {
    /// Whether the reconciler moves the files it deletes to the trash.
    /// When off, files are deleted directly and there is no way back.
    pub trash_enabled: bool,
    /// Number of runs kept in the trash.
    pub trash_keep_runs: usize,
    /// How long to wait before a file deleted from disk counts as a real deletion.
    /// Moves show up in the operating system as delete + create, so
    /// this window is needed. Can be raised on slow disks.
    pub delete_grace_ms: u64,
    /// Whether `syncix rm` asks for confirmation.
    pub confirm_delete: bool,
}

/// Settings determining what is synced.
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ScopeSettings {
    /// Services to observe. If left empty, the plugin's default list applies.
    pub service_list: Vec<String>,
    /// These classes are never synced (for example "Camera", "Terrain").
    pub class_ignore_list: Vec<String>,
    /// These properties are never synced. For filtering out noisy or
    /// machine-specific fields.
    pub property_ignore_list: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    /// Sync folder, relative to the core's working directory (e.g. "../src").
    pub sync_dir: String,
    /// The directory containing syncix.toml, as an absolute path.
    pub root: PathBuf,
    /// Project name shown to the user (the name of the root folder).
    pub name: String,
    /// Port requested in syncix.toml. It may be taken; the port actually bound
    /// is decided by `bind_with_fallback`.
    pub wanted_port: u16,
    /// Whether sourcemap.json is updated on every sync (for luau-lsp).
    pub sourcemap: bool,
    /// Paths excluded from sync (glob). These files are neither read nor deleted.
    pub ignore: Vec<String>,
    /// True when the port was requested explicitly on the command line; then there is no fallback.
    /// Reason: the user types the same port into the Studio plugin; if the core silently
    /// moved to another port the two sides would diverge for no visible reason.
    pub port_fixed: bool,

    /// Sync direction.
    pub mode_value: SyncMode,
    /// Wait time of the disk writer. A small value reflects changes faster but
    /// increases the number of writes.
    pub debounce_ms: u64,
    /// What happens to changes that arrive during Play.
    pub play: PlayBehavior,
    /// Whether Studio asks for permission on the first connection.
    pub prompt_permission: bool,
    /// Whether Syncix's changes go onto Studio's undo stack.
    pub restore_cmd: bool,
    /// Whether a .meta.json is written next to scripts. When off, scripts'
    /// properties and attributes are never written to disk.
    pub meta_files: bool,
    pub safety_settings: SafetySettings,
    pub scope_settings: ScopeSettings,
}

impl Default for SafetySettings {
    fn default() -> Self {
        Self {
            trash_enabled: true,
            trash_keep_runs: 10,
            delete_grace_ms: 800,
            confirm_delete: true,
        }
    }
}


/// Small helpers for reading a section from TOML.
/// Unknown keys are not silently swallowed; the caller prints a warning.
fn string_list(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn read_bool(section: Option<&toml::Value>, key_name: &str, fallback_value: bool) -> bool {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_bool())
        .unwrap_or(fallback_value)
}

fn read_number(section: Option<&toml::Value>, key_name: &str, fallback_value: u64) -> u64 {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_integer())
        .and_then(|x| u64::try_from(x).ok())
        .unwrap_or(fallback_value)
}

impl ProjectConfig {
    /// Looks for syncix.toml upwards from the working directory.
    /// The core can be run from the project root as well as from inside core-engine/.
    pub fn load() -> Self {
        for (config_path, base) in [("../syncix.toml", ".."), ("syncix.toml", ".")] {
            let path = Path::new(config_path);
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            let value = match text.parse::<toml::Value>() {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("Could not read syncix.toml ({}): {}", config_path, e);
                    continue;
                }
            };
            return Self::resolve_arg(&value, base);
        }

        // Without syncix.toml, portable defaults.
        Self::resolve_arg(&toml::Value::Table(Default::default()), "..")
    }

    /// Kept separate so tests can call it too: configuration behaviour must be
    /// verifiable without depending on the file system.
    pub fn resolve_arg(value: &toml::Value, base: &str) -> Self {
        // Keys are accepted both at the top level and inside sections.
        // Reason: old syncix.toml files were flat, and an upgrade must not break anyone's
        // file. If the section exists, it wins.
        let sync = value.get("sync");
        let files = value.get("files");
        let safety = value.get("safety");
        let scope = value.get("scope");
        let server = value.get("server");
        let editor = value.get("editor");

        let al = |section: Option<&toml::Value>, key_name: &str| -> Option<toml::Value> {
            section
                .and_then(|b| b.get(key_name))
                .or_else(|| value.get(key_name))
                .cloned()
        };

        let dir = al(files, "sync_dir")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "src".to_string());

        let port = al(server, "port")
            .and_then(|x| x.as_integer())
            .and_then(|x| u16::try_from(x).ok())
            .unwrap_or(DEFAULT_PORT);

        let mode_value = al(sync, "mode")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .and_then(|s| {
                let m = SyncMode::resolve_arg(&s);
                if m.is_none() {
                    tracing::warn!(
                        "Unknown sync mode '{}'; falling back to two_way.                          Valid values: two_way, studio_to_disk, disk_to_studio, manual.",
                        s
                    );
                }
                m
            })
            .unwrap_or(SyncMode::TwoWay);

        let play = al(sync, "play_mode")
            .and_then(|x| x.as_str().map(|s| s.to_string()))
            .and_then(|s| {
                let p = PlayBehavior::resolve_arg(&s);
                if p.is_none() {
                    tracing::warn!(
                        "Unknown play_mode '{}'; falling back to queue.                          Valid values: queue, ignore, apply.",
                        s
                    );
                }
                p
            })
            .unwrap_or(PlayBehavior::Queue);

        let root = clean_path(fs::canonicalize(base).unwrap_or_else(|_| PathBuf::from(base)));

        Self {
            sync_dir: format!("{}/{}", base, dir),
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Syncix".to_string()),
            root,
            wanted_port: port,
            sourcemap: read_bool(editor, "sourcemap", true)
                && value.get("sourcemap").and_then(|x| x.as_bool()).unwrap_or(true),
            ignore: string_list(al(files, "ignore").as_ref()),
            port_fixed: false,

            mode_value,
            debounce_ms: read_number(sync, "debounce_ms", 120).clamp(10, 10_000),
            play,
            prompt_permission: read_bool(sync, "ask_permission", false),
            restore_cmd: read_bool(sync, "undo", true),
            meta_files: read_bool(files, "meta_files", true),

            safety_settings: SafetySettings {
                trash_enabled: read_bool(safety, "trash", true),
                trash_keep_runs: read_number(safety, "trash_keep", 10).clamp(1, 500) as usize,
                delete_grace_ms: read_number(safety, "delete_grace_ms", 800).clamp(0, 30_000),
                confirm_delete: read_bool(safety, "confirm_delete", true),
            },
            scope_settings: ScopeSettings {
                service_list: string_list(al(scope, "services").as_ref()),
                class_ignore_list: string_list(al(scope, "ignore_classes").as_ref()),
                property_ignore_list: string_list(al(scope, "ignore_properties").as_ref()),
            },
        }
    }

    /// Is this class synced?
    pub fn class_allowed(&self, class_str: &str) -> bool {
        !self.scope_settings.class_ignore_list.iter().any(|d| d == class_str)
    }

    /// Is this property synced?
    pub fn property_allowed(&self, item_name: &str) -> bool {
        !self.scope_settings.property_ignore_list.iter().any(|d| d == item_name)
    }

    /// The identity file of the place this folder is bound to: <project>/.syncix/place
    pub fn place_file(&self) -> PathBuf {
        self.runtime_dir().join("place")
    }

    /// Which place was bound to this folder before? None if never bound.
    pub fn linked_place(&self) -> Option<String> {
        let file_content = fs::read_to_string(self.place_file()).ok()?;
        let k = file_content.trim().to_string();
        (!k.is_empty()).then_some(k)
    }

    /// Binds the folder to a place.
    pub fn bind_place(&self, identity: &str) {
        let dir = self.runtime_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Could not create the .syncix folder: {}", e);
            return;
        }
        if let Err(e) = fs::write(self.place_file(), identity) {
            tracing::warn!("Could not write the place identity: {}", e);
        }
    }

    fn runtime_dir(&self) -> PathBuf {
        self.root.join(".syncix")
    }

    pub fn port_file(&self) -> PathBuf {
        self.runtime_dir().join("port")
    }

    /// The sourcemap file luau-lsp reads, at the project root.
    pub fn sourcemap_file(&self) -> PathBuf {
        self.root.join("sourcemap.json")
    }

    /// Writes the port actually bound to disk. The editor and the CLI read it.
    /// That removes the "assume it is 8080" guess entirely.
    pub fn write_port_file(&self, port: u16) {
        let dir = self.runtime_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("Could not create the .syncix folder: {}", e);
            return;
        }
        if let Err(e) = fs::write(self.port_file(), port.to_string()) {
            tracing::warn!("Could not write the port file: {}", e);
        }
    }

    /// So the core does not leave a stale port file behind when it exits.
    pub fn clear_port_file(&self) {
        let _ = fs::remove_file(self.port_file());
    }
}

/// On Windows `fs::canonicalize` puts a `\\?\` (extended-length) prefix in front of the path.
/// The path is shown to the user in Studio's approval dialog, so it is cleaned up.
fn clean_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(remaining) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(remaining);
    }
    p
}

/// Says whether two versions can work together.
/// Rule: major and minor must match, the patch may differ.
/// (0.3.1 and 0.3.9 are compatible; 0.3.x and 0.4.x are not.)
pub fn versions_compatible(a: &str, b: &str) -> bool {
    fn major_minor(v: &str) -> (u32, u32) {
        let mut it = v.split('.');
        let major = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let minor = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (major, minor)
    }
    major_minor(a) == major_minor(b)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

#[cfg(test)]
mod config_tests {
    use super::*;

    fn resolve_arg(toml_text: &str) -> ProjectConfig {
        ProjectConfig::resolve_arg(&toml_text.parse::<toml::Value>().unwrap(), ".")
    }

    #[test]
    fn empty_file_gives_defaults() {
        let c = resolve_arg("");
        assert_eq!(c.mode_value, SyncMode::TwoWay);
        assert_eq!(c.play, PlayBehavior::Queue);
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
}

#[cfg(test)]
mod place_identity_tests {
    use super::*;

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
}
