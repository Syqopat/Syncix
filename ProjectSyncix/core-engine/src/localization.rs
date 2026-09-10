//! LocalizationTable içeriği ile CSV arasında dönüşüm.
//!
//! UYARLAMA NOTU:
//! Rojo `.csv` dosyalarını LocalizationTable'a çevirir. Bunu körü körüne kopyalamak
//! yerine önce şuna baktım: LocalizationTable'ın `Contents` property'si zaten bir
//! JSON METNİDİR. Yani bizim current_value String property yolumuza olduğu gibi oturuyor;
//! çeviri tablolarını syncing etmek için fresh bir kanal açmaya gerek yok.
//!
//! Geriye single soru kalıyor: diske JSON olarak mı yazalım, CSV olarak mı?
//! CSV'nin single gerçek faydası çevirilerin tabloda (Excel, Sheets) düzenlenebilmesi.
//! Ama dönüşüm KAYIPLI olursa çeviri verisi bozulur — virgül, tırnak, satır sonu
//! ve boş hücre içeren çeviriler gerçek hayatta çok yaygındır.
//!
//! Bu yüzden dönüşümü kayıpsız yazdım ve gidiş-dönüş testleriyle kilitledim:
//! virgüllü, tırnaklı, satır sonlu, unicode ve boş değerli input_list test ediliyor.
//! Kayıpsız olduğunu kanıtlayamasaydım CSV'yi atlar, JSON'da bırakırdım.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Roblox LocalizationTable girdisi.
///
/// ALAN ADLARI ROBLOX'A AITTIR, PascalCase'dir. Ilk denemede kucuk harf
/// kullanilmisti ve Studio soyle dedi:
///   "Entry at index 1 had invalid Key/Source/Context: Cannot create entry
///    with empty source text and no key."
/// Yani alanlar hic taninmamis, hepsi bos sayilmisti. Bu yuzden isimler
/// serde rename ile birebir Roblox bicimine sabitlendi.
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
// CSV yazma/okuma
//
// Kendi CSV işleyicimizi yazıyoruz çünkü ihtiyacımız dar ve kurallar nettir
// (RFC 4180). Bir kütüphane eklemek bu kadarı için gereksiz ağırlık olurdu,
// ama kayıpsızlık şart olduğu için kaçış kuralları testlerle kilitleniyor.
// ---------------------------------------------------------------------------

fn write_cell(s: &str) -> String {
    // Virgül, tırnak, satır sonu veya baştaki/sondaki boşluk varsa tırnaklanır;
    // içerideki tırnaklar ikiye katlanır.
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

/// Bir CSV satırını hücrelere ayırır (tırnak içindeki virgül ve satır sonları korunur).
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

/// Tüm metni satırlara böler; tırnak içindeki satır sonları bölme sayılmaz.
fn split_lines(text_value: &str) -> Vec<String> {
    let mut line_list = Vec::new();
    let mut current_value = String::new();
    let mut in_quotes = false;
    let mut char_list = text_value.chars().peekable();

    while let Some(c) = char_list.next() {
        match c {
            '"' => {
                // Kaçırılmış tırnak (""), tırnak durumunu değiştirmez.
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
            '\r' if !in_quotes => { /* CRLF: \r atlanır */ }
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

    // Diller birleşiminden sütun düzeni; sıralı olmalı ki çıktı belirleyici olsun.
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
        return Err("CSV başlığı en az Key,Source,Context,Example içermeli".to_string());
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
            // Boş hücre "çeviri yok" demektir; JSON'a boş dize yazmak
            // gidiş-dönüşte olmayan bir çeviri uydurmak olurdu.
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
                "Key": "selam",
                "Source": "Hello",
                "Context": "",
                "Values": { "tr": "Merhaba", "de": "Hallo" }
            },
            {
                "Key": "veda",
                "Source": "Bye",
                "Context": "ana_menu",
                "Values": { "tr": "Hoşça kal" }
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
        assert_eq!(first_item, "Key,Source,Context,Example,de,tr");
    }

    /// Gercek hayattaki ceviriler virgul, tirnak ve line_text sonu icerir.
    /// Bunlar bozulursa ceviri verisi sessizce kaybolur.
    #[test]
    fn tricky_characters_are_lossless() {
        let tricky = serde_json::json!([{
            "Key": "zor",
            "Source": "a,b \"c\" d",
            "Context": "",
            "Values": {
                "tr": "virgul, tirnak \" ve\nsatir sonu",
                "en": " bosluklu "
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
            "Values": { "tr": "Hoşça kal ğüşiöç", "ja": "こんにちは", "emoji": "🎮🚀" }
        }])
        .to_string();

        let csv = json_to_csv(&u).unwrap();
        let restored_count = csv_to_json(&csv).unwrap();
        let a: Vec<Entry> = serde_json::from_str(&u).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&restored_count).unwrap();
        assert_eq!(a, b);
    }

    /// Bos hucre "ceviri yok" demektir; bos dize olarak restored_count gelmemeli,
    /// yoksa olmayan bir ceviri uydurmus oluruz.
    #[test]
    fn empty_cell_invents_no_translation() {
        let input_value = serde_json::json!([
            { "Key": "a", "Source": "A", "Context": "", "Values": { "tr": "A-tr" } },
            { "Key": "b", "Source": "B", "Context": "", "Values": { "de": "B-de" } }
        ])
        .to_string();

        let csv = json_to_csv(&input_value).unwrap();
        let restored_count: Vec<Entry> = serde_json::from_str(&csv_to_json(&csv).unwrap()).unwrap();

        assert_eq!(restored_count[0].values.len(), 1, "a icin yalnizca tr olmali");
        assert!(!restored_count[0].values.contains_key("de"));
        assert_eq!(restored_count[1].values.len(), 1, "b icin yalnizca de olmali");
    }

    #[test]
    fn empty_table_does_not_crash() {
        assert_eq!(json_to_csv("[]").unwrap().trim(), "Key,Source,Context,Example");
        assert_eq!(csv_to_json("Key,Source,Context,Example\n").unwrap(), "[]");
    }

    #[test]
    fn bad_input_errors_without_panic() {
        assert!(json_to_csv("{bozuk").is_err());
        assert!(csv_to_json("A,B").is_err());
    }

    #[test]
    fn crlf_line_endings_are_accepted() {
        let csv = "Key,Source,Context,Example,tr\r\nselam,Hello,,,Merhaba\r\n";
        let input_list: Vec<Entry> = serde_json::from_str(&csv_to_json(csv).unwrap()).unwrap();
        assert_eq!(input_list.len(), 1);
        assert_eq!(input_list[0].key, "selam");
        assert_eq!(input_list[0].values.get("tr").unwrap(), "Merhaba");
    }
}
