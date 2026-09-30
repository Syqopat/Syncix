//! Split out of project.rs.

#[allow(unused_imports)]
use super::*;

use super::settings::{self, RETIRED, SYNC_MODES};

/// Is this a key that was understood once and is now ignored? Such a key is removed
/// from the file by the migration, so it is named, not treated as a typo.
fn retired(key: &str) -> Option<&'static str> {
    RETIRED.iter().find(|(name, _)| *name == key).map(|(_, why)| *why)
}

/// Sections and their keys, from the one table that describes every setting.
fn known_keys() -> Vec<(&'static str, Vec<&'static str>)> {
    settings::sections()
        .into_iter()
        .map(|section| (section, settings::keys_of(section)))
        .collect()
}

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
    let known = known_keys();
    let sections = known.iter().map(|(section, _)| *section);
    let flat_keys: Vec<&str> = settings::all_keys();

    for (key, entry) in table {
        if let Some(why) = retired(key) {
            out.push(format!("{} is no longer used and is removed from syncix.toml: {}.", key, why));
            continue;
        }
        match (entry.as_table(), known.iter().find(|(section, _)| section == key)) {
            (Some(inner), Some((section, keys))) => {
                for inner_key in inner.keys().filter(|k| !keys.contains(&k.as_str())) {
                    if let Some(why) = retired(inner_key) {
                        out.push(format!(
                            "{} is no longer used and is removed from syncix.toml: {}.",
                            inner_key, why
                        ));
                        continue;
                    }
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
