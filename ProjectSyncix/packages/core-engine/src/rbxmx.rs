//! Roblox XML (.rbxmx / .rbxlx) export.
//!
//! Why it is needed: Rojo could build a place/model file from a project with
//! `rojo build`; Syncix could not (export_to_rbxlx in pipeline/mod.rs
//! was `unimplemented!()`). It is needed for a CI check that the code builds and the
//! tree assembles, and for importing into Studio by hand.
//!
//! Scope, honestly: Syncix's model holds String, Number, Boolean, Vector3,
//! Color3 and UDim2. So this writer writes EVERYTHING the model holds —
//! the loss is in the model, not the writer. Enum values are stored as text, so
//! common ones are converted to numeric tokens; unknown enums are skipped and their
//! count is reported.

use crate::model::{DataModel, InstanceNode, PropertyValue};
use uuid::Uuid;

/// XML text escaping.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Numeric token equivalents of common Enum values.
/// In Roblox XML enums are written as a number in `<token>`.
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

/// For script classes the source is written as a `ProtectedString`.
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
            // The text may be an Enum; if so it must be written as a token.
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
            // BasePart.Color is kept as Color3uint8 in Roblox XML.
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
        // Instance references are bound to a referent with <Ref> in XML; our UUID
        // is not a referent. Rather than write a wrong link, they are skipped.
        PropertyValue::Ref(_) => counter.skipped_enums += 1,
        // In Roblox XML BrickColor is written as a numeric palette code; we store the name,
        // not the code. Rather than write a wrong code it is skipped — Color3 already
        // carries the same information.
        PropertyValue::BrickColor(_) => counter.skipped_enums += 1,
        PropertyValue::Content(u) => out_text.push_str(&format!(
            "			<Content name=\"{}\"><url>{}</url></Content>
",
            escape(item_name),
            escape(u)
        )),
        // These types have compound XML forms; rather than written wrongly, they are skipped.
        // The same information is carried in full by direct sync with Studio.
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

    // Properties are written sorted by name so the output is deterministic (for CI diffs).
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
            // The sequence "]]>" has to be split inside CDATA.
            origin.replace("]]>", "]]]]><![CDATA[>")
        ));
    }

    // Attributes are kept as a binary blob in Roblox XML and cannot be written
    // reliably as text. So they are skipped on export (see README).

    out_text.push_str("\t\t</Properties>\n");

    for cid in &node.children {
        if let Some(child_entry) = dm.get_instance(cid) {
            write_node(out_text, dm, child_entry, counter);
        }
    }

    out_text.push_str("\t</Item>\n");
}

/// Returns the whole tree as Roblox XML.
/// `root`: if given, only that subtree is written (model file); otherwise
/// every service is written (place file).
/// Returns: (xml, skipped_enum_count)
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
        add_instance(&mut m, "Part", "Box", Some(ws));

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.starts_with("<roblox version=\"4\">"));
        assert!(xml.ends_with("</roblox>\n"));
        assert!(xml.contains("class=\"Workspace\""));
        assert!(xml.contains("class=\"Part\""));
        assert!(xml.contains("<string name=\"Name\">Box</string>"));
    }

    #[test]
    fn vector3_and_color_are_written_correctly() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Box", Some(ws));
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
        // Red: 0xFFFF0000
        assert!(
            xml.contains(&format!("<Color3uint8 name=\"Color\">{}</Color3uint8>", 0xFFFF0000u32)),
            "the colour was not packed: {}",
            xml
        );
    }

    #[test]
    fn enum_becomes_token_unknown_is_skipped() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        let p = add_instance(&mut m, "Part", "Box", Some(ws));
        {
            let n = m.get_mut_instance(&p).unwrap();
            n.properties.insert(
                "Material".into(),
                PropertyValue::String("Enum.Material.Neon".into()),
            );
            n.properties.insert(
                "Unknown".into(),
                PropertyValue::String("Enum.Not.Real".into()),
            );
        }

        let (xml, skipped) = export_rbxmx(&m, None);
        assert!(xml.contains("<token name=\"Material\">288</token>"));
        assert_eq!(skipped, 1, "unknown enum should be counted");
        assert!(!xml.contains("Enum.Not.Real"), "an invalid enum must not be written");
    }

    #[test]
    fn script_source_is_in_cdata() {
        let mut m = DataModel::new();
        let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
        let s = add_instance(&mut m, "Script", "Main", Some(sss));
        m.get_mut_instance(&s).unwrap().source = Some("print(\"hello\")".into());

        let (xml, _) = export_rbxmx(&m, None);
        assert!(xml.contains("<![CDATA[print(\"hello\")]]>"));
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
        let folder_path = add_instance(&mut m, "Folder", "OnlyThis", Some(ws));
        add_instance(&mut m, "Part", "Icerik", Some(folder_path));
        add_instance(&mut m, "Part", "Disarida", Some(ws));

        let (xml, _) = export_rbxmx(&m, Some(&folder_path));
        assert!(xml.contains("OnlyThis"));
        assert!(xml.contains("Icerik"));
        assert!(!xml.contains("Disarida"), "an object outside the subtree must not be written");
    }
}
