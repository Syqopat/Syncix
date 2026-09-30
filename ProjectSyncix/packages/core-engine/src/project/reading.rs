//! Split out of project.rs.

#[allow(unused_imports)]
use super::*;

/// Small helpers for reading a section from TOML.
/// Unknown keys are not silently swallowed; the caller prints a warning.
pub(crate) fn string_list(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn read_bool(section: Option<&toml::Value>, key_name: &str, fallback_value: bool) -> bool {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_bool())
        .unwrap_or(fallback_value)
}

pub(crate) fn read_number(section: Option<&toml::Value>, key_name: &str, fallback_value: u64) -> u64 {
    section
        .and_then(|b| b.get(key_name))
        .and_then(|x| x.as_integer())
        .and_then(|x| u64::try_from(x).ok())
        .unwrap_or(fallback_value)
}

/// On Windows `fs::canonicalize` puts a `\\?\` (extended-length) prefix in front of the path.
/// The path is shown to the user in Studio's approval dialog, so it is cleaned up.
pub(crate) fn clean_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(remaining) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(remaining);
    }
    p
}
