//! Bringing an older syncix.toml up to date, without losing the user's comments.
//!
//! The file used to be flat: `sync_dir = "src"` at the top level. Sections came later and
//! both shapes are still read, so nobody's file broke -- but a flat file silently missed
//! every setting documented as `[files] sync_dir`, and a key that is no longer used sat
//! there looking effective. The file is rewritten once, in place: keys move into their
//! section, retired keys go, and the comments stay where they were, because the edit is
//! made on the parsed document rather than by re-serialising it.

use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

use super::settings::{self, RETIRED};

pub struct Migration {
    /// What changed, in the words the user sees.
    pub changes: Vec<String>,
    /// The file as it should now be on disk.
    pub text: String,
}

/// Works out what an older file needs. None when it is already current.
pub fn plan(text: &str) -> Option<Migration> {
    let mut doc = text.parse::<DocumentMut>().ok()?;
    let mut changes = Vec::new();

    // Top-level keys that belong in a section.
    let flat: Vec<String> = doc
        .as_table()
        .iter()
        .filter(|(_, item)| !item.is_table())
        .map(|(key, _)| key.to_string())
        .collect();

    for key in flat {
        if let Some((_, why)) = RETIRED.iter().find(|(name, _)| *name == key) {
            doc.as_table_mut().remove(&key);
            changes.push(format!("{} is no longer used and was removed: {}.", key, why));
            continue;
        }
        let Some(setting) = settings::find(&key) else {
            // Not ours to move. An unknown key is reported by the warning check and left
            // alone: it may be something the user keeps there on purpose.
            continue;
        };
        // remove_entry keeps the key itself, and with it the comments written above and
        // beside the line. Re-inserting that same key is what carries the comments along;
        // taking only the value left them behind in the old file.
        let Some((name, item)) = doc.as_table_mut().remove_entry(&key) else {
            continue;
        };
        if !item.is_value() {
            doc.as_table_mut().insert_formatted(&name, item);
            continue;
        }
        ensure_section(&mut doc, setting.section);
        if let Some(target) = doc[setting.section].as_table_mut() {
            target.insert_formatted(&name, item);
        }
        changes.push(format!("{} moved into [{}].", key, setting.section));
    }

    // Retired keys inside a section.
    for section in settings::sections() {
        for (name, why) in RETIRED {
            if doc.get(section).and_then(|s| s.get(name)).is_some() {
                doc[section]
                    .as_table_like_mut()
                    .map(|table| table.remove(name));
                changes.push(format!("{} is no longer used and was removed: {}.", name, why));
            }
        }
    }

    if changes.is_empty() {
        return None;
    }
    Some(Migration {
        changes,
        text: doc.to_string(),
    })
}

fn ensure_section(doc: &mut DocumentMut, section: &str) {
    if doc.get(section).is_none() {
        doc.as_table_mut()
            .insert(section, Item::Table(Table::new()));
    }
}

/// Migrates the file in place, keeping a copy of the old one next to it.
///
/// A backup is written first: this is the user's settings file, and an edit made by a
/// program has to be undoable by hand.
pub fn apply(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Some(migration) = plan(&text) else {
        return Vec::new();
    };

    let backup = path.with_extension("toml.bak");
    if let Err(err) = std::fs::write(&backup, &text) {
        tracing::warn!(
            "syncix.toml was left as it is: the backup could not be written ({}).",
            err
        );
        return Vec::new();
    }
    if let Err(err) = std::fs::write(path, &migration.text) {
        tracing::warn!("syncix.toml could not be updated: {}", err);
        return Vec::new();
    }
    tracing::info!(
        "syncix.toml was brought up to date; the old one is at {}.",
        backup.display()
    );
    for change in &migration.changes {
        tracing::info!("  {}", change);
    }
    migration.changes.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_file_gains_sections_and_keeps_its_comments() {
        let text = "# my project\nsync_dir = \"game\"  # the tree\nport = 9000\n";
        let migration = plan(text).expect("a flat file needs migrating");
        assert!(migration.text.contains("[files]"));
        assert!(migration.text.contains("[server]"));
        assert!(migration.text.contains("# my project"));
        assert!(migration.text.contains("# the tree"));
        assert!(migration.text.contains("\"game\""));
        assert!(migration.text.contains("9000"));
    }

    #[test]
    fn a_retired_key_is_removed_from_a_section() {
        let text = "[sync]\nmode = \"two_way\"\nplay_mode = \"queue\"\n";
        let migration = plan(text).expect("a retired key needs migrating");
        assert!(!migration.text.contains("play_mode"));
        assert!(migration.text.contains("mode = \"two_way\""));
    }

    #[test]
    fn a_current_file_is_left_alone() {
        let text = "[files]\nsync_dir = \"src\"\n\n[server]\nport = 8080\n";
        assert!(plan(text).is_none());
    }

    #[test]
    fn an_unknown_top_level_key_is_not_moved() {
        let text = "whatever = 1\n";
        assert!(plan(text).is_none());
    }
}
