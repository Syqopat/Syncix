//! Split out of project.rs.

#[allow(unused_imports)]
use super::*;

/// Every key syncix.toml understands, per section. Anything else is a typo that used to
/// be skipped without a word, leaving the setting at its default.
pub(crate) const KNOWN_KEYS: &[(&str, &[&str])] = &[
    ("sync", &["mode", "debounce_ms", "ask_permission", "undo", "play_mode"]),
    ("files", &["sync_dir", "ignore", "meta_files"]),
    ("safety", &["trash", "trash_keep", "delete_grace_ms", "confirm_delete"]),
    ("scope", &["services", "ignore_classes", "ignore_properties"]),
    ("server", &["port", "job_workers"]),
    ("editor", &["sourcemap"]),
];

pub(crate) const SYNC_MODES: [&str; 4] = ["two_way", "studio_to_disk", "disk_to_studio", "manual"];

pub(crate) fn with_hint<'a>(message: String, typed: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    match crate::suggest::hint(typed, candidates) {
        Some(hint) => format!("{} {}", message, hint),
        None => message,
    }
}

/// What in syncix.toml is not understood, with the closest valid spelling. The file still
/// loads: a typo must not stop sync, it falls back to the default and is reported.
pub fn config_warnings(value: &toml::Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(table) = value.as_table() else { return out };
    let sections = KNOWN_KEYS.iter().map(|(section, _)| *section);
    let flat_keys: Vec<&str> = KNOWN_KEYS.iter().flat_map(|(_, keys)| keys.iter().copied()).collect();

    for (key, entry) in table {
        match (entry.as_table(), KNOWN_KEYS.iter().find(|(section, _)| section == key)) {
            (Some(inner), Some((section, keys))) => {
                for inner_key in inner.keys().filter(|k| !keys.contains(&k.as_str())) {
                    out.push(with_hint(
                        format!("Unknown setting {} in [{}].", inner_key, section),
                        inner_key,
                        keys.iter().copied(),
                    ));
                }
            }
            (Some(_), None) => out.push(with_hint(format!("Unknown section [{}].", key), key, sections.clone())),
            // Old flat files keep the keys at the top level.
            (None, _) if !flat_keys.contains(&key.as_str()) => {
                out.push(with_hint(format!("Unknown setting {}.", key), key, flat_keys.iter().copied()))
            }
            _ => {}
        }
    }

    let read = |section: &str, key: &str| value.get(section).and_then(|s| s.get(key)).or_else(|| value.get(key));
    if let Some(mode) = read("sync", "mode").and_then(|m| m.as_str()) {
        if SyncMode::resolve_arg(mode).is_none() {
            out.push(with_hint(format!("mode = \"{}\" is not a sync mode.", mode), mode, SYNC_MODES));
        }
    }
    for service in string_list(read("scope", "services")) {
        if !crate::catalog::is_class(&service) {
            out.push(with_hint(
                format!("services: {} is not a Roblox service.", service),
                &service,
                crate::rbxmx_import::service_names(),
            ));
        }
    }
    for class in string_list(read("scope", "ignore_classes")) {
        if !crate::catalog::is_class(&class) {
            out.push(with_hint(
                format!("ignore_classes: {} is not a Roblox class.", class),
                &class,
                crate::catalog::class_names(),
            ));
        }
    }
    out
}
