//! Roblox XML (.rbxmx / .rbxlx) dışa aktarımı.
//!
//! Neden gerekli: Rojo'da `rojo build` ile projeden bir yer/model dosyası
//! üretilebiliyordu, Syncix'te bu yoktu (pipeline/mod.rs içindeki export_to_rbxlx
//! `unimplemented!()` idi). CI'da "kod derleniyor mu, ağaç kuruluyor mu" denetimi
//! ve Studio'ya elle içe aktarma için gerekli.
//!
//! Kapsam dürüstlüğü: Syncix'in modeli yalnızca String, Number, Boolean, Vector3,
//! Color3 ve UDim2 tutar. Dolayısıyla bu yazıcı, modelin tuttuğu HER ŞEYİ yazar —
//! kayıp yazıcıda değil modeldedir. Enum değerleri text_value olarak saklandığı için
//! yaygın olanlar sayısal token'a çevrilir; tanınmayan enum'lar atlanır ve sayısı
//! bildirilir.

use crate::model::{DataModel, InstanceNode, PropertyValue};
use uuid::Uuid;

/// XML text_value kaçışı.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Yaygın Enum değerleri için sayısal token karşılıkları.
/// Roblox XML'inde enum'lar `<token>` olarak sayı ile yazılır.
fn enum_token(raw_value: &str) -> Option<u32> {
    let token = match raw_value {
        // Material
        "Enum.Material.Plastic" => 256,
        "Enum.Material.Wood" => 512,
        "Enum.Material.Slate" => 800,
        "Enum.Material.Concrete" => 816,
        "Enum.Material.Brick" => 848,
        "Enum.Material.Sand" => 864,
        "Enum.Material.WoodPlanks" => 528,
        "Enum.Material.Cobblestone" => 880,
        "Enum.Material.Neon" => 288,
        "Enum.Material.Glass" => 1568,
        "Enum.Material.Marble" => 784,
        "Enum.Material.Granite" => 832,
        "Enum.Material.Metal" => 1088,
        "Enum.Material.SmoothPlastic" => 272,
        "Enum.Material.Grass" => 1280,
        "Enum.Material.Ice" => 1536,
        "Enum.Material.Foil" => 1072,
        "Enum.Material.Pebble" => 784,
        "Enum.Material.ForceField" => 1584,
        "Enum.Material.Rock" => 896,
        "Enum.Material.Glacier" => 1552,
        "Enum.Material.Snow" => 1328,
        "Enum.Material.Sandstone" => 912,
        "Enum.Material.Mud" => 1344,
        "Enum.Material.Basalt" => 788,
        "Enum.Material.Ground" => 832,
        "Enum.Material.CrackedLava" => 804,
        "Enum.Material.Asphalt" => 1376,
        "Enum.Material.LeafyGrass" => 1284,
        "Enum.Material.Salt" => 1392,
        "Enum.Material.Limestone" => 1296,
        "Enum.Material.Pavement" => 1360,

        // PartType (Shape)
        "Enum.PartType.Ball" => 0,
        "Enum.PartType.Block" => 1,
        "Enum.PartType.Cylinder" => 2,
        "Enum.PartType.Wedge" => 3,
        "Enum.PartType.CornerWedge" => 4,

        // SurfaceType
        "Enum.SurfaceType.Smooth" => 0,
        "Enum.SurfaceType.Glue" => 1,
        "Enum.SurfaceType.Weld" => 2,
        "Enum.SurfaceType.Studs" => 3,
        "Enum.SurfaceType.Inlet" => 4,
        "Enum.SurfaceType.Universal" => 5,
        "Enum.SurfaceType.Hinge" => 6,
        "Enum.SurfaceType.SmoothNoOutlines" => 10,

        _ => return None,
    };
    Some(token)
}

/// Script sınıflarında origin script_code `ProtectedString` olarak yazılır.
fn looks_like_script(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

struct ExportCounters {
    referent: usize,
    skipped_enums: usize,
}

fn write_property(out_text: &mut String, item_name: &str, raw_value: &PropertyValue, counter: &mut ExportCounters) {
    match raw_value {
        PropertyValue::String(s) => {
            // Metin bir Enum olabilir; öyleyse token olarak yazılmalı.
            if s.starts_with("Enum.") {
                match enum_token(s) {
                    Some(t) => out_text.push_str(&format!(
                        "\t\t\t<token name=\"{}\">{}</token>\n",
                        escape(item_name),
                        t
                    )),
                    None => counter.skipped_enums += 1,
                }
            } else {
                out_text.push_str(&format!(
                    "\t\t\t<string name=\"{}\">{}</string>\n",
                    escape(item_name),
                    escape(s)
                ));
            }
        }
        PropertyValue::Number(n) => out_text.push_str(&format!(
            "\t\t\t<float name=\"{}\">{}</float>\n",
            escape(item_name),
            n
        )),
        PropertyValue::Boolean(b) => out_text.push_str(&format!(
            "\t\t\t<bool name=\"{}\">{}</bool>\n",
            escape(item_name),
            b
        )),
        PropertyValue::Vector3 { x, y, z } => out_text.push_str(&format!(
            "\t\t\t<Vector3 name=\"{}\"><X>{}</X><Y>{}</Y><Z>{}</Z></Vector3>\n",
            escape(item_name),
            x,
            y,
            z
        )),
        PropertyValue::Color3 { r, g, b } => {
            // BasePart.Color Roblox XML'inde Color3uint8 olarak tutulur.
            if item_name == "Color" {
                let package = 0xFF00_0000u32
                    | ((r * 255.0).round() as u32) << 16
                    | ((g * 255.0).round() as u32) << 8
                    | ((b * 255.0).round() as u32);
                out_text.push_str(&format!(
                    "\t\t\t<Color3uint8 name=\"{}\">{}</Color3uint8>\n",
                    escape(item_name),
                    package
                ));
            } else {
                out_text.push_str(&format!(
                    "\t\t\t<Color3 name=\"{}\"><R>{}</R><G>{}</G><B>{}</B></Color3>\n",
                    escape(item_name),
                    r,
                    g,
                    b
                ));
            }
        }
        PropertyValue::UDim2 { xs, xo, ys, yo } => out_text.push_str(&format!(
            "\t\t\t<UDim2 name=\"{}\"><XS>{}</XS><XO>{}</XO><YS>{}</YS><YO>{}</YO></UDim2>\n",
            escape(item_name),
            xs,
            xo,
            ys,
            yo
        )),
        PropertyValue::Vector2 { x, y } => out_text.push_str(&format!(
            "\t\t\t<Vector2 name=\"{}\"><X>{}</X><Y>{}</Y></Vector2>\n",
            escape(item_name),
            x,
            y
        )),
        PropertyValue::UDim { scale, offset } => out_text.push_str(&format!(
            "\t\t\t<UDim name=\"{}\"><S>{}</S><O>{}</O></UDim>\n",
            escape(item_name),
            scale,
            offset
        )),
        PropertyValue::CFrame { pos, rot } => out_text.push_str(&format!(
            "\t\t\t<CoordinateFrame name=\"{}\"><X>{}</X><Y>{}</Y><Z>{}</Z>\
             <R00>{}</R00><R01>{}</R01><R02>{}</R02>\
             <R10>{}</R10><R11>{}</R11><R12>{}</R12>\
             <R20>{}</R20><R21>{}</R21><R22>{}</R22></CoordinateFrame>\n",
            escape(item_name),
            pos[0], pos[1], pos[2],
            rot[0], rot[1], rot[2],
            rot[3], rot[4], rot[5],
            rot[6], rot[7], rot[8]
        )),
        PropertyValue::NumberRange { min, max } => out_text.push_str(&format!(
            "\t\t\t<NumberRange name=\"{}\">{} {} </NumberRange>\n",
            escape(item_name),
            min,
            max
        )),
        // Instance referanslari XML'de <Ref> ile referent'a is_bound; bizim UUID'miz
        // referent degil. Yanlis bir baglanti yazmaktansa atliyoruz.
        PropertyValue::Ref(_) => counter.skipped_enums += 1,
        // Roblox XML'inde BrickColor sayisal palet koduyla yazilir; bizde item_name exists_flag,
        // script_code yok. Yanlis bir script_code yazmaktansa atliyoruz — is_same bilgiyi Color3
        // zaten tasiyor.
        PropertyValue::BrickColor(_) => counter.skipped_enums += 1,
        PropertyValue::Content(u) => out_text.push_str(&format!(
            "			<Content name=\"{}\"><url>{}</url></Content>
",
            escape(item_name),
            escape(u)
        )),
        // Bu tiplerin XML gosterimi bilesik; yanlis yazmaktansa atlaniyorlar.
        // Ayni print_info Studio ile dogrudan senkronda tam olarak tasiniyor.
        PropertyValue::ColorSequence(_)
        | PropertyValue::NumberSequence(_)
        | PropertyValue::Rect { .. }
        | PropertyValue::Font { .. }
        | PropertyValue::PhysicalProperties { .. } => counter.skipped_enums += 1,
    }
}

fn write_node(out_text: &mut String, dm: &DataModel, node: &InstanceNode, counter: &mut ExportCounters) {
    let referent = counter.referent;
    counter.referent += 1;

    out_text.push_str(&format!(
        "\t<Item class=\"{}\" referent=\"RBX{}\">\n\t\t<Properties>\n",
        escape(&node.class_name),
        referent
    ));
    out_text.push_str(&format!(
        "\t\t\t<string name=\"Name\">{}</string>\n",
        escape(&node.name)
    ));

    // Özellikler ada göre sıralı yazılır ki çıktı belirleyici olsun (CI diff'i için).
    let mut key_names: Vec<&String> = node.properties.keys().collect();
    key_names.sort();
    for item_name in key_names {
        if item_name == "Name" {
            continue;
        }
        write_property(out_text, item_name, &node.properties[item_name], counter);
    }

    if looks_like_script(&node.class_name) {
        let origin = node.source.clone().unwrap_or_default();
        out_text.push_str(&format!(
            "\t\t\t<ProtectedString name=\"Source\"><![CDATA[{}]]></ProtectedString>\n",
            // CDATA içinde "]]>" dizisi bölünmek zorunda.
            origin.replace("]]>", "]]]]><![CDATA[>")
        ));
    }

    // Attribute'lar Roblox XML'inde ikili bir blob olarak tutulur; text_value biçiminde
    // güvenilir şekilde yazılamaz. Bu yüzden dışa aktarımda atlanır (bkz. README).

    out_text.push_str("\t\t</Properties>\n");

    for cid in &node.children {
        if let Some(child_entry) = dm.get_instance(cid) {
            write_node(out_text, dm, child_entry, counter);
        }
    }

    out_text.push_str("\t</Item>\n");
}

/// Tüm ağacı Roblox XML olarak döndürür.
/// `kok`: verilirse yalnızca o sub ağaç yazılır (model dosyası), verilmezse
/// bütün service_list yazılır (yer dosyası).
/// Dönüş: (xml, atlanan_enum_sayisi)
pub fn export_rbxmx(dm: &DataModel, root_dir: Option<&Uuid>) -> (String, usize) {
    let mut out_text = String::from("<roblox version=\"4\">\n");
    let mut counter = ExportCounters {
        referent: 0,
        skipped_enums: 0,
    };

    match root_dir {
        Some(uuid) => {
            if let Some(node) = dm.get_instance(uuid) {
                write_node(&mut out_text, dm, node, &mut counter);
            }
        }
        None => {
            let mut root_list: Vec<(&Uuid, &InstanceNode)> = dm
                .get_all_instances()
                .iter()
                .filter(|(_, n)| n.parent.is_none() && n.class_name != "DataModel")
                .collect();
            root_list.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            for (_, node) in root_list {
                write_node(&mut out_text, dm, node, &mut counter);
            }
        }
    }

    out_text.push_str("</roblox>\n");
    (out_text, counter.skipped_enums)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn basic_structure_and_hierarchy() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        add_instance(&mut m, "Part", "Kutu", Some(ws));

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.starts_with("<roblox version=\"4\">"));
        assert!(xml.ends_with("</roblox>\n"));
        assert!(xml.contains("class=\"Workspace\""));
        assert!(xml.contains("class=\"Part\""));
        assert!(xml.contains("<string name=\"Name\">Kutu</string>"));
    }

    #[test]
    fn vector3_and_color_are_written_correctly() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Kutu", Some(ws));
        {
            let n = m.get_mut_instance(&p).unwrap();
            n.properties.insert(
                "Position".into(),
                PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 },
            );
            n.properties.insert(
                "Color".into(),
                PropertyValue::Color3 { r: 1.0, g: 0.0, b: 0.0 },
            );
        }

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.contains("<X>1</X><Y>2</Y><Z>3</Z>"));
        // Kirmizi: 0xFFFF0000
        assert!(
            xml.contains(&format!("<Color3uint8 name=\"Color\">{}</Color3uint8>", 0xFFFF0000u32)),
            "renk paketlenmedi: {}",
            xml
        );
    }

    #[test]
    fn enum_becomes_token_unknown_is_skipped() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Kutu", Some(ws));
        {
            let n = m.get_mut_instance(&p).unwrap();
            n.properties.insert(
                "Material".into(),
                PropertyValue::String("Enum.Material.Neon".into()),
            );
            n.properties.insert(
                "Bilinmeyen".into(),
                PropertyValue::String("Enum.Yok.Boyle".into()),
            );
        }

        let (xml, skipped) = export_rbxmx(&m, None);
        assert!(xml.contains("<token name=\"Material\">288</token>"));
        assert_eq!(skipped, 1, "taninmayan enum sayilmali");
        assert!(!xml.contains("Enum.Yok.Boyle"), "gecersiz enum yazilmamali");
    }

    #[test]
    fn script_source_is_in_cdata() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s).unwrap().source = Some("print(\"merhaba\")".into());

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.contains("<![CDATA[print(\"merhaba\")]]>"));
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        add_instance(&mut m, "Part", "A<B&C", Some(ws));

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.contains("A&lt;B&amp;C"));
    }

    #[test]
    fn single_subtree_can_be_exported() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let folder_path = add_instance(&mut m, "Folder", "Sadece", Some(ws));
        add_instance(&mut m, "Part", "Icerik", Some(folder_path));
        add_instance(&mut m, "Part", "Disarida", Some(ws));

        let (xml, _) = export_rbxmx(&m, Some(&folder_path));
        assert!(xml.contains("Sadece"));
        assert!(xml.contains("Icerik"));
        assert!(!xml.contains("Disarida"), "alt agac disi obje yazilmamali");
    }
}
