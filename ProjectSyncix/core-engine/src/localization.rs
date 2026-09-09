//! LocalizationTable içeriği ile CSV arasında dönüşüm.
//!
//! UYARLAMA NOTU:
//! Rojo `.csv` dosyalarını LocalizationTable'a çevirir. Bunu körü körüne kopyalamak
//! yerine önce şuna baktım: LocalizationTable'ın `Contents` property'si zaten bir
//! JSON METNİDİR. Yani bizim mevcut String property yolumuza olduğu gibi oturuyor;
//! çeviri tablolarını senkron etmek için yeni bir kanal açmaya gerek yok.
//!
//! Geriye tek soru kalıyor: diske JSON olarak mı yazalım, CSV olarak mı?
//! CSV'nin tek gerçek faydası çevirilerin tabloda (Excel, Sheets) düzenlenebilmesi.
//! Ama dönüşüm KAYIPLI olursa çeviri verisi bozulur — virgül, tırnak, satır sonu
//! ve boş hücre içeren çeviriler gerçek hayatta çok yaygındır.
//!
//! Bu yüzden dönüşümü kayıpsız yazdım ve gidiş-dönüş testleriyle kilitledim:
//! virgüllü, tırnaklı, satır sonlu, unicode ve boş değerli girdiler test ediliyor.
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

fn hucre_yaz(s: &str) -> String {
    // Virgül, tırnak, satır sonu veya baştaki/sondaki boşluk varsa tırnaklanır;
    // içerideki tırnaklar ikiye katlanır.
    let kacis_gerek = s.contains(',')
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.starts_with(' ')
        || s.ends_with(' ');
    if kacis_gerek {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Bir CSV satırını hücrelere ayırır (tırnak içindeki virgül ve satır sonları korunur).
fn satir_ayir(girdi: &str) -> Vec<String> {
    let mut hucreler = Vec::new();
    let mut mevcut = String::new();
    let mut tirnakta = false;
    let mut karakterler = girdi.chars().peekable();

    while let Some(c) = karakterler.next() {
        if tirnakta {
            if c == '"' {
                if karakterler.peek() == Some(&'"') {
                    mevcut.push('"');
                    karakterler.next();
                } else {
                    tirnakta = false;
                }
            } else {
                mevcut.push(c);
            }
        } else if c == '"' {
            tirnakta = true;
        } else if c == ',' {
            hucreler.push(std::mem::take(&mut mevcut));
        } else {
            mevcut.push(c);
        }
    }
    hucreler.push(mevcut);
    hucreler
}

/// Tüm metni satırlara böler; tırnak içindeki satır sonları bölme sayılmaz.
fn satirlara_bol(metin: &str) -> Vec<String> {
    let mut satirlar = Vec::new();
    let mut mevcut = String::new();
    let mut tirnakta = false;
    let mut karakterler = metin.chars().peekable();

    while let Some(c) = karakterler.next() {
        match c {
            '"' => {
                // Kaçırılmış tırnak (""), tırnak durumunu değiştirmez.
                if tirnakta && karakterler.peek() == Some(&'"') {
                    mevcut.push('"');
                    mevcut.push('"');
                    karakterler.next();
                } else {
                    tirnakta = !tirnakta;
                    mevcut.push('"');
                }
            }
            '\n' if !tirnakta => {
                satirlar.push(std::mem::take(&mut mevcut));
            }
            '\r' if !tirnakta => { /* CRLF: \r atlanır */ }
            _ => mevcut.push(c),
        }
    }
    if !mevcut.is_empty() {
        satirlar.push(mevcut);
    }
    satirlar
}

/// LocalizationTable.Contents (JSON) -> CSV
pub fn json_to_csv(contents: &str) -> Result<String, String> {
    let girdiler: Vec<Entry> =
        serde_json::from_str(contents).map_err(|e| format!("Could not parse Contents: {}", e))?;

    // Diller birleşiminden sütun düzeni; sıralı olmalı ki çıktı belirleyici olsun.
    let mut diller: Vec<String> = Vec::new();
    for g in &girdiler {
        for d in g.values.keys() {
            if !diller.contains(d) {
                diller.push(d.clone());
            }
        }
    }
    diller.sort();

    let mut cikti = String::new();
    let mut basliklar = vec!["Key".to_string(), "Source".to_string(), "Context".to_string(), "Example".to_string()];
    basliklar.extend(diller.iter().cloned());
    cikti.push_str(&basliklar.iter().map(|h| hucre_yaz(h)).collect::<Vec<_>>().join(","));
    cikti.push('\n');

    for g in &girdiler {
        let mut satir = vec![
            hucre_yaz(&g.key),
            hucre_yaz(&g.source),
            hucre_yaz(&g.context),
            hucre_yaz(&g.examples),
        ];
        for d in &diller {
            satir.push(hucre_yaz(g.values.get(d).map(|s| s.as_str()).unwrap_or("")));
        }
        cikti.push_str(&satir.join(","));
        cikti.push('\n');
    }

    Ok(cikti)
}

/// CSV -> LocalizationTable.Contents (JSON)
pub fn csv_to_json(csv: &str) -> Result<String, String> {
    let satirlar = satirlara_bol(csv);
    if satirlar.is_empty() {
        return Ok("[]".to_string());
    }

    let basliklar = satir_ayir(&satirlar[0]);
    if basliklar.len() < 4 {
        return Err("CSV başlığı en az Key,Source,Context,Example içermeli".to_string());
    }

    let mut girdiler: Vec<Entry> = Vec::new();
    for satir in satirlar.iter().skip(1) {
        if satir.trim().is_empty() {
            continue;
        }
        let hucreler = satir_ayir(satir);
        let al = |i: usize| hucreler.get(i).cloned().unwrap_or_default();

        let mut values = BTreeMap::new();
        for (i, baslik) in basliklar.iter().enumerate().skip(4) {
            let deger = al(i);
            // Boş hücre "çeviri yok" demektir; JSON'a boş dize yazmak
            // gidiş-dönüşte olmayan bir çeviri uydurmak olurdu.
            if !deger.is_empty() {
                values.insert(baslik.clone(), deger);
            }
        }

        girdiler.push(Entry {
            key: al(0),
            source: al(1),
            context: al(2),
            examples: al(3),
            values,
        });
    }

    serde_json::to_string(&girdiler).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek() -> String {
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
    fn json_csv_gidis_donus_temel() {
        let csv = json_to_csv(&ornek()).unwrap();
        let geri = csv_to_json(&csv).unwrap();

        let a: Vec<Entry> = serde_json::from_str(&ornek()).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&geri).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn baslik_satiri_dogru() {
        let csv = json_to_csv(&ornek()).unwrap();
        let ilk = csv.lines().next().unwrap();
        assert_eq!(ilk, "Key,Source,Context,Example,de,tr");
    }

    /// Gercek hayattaki ceviriler virgul, tirnak ve satir sonu icerir.
    /// Bunlar bozulursa ceviri verisi sessizce kaybolur.
    #[test]
    fn zor_karakterler_kayipsiz() {
        let zor = serde_json::json!([{
            "Key": "zor",
            "Source": "a,b \"c\" d",
            "Context": "",
            "Values": {
                "tr": "virgul, tirnak \" ve\nsatir sonu",
                "en": " bosluklu "
            }
        }])
        .to_string();

        let csv = json_to_csv(&zor).unwrap();
        let geri = csv_to_json(&csv).unwrap();

        let a: Vec<Entry> = serde_json::from_str(&zor).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&geri).unwrap();
        assert_eq!(a, b, "csv:\n{}", csv);
    }

    #[test]
    fn unicode_korunur() {
        let u = serde_json::json!([{
            "Key": "u",
            "Source": "x",
            "Context": "",
            "Values": { "tr": "Hoşça kal ğüşiöç", "ja": "こんにちは", "emoji": "🎮🚀" }
        }])
        .to_string();

        let csv = json_to_csv(&u).unwrap();
        let geri = csv_to_json(&csv).unwrap();
        let a: Vec<Entry> = serde_json::from_str(&u).unwrap();
        let b: Vec<Entry> = serde_json::from_str(&geri).unwrap();
        assert_eq!(a, b);
    }

    /// Bos hucre "ceviri yok" demektir; bos dize olarak geri gelmemeli,
    /// yoksa olmayan bir ceviri uydurmus oluruz.
    #[test]
    fn bos_hucre_ceviri_uydurmaz() {
        let girdi = serde_json::json!([
            { "Key": "a", "Source": "A", "Context": "", "Values": { "tr": "A-tr" } },
            { "Key": "b", "Source": "B", "Context": "", "Values": { "de": "B-de" } }
        ])
        .to_string();

        let csv = json_to_csv(&girdi).unwrap();
        let geri: Vec<Entry> = serde_json::from_str(&csv_to_json(&csv).unwrap()).unwrap();

        assert_eq!(geri[0].values.len(), 1, "a icin yalnizca tr olmali");
        assert!(!geri[0].values.contains_key("de"));
        assert_eq!(geri[1].values.len(), 1, "b icin yalnizca de olmali");
    }

    #[test]
    fn bos_tablo_cokmez() {
        assert_eq!(json_to_csv("[]").unwrap().trim(), "Key,Source,Context,Example");
        assert_eq!(csv_to_json("Key,Source,Context,Example\n").unwrap(), "[]");
    }

    #[test]
    fn bozuk_girdi_hata_doner_panik_etmez() {
        assert!(json_to_csv("{bozuk").is_err());
        assert!(csv_to_json("A,B").is_err());
    }

    #[test]
    fn crlf_satir_sonu_kabul_edilir() {
        let csv = "Key,Source,Context,Example,tr\r\nselam,Hello,,,Merhaba\r\n";
        let girdiler: Vec<Entry> = serde_json::from_str(&csv_to_json(csv).unwrap()).unwrap();
        assert_eq!(girdiler.len(), 1);
        assert_eq!(girdiler[0].key, "selam");
        assert_eq!(girdiler[0].values.get("tr").unwrap(), "Merhaba");
    }
}
