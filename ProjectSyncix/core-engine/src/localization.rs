//! Conversion between LocalizationTable contents and CSV.
//!
//! ADAPTATION NOTE:
//! Rojo turns `.csv` files into LocalizationTables. Rather than copying that blindly,
//! the first question was: a LocalizationTable's `Contents` property already is a
//! JSON STRING. So it fits the existing String property path as is;
//! no new channel is needed to sync translation tables.
//!
//! One question remains: write it to disk as JSON or as CSV?
//! CSV's one real benefit is that translations can be edited in a table (Excel, Sheets).
//! But if the conversion were LOSSY, translation data would be corrupted — translations
//! containing commas, quotes, line breaks and empty cells are very common in practice.
//!
//! So the conversion is lossless and locked with round-trip tests:
//! inputs with commas, quotes, line breaks, unicode and empty values are all tested.
//! Had losslessness not been provable, CSV would have been dropped and JSON kept.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A Roblox LocalizationTable entry.
///
/// THE FIELD NAMES BELONG TO ROBLOX AND ARE PascalCase. The first attempt used
/// lowercase, and Studio said:
///   "Entry at index 1 had invalid Key/Source/Context: Cannot create entry
///    with empty source text and no key."
/// So none of the fields was recognised and all were treated as empty. That is why the
/// names are pinned to Roblox's exact form with serde rename.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Entry {
    #[serde(rename = "Key", default)]
    pub key: String,
    #[serde(rename = "Source", default)]
    pub source: String,
    #[serde(rename = "Context", default)]
    pub context: String,
    #[serde(rename = "Example", default, skip_serializing_if = "String::is_empty")]
    pub examples: String,
    #[serde(rename = "Values", default)]
    pub values: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// CSV writing/reading
//
// We write our own CSV handler because our needs are narrow and the rules are clear
// (RFC 4180). Adding a library for this much would be needless weight,
// but losslessness is mandatory, so the escaping rules are locked by tests.
// ---------------------------------------------------------------------------

fn write_cell(s: &str) -> String {
    // Quoted when it contains a comma, quote, line break or leading/trailing space;
    // quotes inside are doubled.
    let needs_escape = s.contains(',')
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.starts_with(' ')
        || s.ends_with(' ');
    if needs_escape {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Splits a CSV row into cells (commas and line breaks inside quotes are kept).
fn split_line(input_value: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current_value = String::new();
    let mut in_quotes = false;
    let mut char_list = input_value.chars().peekable();

    while let Some(c) = char_list.next() {
        if in_quotes {
            if c == '"' {
                if char_list.peek() == Some(&'"') {
                    current_value.push('"');
                    char_list.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current_value.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            cells.push(std::mem::take(&mut current_value));
        } else {
            current_value.push(c);
        }
    }
    cells.push(current_value);
    cells
}

/// Splits the whole text into rows; line breaks inside quotes do not split.
fn split_lines(text_value: &str) -> Vec<String> {
    let mut line_list = Vec::new();
    let mut current_value = String::new();
    let mut in_quotes = false;
    let mut char_list = text_value.chars().peekable();

    while let Some(c) = char_list.next() {
        match c {
            '"' => {
                // An escaped quote ("") does not change the quote state.
                if in_quotes && char_list.peek() == Some(&'"') {
                    current_value.push('"');
                    current_value.push('"');
                    char_list.next();
                } else {
                    in_quotes = !in_quotes;
                    current_value.push('"');
                }
            }
            '\n' if !in_quotes => {
                line_list.push(std::mem::take(&mut current_value));
            }
            '\r' if !in_quotes => { /* CRLF: \r is skipped */ }
            _ => current_value.push(c),
        }
    }
    if !current_value.is_empty() {
        line_list.push(current_value);
    }
    line_list
}

/// LocalizationTable.Contents (JSON) -> CSV
pub fn json_to_csv(contents: &str) -> Result<String, String> {
    let input_list: Vec<Entry> =
        serde_json::from_str(contents).map_err(|e| format!("Could not parse Contents: {}", e))?;

    // Column order from the union of languages; sorted so the output is deterministic.
    let mut locales: Vec<String> = Vec::new();
    for g in &input_list {
        for d in g.values.keys() {
            if !locales.contains(d) {
                locales.push(d.clone());
            }
        }
    }
    locales.sort();

    let mut out_text = String::new();
    let mut headers = vec!["Key".to_string(), "Source".to_string(), "Context".to_string(), "Example".to_string()];
    headers.extend(locales.iter().cloned());
    out_text.push_str(&headers.iter().map(|h| write_cell(h)).collect::<Vec<_>>().join(","));
    out_text.push('\n');

    for g in &input_list {
        let mut line_text = vec![
            write_cell(&g.key),
            write_cell(&g.source),
            write_cell(&g.context),
            write_cell(&g.examples),
        ];
        for d in &locales {
            line_text.push(write_cell(g.values.get(d).map(|s| s.as_str()).unwrap_or("")));
        }
        out_text.push_str(&line_text.join(","));
        out_text.push('\n');
    }

    Ok(out_text)
}

/// CSV -> LocalizationTable.Contents (JSON)
pub fn csv_to_json(csv: &str) -> Result<String, String> {
    let line_list = split_lines(csv);
    if line_list.is_empty() {
        return Ok("[]".to_string());
    }

    let headers = split_line(&line_list[0]);
    if headers.len() < 4 {
        return Err("The CSV header must contain at least Key,Source,Context,Example".to_string());
    }

    let mut input_list: Vec<Entry> = Vec::new();
    for line_text in line_list.iter().skip(1) {
        if line_text.trim().is_empty() {
            continue;
        }
        let cells = split_line(line_text);
        let al = |i: usize| cells.get(i).cloned().unwrap_or_default();

        let mut values = BTreeMap::new();
        for (i, title) in headers.iter().enumerate().skip(4) {
            let raw_value = al(i);
            // An empty cell means "no translation"; writing an empty string to JSON
            // would invent a translation that never existed in the round trip.
            if !raw_value.is_empty() {
                values.insert(title.clone(), raw_value);
            }
        }

        input_list.push(Entry {
            key: al(0),
            source: al(1),
            context: al(2),
            examples: al(3),
            values,
        });
    }

    serde_json::to_string(&input_list).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        serde_json::json!([
            {
                "Key": "greeting",
                "Source": "Hello",
                "Context": "",
                "Values": { "es": "Hola", "de": "Hallo" }
            },
            {
                "Key": "farewell",
                "Source": "Bye",
                "Context": "main_menu",
                "Values": { "es": "Adiós" }
            }
        ])
        .to_string()
    }

    #[test]
    fn json_csv_round_trip_basic() {
        let csv = json_to_csv(&sample()).unwrap();
        let restored_count = csv_to_json(&csv).unwrap();

        let a: Vec<Entry> = serde_json::from_str(&sample()).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&restored_count).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn header_row_is_correct() {
        let csv = json_to_csv(&sample()).unwrap();
        let first_item = csv.lines().next().unwrap();
        assert_eq!(first_item, "Key,Source,Context,Example,de,es");
    }

    /// Real-world translations contain commas, quotes and line breaks.
    /// If those break, translation data is silently lost.
    #[test]
    fn tricky_characters_are_lossless() {
        let tricky = serde_json::json!([{
            "Key": "tricky",
            "Source": "a,b \"c\" d",
            "Context": "",
            "Values": {
                "es": "coma, comilla \" y\nsalto de linea",
                "en": " with spaces "
            }
        }])
        .to_string();

        let csv = json_to_csv(&tricky).unwrap();
        let restored_count = csv_to_json(&csv).unwrap();

        let a: Vec<Entry> = serde_json::from_str(&tricky).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&restored_count).unwrap();
        assert_eq!(a, b, "csv:\n{}", csv);
    }

    #[test]
    fn unicode_is_preserved() {
        let u = serde_json::json!([{
            "Key": "u",
            "Source": "x",
            "Context": "",
            "Values": { "es": "Adiós, señor: ñáéíóú ¿¡", "ja": "こんにちは", "emoji": "🎮🚀" }
        }])
        .to_string();

        let csv = json_to_csv(&u).unwrap();
        let restored_count = csv_to_json(&csv).unwrap();
        let a: Vec<Entry> = serde_json::from_str(&u).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&restored_count).unwrap();
        assert_eq!(a, b);
    }

    /// An empty cell means "no translation"; it must not come back as an empty string,
    /// or we would have invented a translation.
    #[test]
    fn empty_cell_invents_no_translation() {
        let input_value = serde_json::json!([
            { "Key": "a", "Source": "A", "Context": "", "Values": { "es": "A-es" } },
            { "Key": "b", "Source": "B", "Context": "", "Values": { "de": "B-de" } }
        ])
        .to_string();

        let csv = json_to_csv(&input_value).unwrap();
        let restored_count: Vec<Entry> = serde_json::from_str(&csv_to_json(&csv).unwrap()).unwrap();

        assert_eq!(restored_count[0].values.len(), 1, "only es is expected for a");
        assert!(!restored_count[0].values.contains_key("de"));
        assert_eq!(restored_count[1].values.len(), 1, "only de is expected for b");
    }

    #[test]
    fn empty_table_does_not_crash() {
        assert_eq!(json_to_csv("[]").unwrap().trim(), "Key,Source,Context,Example");
        assert_eq!(csv_to_json("Key,Source,Context,Example\n").unwrap(), "[]");
    }

    #[test]
    fn bad_input_errors_without_panic() {
        assert!(json_to_csv("{broken").is_err());
        assert!(csv_to_json("A,B").is_err());
    }

    #[test]
    fn crlf_line_endings_are_accepted() {
        let csv = "Key,Source,Context,Example,es\r\ngreeting,Hello,,,Hola\r\n";
        let input_list: Vec<Entry> = serde_json::from_str(&csv_to_json(csv).unwrap()).unwrap();
        assert_eq!(input_list.len(), 1);
        assert_eq!(input_list[0].key, "greeting");
        assert_eq!(input_list[0].values.get("es").unwrap(), "Hola");
    }
}
