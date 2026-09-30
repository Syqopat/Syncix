//! Project configuration and identity.
//!
//! Two things here matter for a release:
//!  1. The port is no longer fixed. It is read from syncix.toml; if taken the next one is
//!     tried and the CHOSEN port is written to disk. The editor and the CLI read that file instead of guessing.
//!  2. The core introduces itself (project name, root directory, version). The Studio plugin
//!     needs this to show the user which project it connected to.

pub(crate) mod migrate;
mod mode;
mod reading;
pub(crate) mod settings;
mod warnings;

pub(crate) use mode::*;
pub(crate) use reading::*;
pub(crate) use warnings::*;

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
    /// How many background jobs may run at once (the full-tree disk write).
    pub job_workers: usize,
    /// True when the port was requested explicitly on the command line; then there is no fallback.
    /// Reason: the user types the same port into the Studio plugin; if the core silently
    /// moved to another port the two sides would diverge for no visible reason.
    pub port_fixed: bool,

    /// Sync direction.
    pub mode_value: SyncMode,
    /// Wait time of the disk writer. A small value reflects changes faster but
    /// increases the number of writes.
    pub debounce_ms: u64,
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


impl ProjectConfig {
    /// What load() would find wrong in syncix.toml, for `syncix config`.
    pub fn warnings() -> Vec<String> {
        for config_path in ["../syncix.toml", "syncix.toml"] {
            let Ok(text) = fs::read_to_string(config_path) else {
                continue;
            };
            return match text.parse::<toml::Value>() {
                Ok(value) => config_warnings(&value),
                Err(e) => vec![format!("syncix.toml could not be read: {}", e)],
            };
        }
        Vec::new()
    }

    /// Looks for syncix.toml upwards from the working directory.
    /// The core can be run from the project root as well as from inside core-engine/.
    pub fn load() -> Self {
        for (config_path, base) in [("../syncix.toml", ".."), ("syncix.toml", ".")] {
            let path = Path::new(config_path);
            if !path.exists() {
                continue;
            }
            // An older file is brought up to date once, before it is read, so what the
            // core applies and what the file says cannot disagree.
            migrate::apply(path);
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
        for warning in config_warnings(value) {
            tracing::warn!("syncix.toml: {}", warning);
        }
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
                // Reported by config_warnings, with the closest mode.
                SyncMode::resolve_arg(&s)
            })
            .unwrap_or(SyncMode::TwoWay);

        // play_mode is gone. Changes always reach the edit session, even while a
        // playtest runs (Studio runs the test in a separate copy); measured, the queue
        // it chose never took effect. A setting someone wrote is not ignored silently.
        if al(sync, "play_mode").is_some() {
            tracing::warn!(
                "play_mode in syncix.toml is no longer used and can be removed: changes go to the \
                 edit session even during a playtest, and a running test sees them after a restart."
            );
        }

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
            job_workers: read_number(server, "job_workers", 2).clamp(1, 16) as usize,
            port_fixed: false,

            mode_value,
            debounce_ms: read_number(sync, "debounce_ms", 120).clamp(10, 10_000),
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
mod tests;
