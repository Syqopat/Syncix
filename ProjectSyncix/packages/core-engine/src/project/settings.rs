//! Every setting syncix.toml understands, described once.
//!
//! The list used to exist twice: once as the names the warning check knew and once as
//! the prose in the example file. A setting added to one and forgotten in the other was
//! reported as a typo. Now the warning check, the JSON schema for editors and the
//! migration all read this table.

/// What kind of value a setting takes. It decides the JSON schema type and is what the
/// warning check compares against.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Bool,
    /// A whole number, with the range that is actually accepted.
    Integer(i64, i64),
    Text,
    TextList,
    /// One of these words.
    Choice(&'static [&'static str]),
}

pub struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub kind: Kind,
    /// Default as it would be written in TOML.
    pub default: &'static str,
    pub doc: &'static str,
}

pub const SYNC_MODES: [&str; 4] = ["two_way", "studio_to_disk", "disk_to_studio", "manual"];

/// Keys that were understood once and are now ignored, with what to do instead.
pub const RETIRED: &[(&str, &str)] = &[(
    "play_mode",
    "changes go to the edit session even during a playtest, and a running test sees them \
     after a restart",
)];

pub const SETTINGS: &[Setting] = &[
    Setting {
        section: "sync",
        key: "mode",
        kind: Kind::Choice(&["two_way", "studio_to_disk", "disk_to_studio", "manual"]),
        default: "\"two_way\"",
        doc: "Which directions are active.",
    },
    Setting {
        section: "sync",
        key: "debounce_ms",
        kind: Kind::Integer(10, 10_000),
        default: "120",
        doc: "How long the writer waits for changes to settle before writing to disk.",
    },
    Setting {
        section: "sync",
        key: "ask_permission",
        kind: Kind::Bool,
        default: "false",
        doc: "Ask in Studio the first time a project connects.",
    },
    Setting {
        section: "sync",
        key: "undo",
        kind: Kind::Bool,
        default: "true",
        doc: "Put Syncix's own changes on Studio's undo stack.",
    },
    Setting {
        section: "files",
        key: "sync_dir",
        kind: Kind::Text,
        default: "\"src\"",
        doc: "Folder that mirrors the Roblox tree.",
    },
    Setting {
        section: "files",
        key: "ignore",
        kind: Kind::TextList,
        default: "[]",
        doc: "Glob patterns that are never read, written or deleted.",
    },
    Setting {
        section: "files",
        key: "meta_files",
        kind: Kind::Bool,
        default: "true",
        doc: "Write a .meta.json next to a script for its properties and attributes.",
    },
    Setting {
        section: "safety",
        key: "trash",
        kind: Kind::Bool,
        default: "true",
        doc: "Move deleted files to .syncix/trash instead of removing them.",
    },
    Setting {
        section: "safety",
        key: "trash_keep",
        kind: Kind::Integer(1, 500),
        default: "10",
        doc: "How many runs of deleted files the trash keeps.",
    },
    Setting {
        section: "safety",
        key: "delete_grace_ms",
        kind: Kind::Integer(0, 30_000),
        default: "800",
        doc: "How long a file deleted on disk waits before it counts as a deletion; a \
              move arrives as delete plus create.",
    },
    Setting {
        section: "safety",
        key: "confirm_delete",
        kind: Kind::Bool,
        default: "true",
        doc: "Ask before `syncix rm` deletes something.",
    },
    Setting {
        section: "scope",
        key: "services",
        kind: Kind::TextList,
        default: "[]",
        doc: "Services to observe. Empty means the plugin's own list.",
    },
    Setting {
        section: "scope",
        key: "ignore_classes",
        kind: Kind::TextList,
        default: "[]",
        doc: "Classes that are never synced.",
    },
    Setting {
        section: "scope",
        key: "ignore_properties",
        kind: Kind::TextList,
        default: "[]",
        doc: "Properties that are never synced.",
    },
    Setting {
        section: "server",
        key: "port",
        kind: Kind::Integer(1, 65_535),
        default: "8080",
        doc: "Port the core asks for. If it is taken, the next one is tried.",
    },
    Setting {
        section: "server",
        key: "job_workers",
        kind: Kind::Integer(1, 16),
        default: "2",
        doc: "How many background jobs (the full-tree disk write) may run at once.",
    },
    Setting {
        section: "editor",
        key: "sourcemap",
        kind: Kind::Bool,
        default: "true",
        doc: "Keep sourcemap.json up to date for luau-lsp.",
    },
];

/// The sections, in the order they are declared.
pub fn sections() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for setting in SETTINGS {
        if !out.contains(&setting.section) {
            out.push(setting.section);
        }
    }
    out
}

/// The keys of one section.
pub fn keys_of(section: &str) -> Vec<&'static str> {
    SETTINGS
        .iter()
        .filter(|s| s.section == section)
        .map(|s| s.key)
        .collect()
}

/// Every key, whatever section it belongs to. Old flat files keep them at the top level.
pub fn all_keys() -> Vec<&'static str> {
    SETTINGS.iter().map(|s| s.key).collect()
}

pub fn find(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.key == key)
}

/// JSON Schema for syncix.toml, for an editor that validates TOML against one
/// (Even Better TOML, taplo). `syncix config --schema` prints it.
pub fn json_schema() -> String {
    let mut sections_json = Vec::new();
    for section in sections() {
        let mut props = Vec::new();
        for setting in SETTINGS.iter().filter(|s| s.section == section) {
            props.push(format!(
                "        \"{}\": {{\n          \"description\": {},\n          \"default\": {},\n{}        }}",
                setting.key,
                json_text(setting.doc),
                setting.default,
                type_json(setting.kind)
            ));
        }
        sections_json.push(format!(
            "    \"{}\": {{\n      \"type\": \"object\",\n      \"additionalProperties\": false,\n      \"properties\": {{\n{}\n      }}\n    }}",
            section,
            props.join(",\n")
        ));
    }

    format!(
        "{{\n  \"$schema\": \"http://json-schema.org/draft-07/schema#\",\n  \"title\": \"Syncix project settings\",\n  \"type\": \"object\",\n  \"additionalProperties\": false,\n  \"properties\": {{\n{}\n  }}\n}}\n",
        sections_json.join(",\n")
    )
}

fn type_json(kind: Kind) -> String {
    match kind {
        Kind::Bool => "          \"type\": \"boolean\"\n".to_string(),
        Kind::Text => "          \"type\": \"string\"\n".to_string(),
        Kind::TextList => {
            "          \"type\": \"array\",\n          \"items\": { \"type\": \"string\" }\n"
                .to_string()
        }
        Kind::Integer(low, high) => format!(
            "          \"type\": \"integer\",\n          \"minimum\": {},\n          \"maximum\": {}\n",
            low, high
        ),
        Kind::Choice(options) => format!(
            "          \"type\": \"string\",\n          \"enum\": [{}]\n",
            options
                .iter()
                .map(|o| format!("\"{}\"", o))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn json_text(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_keys_and_every_key_is_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for setting in SETTINGS {
            assert!(!seen.contains(&setting.key), "duplicate key {}", setting.key);
            seen.push(setting.key);
        }
        for section in sections() {
            assert!(!keys_of(section).is_empty(), "empty section {}", section);
        }
    }

    #[test]
    fn the_schema_is_valid_json_and_covers_every_setting() {
        let text = json_schema();
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        for setting in SETTINGS {
            assert!(
                value["properties"][setting.section]["properties"][setting.key].is_object(),
                "{}.{} missing from the schema",
                setting.section,
                setting.key
            );
        }
    }

    /// The schema in the repository is what an editor downloads, so it has to be the
    /// schema this build would print. Regenerate it with:
    ///   syncix config --schema > schemas/syncix.schema.json
    #[test]
    fn the_committed_schema_is_up_to_date() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/syncix.schema.json");
        let on_disk = std::fs::read_to_string(&path).expect("schemas/syncix.schema.json");
        assert_eq!(
            on_disk.replace("\r\n", "\n"),
            json_schema(),
            "schemas/syncix.schema.json is out of date; regenerate it with \
             `syncix config --schema`"
        );
    }

    #[test]
    fn the_sync_mode_list_matches_the_setting() {
        let Some(Kind::Choice(options)) = find("mode").map(|s| s.kind) else {
            panic!("mode should be a choice");
        };
        assert_eq!(options, SYNC_MODES);
    }
}
