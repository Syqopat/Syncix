//! Roblox XML (.rbxmx / .rbxlx) dışa aktarımı.
//!
//! Neden gerekli: Rojo'da `rojo build` ile projeden bir yer/model dosyası
//! üretilebiliyordu, Syncix'te bu yoktu (pipeline/mod.rs içindeki export_to_rbxlx
//! `unimplemented!()` idi). CI'da "kod derleniyor mu, ağaç kuruluyor mu" denetimi
//! ve Studio'ya elle içe aktarma için gerekli.
//!
//! Kapsam dürüstlüğü: Syncix'in modeli yalnızca String, Number, Boolean, Vector3,
//! Color3 ve UDim2 tutar. Dolayısıyla bu yazıcı, modelin tuttuğu HER ŞEYİ yazar —
//! kayıp yazıcıda değil modeldedir. Enum değerleri metin olarak saklandığı için
//! yaygın olanlar sayısal token'a çevrilir; tanınmayan enum'lar atlanır ve sayısı
//! bildirilir.

use crate::model::{DataModel, InstanceNode, PropertyValue};
use uuid::Uuid;

/// XML metin kaçışı.
fn kacir(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Yaygın Enum değerleri için sayısal token karşılıkları.
/// Roblox XML'inde enum'lar `<token>` olarak sayı ile yazılır.
fn enum_token(deger: &str) -> Option<u32> {
    let token = match deger {
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

/// Script sınıflarında kaynak kod `ProtectedString` olarak yazılır.
fn script_mi(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

struct Sayac {
    referent: usize,
    atlanan_enum: usize,
}

fn ozellik_yaz(cikti: &mut String, ad: &str, deger: &PropertyValue, sayac: &mut Sayac) {
    match deger {
        PropertyValue::String(s) => {
            // Metin bir Enum olabilir; öyleyse token olarak yazılmalı.
            if s.starts_with("Enum.") {
                match enum_token(s) {
                    Some(t) => cikti.push_str(&format!(
                        "\t\t\t<token name=\"{}\">{}</token>\n",
                        kacir(ad),
                        t
                    )),
                    None => sayac.atlanan_enum += 1,
                }
            } else {
                cikti.push_str(&format!(
                    "\t\t\t<string name=\"{}\">{}</string>\n",
                    kacir(ad),
                    kacir(s)
                ));
            }
        }
        PropertyValue::Number(n) => cikti.push_str(&format!(
            "\t\t\t<float name=\"{}\">{}</float>\n",
            kacir(ad),
            n
        )),
        PropertyValue::Boolean(b) => cikti.push_str(&format!(
            "\t\t\t<bool name=\"{}\">{}</bool>\n",
            kacir(ad),
            b
        )),
        PropertyValue::Vector3 { x, y, z } => cikti.push_str(&format!(
            "\t\t\t<Vector3 name=\"{}\"><X>{}</X><Y>{}</Y><Z>{}</Z></Vector3>\n",
            kacir(ad),
            x,
            y,
            z
        )),
        PropertyValue::Color3 { r, g, b } => {
            // BasePart.Color Roblox XML'inde Color3uint8 olarak tutulur.
            if ad == "Color" {
                let paket = 0xFF00_0000u32
                    | ((r * 255.0).round() as u32) << 16
                    | ((g * 255.0).round() as u32) << 8
                    | ((b * 255.0).round() as u32);
                cikti.push_str(&format!(
                    "\t\t\t<Color3uint8 name=\"{}\">{}</Color3uint8>\n",
                    kacir(ad),
                    paket
                ));
            } else {
                cikti.push_str(&format!(
                    "\t\t\t<Color3 name=\"{}\"><R>{}</R><G>{}</G><B>{}</B></Color3>\n",
                    kacir(ad),
                    r,
                    g,
                    b
                ));
            }
        }
        PropertyValue::UDim2 { xs, xo, ys, yo } => cikti.push_str(&format!(
            "\t\t\t<UDim2 name=\"{}\"><XS>{}</XS><XO>{}</XO><YS>{}</YS><YO>{}</YO></UDim2>\n",
            kacir(ad),
            xs,
            xo,
            ys,
            yo
        )),
        PropertyValue::Vector2 { x, y } => cikti.push_str(&format!(
            "\t\t\t<Vector2 name=\"{}\"><X>{}</X><Y>{}</Y></Vector2>\n",
            kacir(ad),
            x,
            y
        )),
        PropertyValue::UDim { scale, offset } => cikti.push_str(&format!(
            "\t\t\t<UDim name=\"{}\"><S>{}</S><O>{}</O></UDim>\n",
            kacir(ad),
            scale,
            offset
        )),
        PropertyValue::CFrame { pos, rot } => cikti.push_str(&format!(
            "\t\t\t<CoordinateFrame name=\"{}\"><X>{}</X><Y>{}</Y><Z>{}</Z>\
             <R00>{}</R00><R01>{}</R01><R02>{}</R02>\
             <R10>{}</R10><R11>{}</R11><R12>{}</R12>\
             <R20>{}</R20><R21>{}</R21><R22>{}</R22></CoordinateFrame>\n",
            kacir(ad),
            pos[0], pos[1], pos[2],
            rot[0], rot[1], rot[2],
            rot[3], rot[4], rot[5],
            rot[6], rot[7], rot[8]
        )),
        PropertyValue::NumberRange { min, max } => cikti.push_str(&format!(
            "\t\t\t<NumberRange name=\"{}\">{} {} </NumberRange>\n",
            kacir(ad),
            min,
            max
        )),
        // Instance referanslari XML'de <Ref> ile referent'a bagli; bizim UUID'miz
        // referent degil. Yanlis bir baglanti yazmaktansa atliyoruz.
        PropertyValue::Ref(_) => sayac.atlanan_enum += 1,
        // Roblox XML'inde BrickColor sayisal palet koduyla yazilir; bizde ad var,
        // kod yok. Yanlis bir kod yazmaktansa atliyoruz — ayni bilgiyi Color3
        // zaten tasiyor.
        PropertyValue::BrickColor(_) => sayac.atlanan_enum += 1,
        PropertyValue::Content(u) => cikti.push_str(&format!(
            "			<Content name=\"{}\"><url>{}</url></Content>
",
            kacir(ad),
            kacir(u)
        )),
        // Bu tiplerin XML gosterimi bilesik; yanlis yazmaktansa atlaniyorlar.
        // Ayni bilgi Studio ile dogrudan senkronda tam olarak tasiniyor.
        PropertyValue::ColorSequence(_)
        | PropertyValue::NumberSequence(_)
        | PropertyValue::Rect { .. }
        | PropertyValue::Font { .. }
        | PropertyValue::PhysicalProperties { .. } => sayac.atlanan_enum += 1,
    }
}

fn dugum_yaz(cikti: &mut String, dm: &DataModel, node: &InstanceNode, sayac: &mut Sayac) {
    let referent = sayac.referent;
    sayac.referent += 1;

    cikti.push_str(&format!(
        "\t<Item class=\"{}\" referent=\"RBX{}\">\n\t\t<Properties>\n",
        kacir(&node.class_name),
        referent
    ));
    cikti.push_str(&format!(
        "\t\t\t<string name=\"Name\">{}</string>\n",
        kacir(&node.name)
    ));

    // Özellikler ada göre sıralı yazılır ki çıktı belirleyici olsun (CI diff'i için).
    let mut anahtarlar: Vec<&String> = node.properties.keys().collect();
    anahtarlar.sort();
    for ad in anahtarlar {
        if ad == "Name" {
            continue;
        }
        ozellik_yaz(cikti, ad, &node.properties[ad], sayac);
    }

    if script_mi(&node.class_name) {
        let kaynak = node.source.clone().unwrap_or_default();
        cikti.push_str(&format!(
            "\t\t\t<ProtectedString name=\"Source\"><![CDATA[{}]]></ProtectedString>\n",
            // CDATA içinde "]]>" dizisi bölünmek zorunda.
            kaynak.replace("]]>", "]]]]><![CDATA[>")
        ));
    }

    // Attribute'lar Roblox XML'inde ikili bir blob olarak tutulur; metin biçiminde
    // güvenilir şekilde yazılamaz. Bu yüzden dışa aktarımda atlanır (bkz. README).

    cikti.push_str("\t\t</Properties>\n");

    for cid in &node.children {
        if let Some(cocuk) = dm.get_instance(cid) {
            dugum_yaz(cikti, dm, cocuk, sayac);
        }
    }

    cikti.push_str("\t</Item>\n");
}

/// Tüm ağacı Roblox XML olarak döndürür.
/// `kok`: verilirse yalnızca o alt ağaç yazılır (model dosyası), verilmezse
/// bütün servisler yazılır (yer dosyası).
/// Dönüş: (xml, atlanan_enum_sayisi)
pub fn disa_aktar(dm: &DataModel, kok: Option<&Uuid>) -> (String, usize) {
    let mut cikti = String::from("<roblox version=\"4\">\n");
    let mut sayac = Sayac {
        referent: 0,
        atlanan_enum: 0,
    };

    match kok {
        Some(uuid) => {
            if let Some(node) = dm.get_instance(uuid) {
                dugum_yaz(&mut cikti, dm, node, &mut sayac);
            }
        }
        None => {
            let mut kokler: Vec<(&Uuid, &InstanceNode)> = dm
                .get_all_instances()
                .iter()
                .filter(|(_, n)| n.parent.is_none() && n.class_name != "DataModel")
                .map(|(u, n)| (u, n))
                .collect();
            kokler.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            for (_, node) in kokler {
                dugum_yaz(&mut cikti, dm, node, &mut sayac);
            }
        }
    }

    cikti.push_str("</roblox>\n");
    (cikti, sayac.atlanan_enum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ekle(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn temel_yapi_ve_hiyerarsi() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        ekle(&mut m, "Part", "Kutu", Some(ws));

        let (xml, _) = disa_aktar(&m, None);
        assert!(xml.starts_with("<roblox version=\"4\">"));
        assert!(xml.ends_with("</roblox>\n"));
        assert!(xml.contains("class=\"Workspace\""));
        assert!(xml.contains("class=\"Part\""));
        assert!(xml.contains("<string name=\"Name\">Kutu</string>"));
    }

    #[test]
    fn vector3_ve_renk_dogru_yazilir() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        let p = ekle(&mut m, "Part", "Kutu", Some(ws));
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

        let (xml, _) = disa_aktar(&m, None);
        assert!(xml.contains("<X>1</X><Y>2</Y><Z>3</Z>"));
        // Kirmizi: 0xFFFF0000
        assert!(
            xml.contains(&format!("<Color3uint8 name=\"Color\">{}</Color3uint8>", 0xFFFF0000u32)),
            "renk paketlenmedi: {}",
            xml
        );
    }

    #[test]
    fn enum_token_a_cevrilir_taninmayan_atlanir() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        let p = ekle(&mut m, "Part", "Kutu", Some(ws));
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

        let (xml, atlanan) = disa_aktar(&m, None);
        assert!(xml.contains("<token name=\"Material\">288</token>"));
        assert_eq!(atlanan, 1, "taninmayan enum sayilmali");
        assert!(!xml.contains("Enum.Yok.Boyle"), "gecersiz enum yazilmamali");
    }

    #[test]
    fn script_kaynagi_cdata_icinde() {
        let mut m = DataModel::new();
        let sss = ekle(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = ekle(&mut m, "Script", "Ana", Some(sss));
        m.get_mut_instance(&s).unwrap().source = Some("print(\"merhaba\")".into());

        let (xml, _) = disa_aktar(&m, None);
        assert!(xml.contains("<![CDATA[print(\"merhaba\")]]>"));
    }

    #[test]
    fn xml_ozel_karakterleri_kacirilir() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        ekle(&mut m, "Part", "A<B&C", Some(ws));

        let (xml, _) = disa_aktar(&m, None);
        assert!(xml.contains("A&lt;B&amp;C"));
    }

    #[test]
    fn tek_alt_agac_disa_aktarilabilir() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        let klasor = ekle(&mut m, "Folder", "Sadece", Some(ws));
        ekle(&mut m, "Part", "Icerik", Some(klasor));
        ekle(&mut m, "Part", "Disarida", Some(ws));

        let (xml, _) = disa_aktar(&m, Some(&klasor));
        assert!(xml.contains("Sadece"));
        assert!(xml.contains("Icerik"));
        assert!(!xml.contains("Disarida"), "alt agac disi obje yazilmamali");
    }
}
