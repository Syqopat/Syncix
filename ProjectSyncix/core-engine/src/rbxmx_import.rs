//! Roblox XML (.rbxmx / .rbxlx) içe aktarma.
//!
//! Dışa aktarma (rbxmx.rs) zaten vardı; içe aktarma yoktu. Bu, Rojo'da olup bizde
//! olmayan son maddeydi: hazır bir model dosyasını ağaca alabilmek.
//!
//! Kapsam dürüstlüğü: modelimizin tuttuğu tipler okunur (String, Number, Boolean,
//! Vector3, Color3, UDim2, ProtectedString/Source, token). Tanınmayan property
//! tipleri ATLANIR ve sayısı bildirilir — sessizce yutulmaz.

use crate::model::PropertyValue;

/// İçe aktarılan tek bir instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedNode {
    pub class_name: String,
    pub name: String,
    pub properties: Vec<(String, PropertyValue)>,
    pub source: Option<String>,
    pub children: Vec<ImportedNode>,
}

/// Enum token sayısını geri metne çevirir.
/// Yalnızca dışa aktarımda tanıdığımız değerler; gerisi atlanır.
fn token_to_enum(prop: &str, token: i64) -> Option<String> {
    let ad = match (prop, token) {
        ("Material", 256) => "Plastic",
        ("Material", 272) => "SmoothPlastic",
        ("Material", 288) => "Neon",
        ("Material", 512) => "Wood",
        ("Material", 528) => "WoodPlanks",
        ("Material", 784) => "Marble",
        ("Material", 800) => "Slate",
        ("Material", 816) => "Concrete",
        ("Material", 832) => "Granite",
        ("Material", 848) => "Brick",
        ("Material", 864) => "Sand",
        ("Material", 880) => "Cobblestone",
        ("Material", 896) => "Rock",
        ("Material", 1072) => "Foil",
        ("Material", 1088) => "Metal",
        ("Material", 1280) => "Grass",
        ("Material", 1284) => "LeafyGrass",
        ("Material", 1296) => "Limestone",
        ("Material", 1328) => "Snow",
        ("Material", 1344) => "Mud",
        ("Material", 1360) => "Pavement",
        ("Material", 1376) => "Asphalt",
        ("Material", 1392) => "Salt",
        ("Material", 1536) => "Ice",
        ("Material", 1552) => "Glacier",
        ("Material", 1568) => "Glass",
        ("Material", 1584) => "ForceField",
        ("Shape", 0) => "Ball",
        ("Shape", 1) => "Block",
        ("Shape", 2) => "Cylinder",
        ("Shape", 3) => "Wedge",
        ("Shape", 4) => "CornerWedge",
        _ => return None,
    };
    let tur = if prop == "Shape" { "PartType" } else { prop };
    Some(format!("Enum.{}.{}", tur, ad))
}

fn alt_metin(dugum: roxmltree::Node, etiket: &str) -> Option<f64> {
    dugum
        .children()
        .find(|c| c.has_tag_name(etiket))
        .and_then(|c| c.text())
        .and_then(|t| t.trim().parse::<f64>().ok())
}

/// Tek bir <Properties> alt öğesini PropertyValue'ya çevirir.
/// Dönüş None ise tip desteklenmiyor demektir.
fn property_oku(p: roxmltree::Node) -> Option<(String, PropertyValue)> {
    let ad = p.attribute("name")?.to_string();
    let tip = p.tag_name().name();
    let metin = p.text().unwrap_or("").trim().to_string();

    let deger = match tip {
        "string" | "ProtectedString" => PropertyValue::String(metin),
        "bool" => PropertyValue::Boolean(metin == "true"),
        "float" | "double" | "int" | "int64" => PropertyValue::Number(metin.parse().ok()?),
        "token" => {
            let sayi: i64 = metin.parse().ok()?;
            PropertyValue::String(token_to_enum(&ad, sayi)?)
        }
        "Vector3" => PropertyValue::Vector3 {
            x: alt_metin(p, "X")? as f32,
            y: alt_metin(p, "Y")? as f32,
            z: alt_metin(p, "Z")? as f32,
        },
        "Color3" => PropertyValue::Color3 {
            r: alt_metin(p, "R")? as f32,
            g: alt_metin(p, "G")? as f32,
            b: alt_metin(p, "B")? as f32,
        },
        "Color3uint8" => {
            // Tek bir sayıya paketlenmiş ARGB.
            let paket: u32 = metin.parse().ok()?;
            PropertyValue::Color3 {
                r: ((paket >> 16) & 0xFF) as f32 / 255.0,
                g: ((paket >> 8) & 0xFF) as f32 / 255.0,
                b: (paket & 0xFF) as f32 / 255.0,
            }
        }
        "UDim2" => PropertyValue::UDim2 {
            xs: alt_metin(p, "XS")? as f32,
            xo: alt_metin(p, "XO")? as f32,
            ys: alt_metin(p, "YS")? as f32,
            yo: alt_metin(p, "YO")? as f32,
        },
        _ => return None,
    };
    Some((ad, deger))
}

fn item_oku(item: roxmltree::Node, atlanan: &mut usize) -> Option<ImportedNode> {
    let class_name = item.attribute("class")?.to_string();

    let mut name = class_name.clone();
    let mut properties = Vec::new();
    let mut source = None;

    if let Some(props) = item.children().find(|c| c.has_tag_name("Properties")) {
        for p in props.children().filter(|c| c.is_element()) {
            let ad = p.attribute("name").unwrap_or("");
            if ad == "Name" {
                name = p.text().unwrap_or("").to_string();
                continue;
            }
            if ad == "Source" {
                source = Some(p.text().unwrap_or("").to_string());
                continue;
            }
            match property_oku(p) {
                Some((k, v)) => properties.push((k, v)),
                None => *atlanan += 1,
            }
        }
    }

    let children = item
        .children()
        .filter(|c| c.has_tag_name("Item"))
        .filter_map(|c| item_oku(c, atlanan))
        .collect();

    Some(ImportedNode {
        class_name,
        name,
        properties,
        source,
        children,
    })
}

/// Bir .rbxmx/.rbxlx metnini kök düğüm listesine çevirir.
/// Dönüş: (kökler, atlanan property sayısı)
pub fn ayristir(xml: &str) -> Result<(Vec<ImportedNode>, usize), String> {
    let belge = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {}", e))?;
    let kok = belge.root_element();
    if kok.tag_name().name() != "roblox" {
        return Err("not a Roblox XML file (missing <roblox> root)".to_string());
    }

    let mut atlanan = 0usize;
    let kokler: Vec<ImportedNode> = kok
        .children()
        .filter(|c| c.has_tag_name("Item"))
        .filter_map(|c| item_oku(c, &mut atlanan))
        .collect();

    Ok((kokler, atlanan))
}

/// Ağaçtaki toplam düğüm sayısı.
pub fn say(dugumler: &[ImportedNode]) -> usize {
    dugumler.iter().map(|d| 1 + say(&d.children)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORNEK: &str = r#"<roblox version="4">
	<Item class="Folder" referent="RBX0">
		<Properties>
			<string name="Name">Group</string>
		</Properties>
	<Item class="Part" referent="RBX1">
		<Properties>
			<string name="Name">Box</string>
			<bool name="Anchored">true</bool>
			<Color3uint8 name="Color">4294901760</Color3uint8>
			<token name="Material">288</token>
			<Vector3 name="Position"><X>1</X><Y>2</Y><Z>3</Z></Vector3>
			<float name="Transparency">0.5</float>
			<SomeUnknownType name="Weird">x</SomeUnknownType>
		</Properties>
	</Item>
	</Item>
</roblox>"#;

    #[test]
    fn hiyerarsi_ve_isimler() {
        let (kokler, _) = ayristir(ORNEK).unwrap();
        assert_eq!(kokler.len(), 1);
        assert_eq!(kokler[0].class_name, "Folder");
        assert_eq!(kokler[0].name, "Group");
        assert_eq!(kokler[0].children.len(), 1);
        assert_eq!(kokler[0].children[0].name, "Box");
        assert_eq!(say(&kokler), 2);
    }

    #[test]
    fn deger_tipleri_dogru_okunur() {
        let (kokler, _) = ayristir(ORNEK).unwrap();
        let p = &kokler[0].children[0].properties;
        let al = |ad: &str| p.iter().find(|(k, _)| k == ad).map(|(_, v)| v.clone());

        assert_eq!(al("Anchored"), Some(PropertyValue::Boolean(true)));
        assert_eq!(al("Transparency"), Some(PropertyValue::Number(0.5)));
        assert_eq!(
            al("Position"),
            Some(PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 })
        );
        assert_eq!(
            al("Material"),
            Some(PropertyValue::String("Enum.Material.Neon".into()))
        );
        // 4294901760 = 0xFFFF0000 -> kirmizi
        match al("Color") {
            Some(PropertyValue::Color3 { r, g, b }) => {
                assert!((r - 1.0).abs() < 0.01 && g < 0.01 && b < 0.01);
            }
            other => panic!("renk okunamadi: {:?}", other),
        }
    }

    /// Taninmayan tip SESSIZCE yutulmaz, sayilir.
    #[test]
    fn taninmayan_tip_sayilir() {
        let (_, atlanan) = ayristir(ORNEK).unwrap();
        assert_eq!(atlanan, 1);
    }

    #[test]
    fn script_kaynagi_okunur() {
        let xml = r#"<roblox version="4"><Item class="Script"><Properties>
            <string name="Name">Main</string>
            <ProtectedString name="Source">print("hi")</ProtectedString>
        </Properties></Item></roblox>"#;
        let (k, _) = ayristir(xml).unwrap();
        assert_eq!(k[0].source.as_deref(), Some("print(\"hi\")"));
    }

    #[test]
    fn gecersiz_girdi_hata_doner() {
        assert!(ayristir("<html></html>").is_err());
        assert!(ayristir("bozuk").is_err());
    }

    /// Disa aktarim ile ice aktarim birbirinin tersi olmali.
    #[test]
    fn disa_ice_gidis_donus() {
        use crate::model::{DataModel, InstanceNode};
        let mut m = DataModel::new();
        let mut ws = InstanceNode::new("Workspace", "Workspace");
        let ws_id = ws.syncix_id;
        ws.parent = None;
        m.upsert_instance(ws).unwrap();

        let mut part = InstanceNode::new("Part", "Box");
        part.parent = Some(ws_id);
        part.properties.insert(
            "Position".into(),
            PropertyValue::Vector3 { x: 5.0, y: 6.0, z: 7.0 },
        );
        part.properties
            .insert("Anchored".into(), PropertyValue::Boolean(true));
        m.upsert_instance(part).unwrap();

        let (xml, _) = crate::rbxmx::disa_aktar(&m, None);
        let (kokler, _) = ayristir(&xml).unwrap();

        let ws_dugum = &kokler[0];
        assert_eq!(ws_dugum.name, "Workspace");
        let kutu = &ws_dugum.children[0];
        assert_eq!(kutu.name, "Box");
        let al = |ad: &str| kutu.properties.iter().find(|(k, _)| k == ad).map(|(_, v)| v.clone());
        assert_eq!(
            al("Position"),
            Some(PropertyValue::Vector3 { x: 5.0, y: 6.0, z: 7.0 })
        );
        assert_eq!(al("Anchored"), Some(PropertyValue::Boolean(true)));
    }
}
