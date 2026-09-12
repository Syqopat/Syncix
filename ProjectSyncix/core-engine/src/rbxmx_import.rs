//! Roblox XML (.rbxmx / .rbxlx) import.
//!
//! Export (rbxmx.rs) already existed; import did not. This was the last item Rojo
//! had and Syncix lacked: bringing a ready-made model file into the tree.
//!
//! Scope, honestly: the types our model holds are read (String, Number, Boolean,
//! Vector3, Vector2, Color3/Color3uint8, UDim, UDim2, CFrame, NumberRange, Content,
//! ProtectedString/Source, Font, token — Material/Shape as "Enum.X.Y", any other
//! token as its integer). Unknown property types (BinaryString AttributesSerialize,
//! SharedString, Ref, ...) are SKIPPED and their count is reported — never
//! silently swallowed.

use crate::model::PropertyValue;

/// A single imported instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedNode {
    pub class_name: String,
    pub name: String,
    pub properties: Vec<(String, PropertyValue)>,
    pub source: Option<String>,
    pub children: Vec<ImportedNode>,
}

/// Turns an Enum token number back into text.
/// Only the values export knows; any other token is imported as its integer
/// (see the "token" branch of read_property).
fn token_to_enum(prop: &str, token: i64) -> Option<String> {
    let item_name = match (prop, token) {
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
    let run_name = if prop == "Shape" { "PartType" } else { prop };
    Some(format!("Enum.{}.{}", run_name, item_name))
}

fn sub_text(node_entry: roxmltree::Node, tag_text: &str) -> Option<f64> {
    node_entry
        .children()
        .find(|c| c.has_tag_name(tag_text))
        .and_then(|c| c.text())
        .and_then(|t| t.trim().parse::<f64>().ok())
}

/// Enum.FontWeight's items by the number Roblox XML stores in <Weight>.
const FONT_WEIGHTS: &[(u32, &str)] = &[
    (100, "Thin"),
    (200, "ExtraLight"),
    (300, "Light"),
    (400, "Regular"),
    (500, "Medium"),
    (600, "SemiBold"),
    (700, "Bold"),
    (800, "ExtraBold"),
    (900, "Heavy"),
];

/// FontFace. Studio writes it as
/// `<Font><Family><url>..</url></Family><Weight>400</Weight><Style>Normal</Style></Font>`
/// (plus a CachedFaceId, which Roblox recomputes and we ignore).
///
/// Weight and style become the exact text PatchBuilder produces with tostring()
/// ("Enum.FontWeight.Bold", "Enum.FontStyle.Italic"): PatchExecutor's decodeFont finds
/// the enum by comparing that text, and anything else quietly turned into
/// Regular/Normal on the Studio side. A missing <Weight>/<Style> takes Roblox's own
/// default; a value we cannot name, or a missing family (Font.new refuses it), makes
/// the property count as skipped instead of arriving as a different font.
fn read_font(p: roxmltree::Node) -> Option<PropertyValue> {
    let element_text = |tag: &str| {
        p.children()
            .find(|c| c.has_tag_name(tag))
            .and_then(|c| c.text())
            .map(str::trim)
            .filter(|t| !t.is_empty())
    };

    let family_node = p.children().find(|c| c.has_tag_name("Family"))?;
    let family = family_node
        .children()
        .find(|c| c.has_tag_name("url"))
        .and_then(|c| c.text())
        .or_else(|| family_node.text())
        .unwrap_or("")
        .trim()
        .to_string();
    if family.is_empty() {
        return None;
    }

    let weight = match element_text("Weight") {
        None => "Regular",
        Some(t) => match t.parse::<u32>() {
            Ok(n) => FONT_WEIGHTS.iter().find(|(v, _)| *v == n)?.1,
            // Hand-written or generated files sometimes name the weight instead.
            Err(_) => FONT_WEIGHTS.iter().find(|(_, name)| *name == t)?.1,
        },
    };
    let style = match element_text("Style") {
        None | Some("Normal") | Some("0") => "Normal",
        Some("Italic") | Some("1") => "Italic",
        Some(_) => return None,
    };

    Some(PropertyValue::Font {
        family,
        weight: format!("Enum.FontWeight.{}", weight),
        style: format!("Enum.FontStyle.{}", style),
    })
}

/// Turns a single <Properties> child element into a PropertyValue.
/// None means the type is not supported.
/// The member a property is written under in Roblox XML. Some properties carry their
/// serialized name instead (a Part's Size is written "size", its Color "Color3uint8",
/// its Shape "shape"); stored under those names the plugin could not apply them.
/// None: a name that exists only for serialization and has no member (formFactorRaw).
fn member_name(xml_name: &str) -> Option<&str> {
    match xml_name {
        "size" => Some("Size"),
        "shape" => Some("Shape"),
        "Color3uint8" => Some("Color"),
        "formFactorRaw" | "formFactor" => None,
        other => Some(other),
    }
}

fn read_property(p: roxmltree::Node) -> Option<(String, PropertyValue)> {
    let item_name = member_name(p.attribute("name")?)?.to_string();
    let type_name = p.tag_name().name();
    let text_value = p.text().unwrap_or("").trim().to_string();

    let raw_value = match type_name {
        "string" | "ProtectedString" => PropertyValue::String(text_value),
        "bool" => PropertyValue::Boolean(text_value == "true"),
        "float" | "double" | "int" | "int64" => PropertyValue::Number(text_value.parse().ok()?),
        "token" => {
            let number_value: i64 = text_value.parse().ok()?;
            match token_to_enum(&item_name, number_value) {
                Some(enum_text) => PropertyValue::String(enum_text),
                // Skipping every token the table above does not name threw away most
                // enum settings of a Studio export (TextXAlignment, SurfaceType,
                // SizeConstraint, any Material added after the table was written).
                // The integer alone is enough: pv_to_wire sends a Number as a plain
                // JSON number, PatchExecutor's decodeEnum ignores anything that is not
                // text, so the value lands in `instance[propName] = value`, and Roblox's
                // enum setter accepts an item's integer Value just like the EnumItem or
                // its Name. The echo guard expects the number, not the EnumItem, so
                // Studio then reports the property back as "Enum.X.Y" (PatchBuilder's
                // form) and that replaces the number in the model.
                // Only for real members (PascalCase); an unknown serialization-only token
                // would otherwise land in the files as a property nothing can apply.
                None if item_name.starts_with(|c: char| c.is_ascii_uppercase()) => {
                    PropertyValue::Number(number_value as f64)
                }
                None => return None,
            }
        }
        "Font" => read_font(p)?,
        "Vector3" => {
            let x = sub_text(p, "X")? as f32;
            let y = sub_text(p, "Y")? as f32;
            let z = sub_text(p, "Z")? as f32;
            if item_name == "CFrame" {
                PropertyValue::CFrame {
                    pos: [x, y, z],
                    rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                }
            } else {
                PropertyValue::Vector3 { x, y, z }
            }
        }
        "CoordinateFrame" | "CFrame" => PropertyValue::CFrame {
            pos: [
                sub_text(p, "X").unwrap_or(0.0) as f32,
                sub_text(p, "Y").unwrap_or(0.0) as f32,
                sub_text(p, "Z").unwrap_or(0.0) as f32,
            ],
            rot: [
                sub_text(p, "R00").unwrap_or(1.0) as f32,
                sub_text(p, "R01").unwrap_or(0.0) as f32,
                sub_text(p, "R02").unwrap_or(0.0) as f32,
                sub_text(p, "R10").unwrap_or(0.0) as f32,
                sub_text(p, "R11").unwrap_or(1.0) as f32,
                sub_text(p, "R12").unwrap_or(0.0) as f32,
                sub_text(p, "R20").unwrap_or(0.0) as f32,
                sub_text(p, "R21").unwrap_or(0.0) as f32,
                sub_text(p, "R22").unwrap_or(1.0) as f32,
            ],
        },
        "Color3" => PropertyValue::Color3 {
            r: sub_text(p, "R")? as f32,
            g: sub_text(p, "G")? as f32,
            b: sub_text(p, "B")? as f32,
        },
        "Color3uint8" => {
            let package: u32 = text_value.parse().ok()?;
            PropertyValue::Color3 {
                r: ((package >> 16) & 0xFF) as f32 / 255.0,
                g: ((package >> 8) & 0xFF) as f32 / 255.0,
                b: (package & 0xFF) as f32 / 255.0,
            }
        }
        "UDim2" => PropertyValue::UDim2 {
            xs: sub_text(p, "XS")? as f32,
            xo: sub_text(p, "XO")? as f32,
            ys: sub_text(p, "YS")? as f32,
            yo: sub_text(p, "YO")? as f32,
        },
        "Vector2" => PropertyValue::Vector2 {
            x: sub_text(p, "X")? as f32,
            y: sub_text(p, "Y")? as f32,
        },
        "UDim" => PropertyValue::UDim {
            scale: sub_text(p, "S")? as f32,
            offset: sub_text(p, "O")? as f32,
        },
        "NumberRange" => {
            let parts: Vec<&str> = text_value.split_whitespace().collect();
            if parts.len() >= 2 {
                let min = parts[0].parse().ok()?;
                let max = parts[1].parse().ok()?;
                PropertyValue::NumberRange { min, max }
            } else {
                return None;
            }
        }
        "Content" => {
            let url = p.children().find(|c| c.has_tag_name("url")).and_then(|c| c.text()).unwrap_or("").trim().to_string();
            PropertyValue::Content(url)
        }
        _ => return None,
    };
    Some((item_name, raw_value))
}

fn read_item(item: roxmltree::Node, skipped: &mut usize) -> Option<ImportedNode> {
    let class_name = item.attribute("class")?.to_string();

    let mut name = class_name.clone();
    let mut properties = Vec::new();
    let mut source = None;

    if let Some(props) = item.children().find(|c| c.has_tag_name("Properties")) {
        for p in props.children().filter(|c| c.is_element()) {
            let item_name = p.attribute("name").unwrap_or("");
            if item_name == "Name" {
                name = p.text().unwrap_or("").to_string();
                continue;
            }
            if item_name == "Source" {
                source = Some(p.text().unwrap_or("").to_string());
                continue;
            }
            match read_property(p) {
                Some((k, v)) => properties.push((k, v)),
                None => *skipped += 1,
            }
        }
    }

    let children = item
        .children()
        .filter(|c| c.has_tag_name("Item"))
        .filter_map(|c| read_item(c, skipped))
        .collect();

    Some(ImportedNode {
        class_name,
        name,
        properties,
        source,
        children,
    })
}

/// Turns .rbxmx/.rbxlx text into a list of root nodes.
/// Returns: (roots, number of skipped properties)
pub fn parse_text(xml: &str) -> Result<(Vec<ImportedNode>, usize), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {}", e))?;
    let root_dir = doc.root_element();
    if root_dir.tag_name().name() != "roblox" {
        return Err("not a Roblox XML file (missing <roblox> root)".to_string());
    }

    let mut skipped = 0usize;
    let root_list: Vec<ImportedNode> = root_dir
        .children()
        .filter(|c| c.has_tag_name("Item"))
        .filter_map(|c| read_item(c, &mut skipped))
        .collect();

    Ok((root_list, skipped))
}

/// Total number of nodes in the tree.
pub fn tally(node_list: &[ImportedNode]) -> usize {
    node_list.iter().map(|d| 1 + tally(&d.children)).sum()
}

/// Services a place file carries at its top level. There is exactly one of each in a
/// place and `Instance.new` cannot create them, so an import must merge into the
/// existing one instead of creating a copy.
const SERVICE_CLASSES: &[&str] = &[
    "Workspace",
    "Players",
    "Lighting",
    "MaterialService",
    "ReplicatedFirst",
    "ReplicatedStorage",
    "ServerScriptService",
    "ServerStorage",
    "StarterGui",
    "StarterPack",
    "StarterPlayer",
    "Teams",
    "SoundService",
    "Chat",
    "TextChatService",
    "LocalizationService",
    "TestService",
    "VoiceChatService",
    "ProximityPromptService",
    "HttpService",
    "InsertService",
    "CollectionService",
];

/// Containers that exist once under their parent and cannot be created either.
/// TextChatService's four configurations were missing here: an import tried to create
/// copies, Studio refused, and a UIGradient inside one waited for a parent forever.
const SINGLETON_CHILD_CLASSES: &[&str] = &[
    "StarterPlayerScripts",
    "StarterCharacterScripts",
    "Terrain",
    "BubbleChatConfiguration",
    "ChatWindowConfiguration",
    "ChatInputBarConfiguration",
    "ChannelTabsConfiguration",
];

pub fn is_service(class_name: &str) -> bool {
    SERVICE_CLASSES.contains(&class_name)
}

/// True for every class an import must map onto an existing instance rather than create.
pub fn is_singleton(class_name: &str) -> bool {
    is_service(class_name) || SINGLETON_CHILD_CLASSES.contains(&class_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<roblox version="4">
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
    fn hierarchy_and_names() {
        let (root_list, _) = parse_text(SAMPLE_XML).unwrap();
        assert_eq!(root_list.len(), 1);
        assert_eq!(root_list[0].class_name, "Folder");
        assert_eq!(root_list[0].name, "Group");
        assert_eq!(root_list[0].children.len(), 1);
        assert_eq!(root_list[0].children[0].name, "Box");
        assert_eq!(tally(&root_list), 2);
    }

    #[test]
    fn value_types_are_read_correctly() {
        let (root_list, _) = parse_text(SAMPLE_XML).unwrap();
        let p = &root_list[0].children[0].properties;
        let al = |item_name: &str| p.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone());

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
        // 4294901760 = 0xFFFF0000 -> red
        match al("Color") {
            Some(PropertyValue::Color3 { r, g, b }) => {
                assert!((r - 1.0).abs() < 0.01 && g < 0.01 && b < 0.01);
            }
            other => panic!("the colour could not be read: {:?}", other),
        }
    }

    /// Unknown type_name is NOT silently swallowed, it is counted.
    #[test]
    fn unknown_type_is_counted() {
        let (_, skipped) = parse_text(SAMPLE_XML).unwrap();
        assert_eq!(skipped, 1);
    }

    #[test]
    fn script_source_is_read() {
        let xml = r#"<roblox version="4"><Item class="Script"><Properties>
            <string name="Name">Main</string>
            <ProtectedString name="Source">print("hi")</ProtectedString>
        </Properties></Item></roblox>"#;
        let (k, _) = parse_text(xml).unwrap();
        assert_eq!(k[0].source.as_deref(), Some("print(\"hi\")"));
    }

    #[test]
    fn invalid_input_returns_error() {
        assert!(parse_text("<html></html>").is_err());
        assert!(parse_text("broken").is_err());
    }

    /// Export and import must be each other's inverse.
    #[test]
    fn export_import_round_trip() {
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

        let (xml, _) = crate::rbxmx::export_rbxmx(&m, None);
        let (root_list, _) = parse_text(&xml).unwrap();

        let ws_node = &root_list[0];
        assert_eq!(ws_node.name, "Workspace");
        let boxed = &ws_node.children[0];
        assert_eq!(boxed.name, "Box");
        let al = |item_name: &str| boxed.properties.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone());
        assert_eq!(al("Position"), Some(PropertyValue::Vector3 { x: 5.0, y: 6.0, z: 7.0 }));
        assert_eq!(al("Anchored"), Some(PropertyValue::Boolean(true)));
    }

    #[test]
    fn cframe_and_coordinate_frame_parsed() {
        let xml = r#"<roblox version="4"><Item class="Part"><Properties>
            <string name="Name">TestPart</string>
            <CoordinateFrame name="CFrame">
                <X>10</X><Y>20</Y><Z>30</Z>
                <R00>1</R00><R01>0</R01><R02>0</R02>
                <R10>0</R10><R11>1</R11><R12>0</R12>
                <R20>0</R20><R21>0</R21><R22>1</R22>
            </CoordinateFrame>
            <Vector3 name="OtherVec"><X>1</X><Y>2</Y><Z>3</Z></Vector3>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        assert_eq!(skipped, 0);
        let p = &root_list[0].properties;
        let al = |item_name: &str| p.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone());
        assert_eq!(
            al("CFrame"),
            Some(PropertyValue::CFrame {
                pos: [10.0, 20.0, 30.0],
                rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            })
        );
        assert_eq!(
            al("OtherVec"),
            Some(PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 })
        );
    }

    #[test]
    fn serialized_names_become_member_names() {
        let xml = r#"<roblox version="4"><Item class="Part"><Properties>
            <string name="Name">Box</string>
            <Vector3 name="size"><X>4</X><Y>1</Y><Z>2</Z></Vector3>
            <token name="shape">2</token>
            <Color3uint8 name="Color3uint8">4294901760</Color3uint8>
            <token name="formFactorRaw">1</token>
        </Properties></Item></roblox>"#;
        let (roots, skipped) = parse_text(xml).unwrap();
        let props = &roots[0].properties;
        let get = |n: &str| props.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
        assert_eq!(get("Size"), Some(PropertyValue::Vector3 { x: 4.0, y: 1.0, z: 2.0 }));
        assert_eq!(get("Shape"), Some(PropertyValue::String("Enum.PartType.Cylinder".into())));
        assert!(matches!(get("Color"), Some(PropertyValue::Color3 { .. })));
        assert!(get("size").is_none() && get("formFactorRaw").is_none());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn services_and_singletons_are_recognised() {
        assert!(is_service("Lighting"));
        assert!(is_service("ReplicatedStorage"));
        assert!(!is_service("Folder"));
        assert!(is_singleton("StarterPlayerScripts"));
        assert!(is_singleton("BubbleChatConfiguration"));
        assert!(is_singleton("Workspace"));
        assert!(!is_singleton("Part"));
    }

    /// A token the name table does not know used to be skipped; it must arrive as its
    /// integer, sent as a plain number the plugin assigns straight to the enum property.
    #[test]
    fn unmapped_token_is_kept_as_its_integer() {
        let xml = r#"<roblox version="4"><Item class="Part"><Properties>
            <string name="Name">P</string>
            <token name="Shape">1</token>
            <token name="Material">1712</token>
            <token name="TopSurface">0</token>
        </Properties></Item><Item class="TextLabel"><Properties>
            <token name="TextXAlignment">2</token>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        assert_eq!(skipped, 0);
        let al = |i: usize, item_name: &str| {
            root_list[i].properties.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone())
        };

        // The mapping is kept for the values it names.
        assert_eq!(al(0, "Shape"), Some(PropertyValue::String("Enum.PartType.Block".into())));
        // 1712 (Rubber) is newer than the table: kept as a number, not dropped.
        assert_eq!(al(0, "Material"), Some(PropertyValue::Number(1712.0)));
        assert_eq!(al(0, "TopSurface"), Some(PropertyValue::Number(0.0)));
        assert_eq!(al(1, "TextXAlignment"), Some(PropertyValue::Number(2.0)));
        assert_eq!(
            crate::pv_to_wire(&PropertyValue::Number(2.0)),
            serde_json::json!(2.0)
        );
    }

    /// FontFace as Studio exports it; the result must be the shape PatchBuilder sends
    /// and PatchExecutor's decodeFont matches, or the font silently becomes Regular/Normal.
    #[test]
    fn font_face_is_read_in_the_plugin_format() {
        let xml = r#"<roblox version="4"><Item class="TextLabel"><Properties>
            <string name="Name">Title</string>
            <Font name="FontFace">
                <Family><url>rbxasset://fonts/families/GothamSSm.json</url></Family>
                <Weight>700</Weight>
                <Style>Italic</Style>
                <CachedFaceId><url>rbxasset://fonts/GothamSSm-BoldItalic.otf</url></CachedFaceId>
            </Font>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        assert_eq!(skipped, 0);
        let font = root_list[0]
            .properties
            .iter()
            .find(|(k, _)| k == "FontFace")
            .map(|(_, v)| v.clone());
        assert_eq!(
            font,
            Some(PropertyValue::Font {
                family: "rbxasset://fonts/families/GothamSSm.json".into(),
                weight: "Enum.FontWeight.Bold".into(),
                style: "Enum.FontStyle.Italic".into(),
            })
        );
        assert_eq!(
            crate::pv_to_wire(font.as_ref().unwrap()),
            serde_json::json!({ "Font": {
                "family": "rbxasset://fonts/families/GothamSSm.json",
                "weight": "Enum.FontWeight.Bold",
                "style": "Enum.FontStyle.Italic"
            }})
        );
    }

    #[test]
    fn font_defaults_named_weights_and_rejects() {
        let xml = r#"<roblox version="4"><Item class="TextLabel"><Properties>
            <Font name="Bare"><Family><url>rbxasset://fonts/families/Arial.json</url></Family></Font>
            <Font name="Named"><Family><url>rbxasset://fonts/families/Arial.json</url></Family><Weight>SemiBold</Weight><Style>Normal</Style></Font>
            <Font name="OddWeight"><Family><url>rbxasset://fonts/families/Arial.json</url></Family><Weight>450</Weight></Font>
            <Font name="OddStyle"><Family><url>rbxasset://fonts/families/Arial.json</url></Family><Style>Oblique</Style></Font>
            <Font name="NoFamily"><Weight>400</Weight><Style>Normal</Style></Font>
            <Font name="EmptyFamily"><Family><url></url></Family></Font>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        let p = &root_list[0].properties;
        let al = |item_name: &str| p.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone());
        let arial = |weight: &str, style: &str| {
            Some(PropertyValue::Font {
                family: "rbxasset://fonts/families/Arial.json".into(),
                weight: weight.into(),
                style: style.into(),
            })
        };

        // Missing weight and style take Roblox's defaults.
        assert_eq!(al("Bare"), arial("Enum.FontWeight.Regular", "Enum.FontStyle.Normal"));
        assert_eq!(al("Named"), arial("Enum.FontWeight.SemiBold", "Enum.FontStyle.Normal"));
        // A value we cannot name, or no family, is counted rather than guessed.
        assert_eq!(al("OddWeight"), None);
        assert_eq!(al("OddStyle"), None);
        assert_eq!(al("NoFamily"), None);
        assert_eq!(al("EmptyFamily"), None);
        assert_eq!(skipped, 4);
    }

    /// The new types must not make genuinely unsupported ones disappear from the count.
    #[test]
    fn unsupported_types_are_still_counted() {
        let xml = r#"<roblox version="4"><Item class="Part"><Properties>
            <string name="Name">P</string>
            <BinaryString name="AttributesSerialize"></BinaryString>
            <Ref name="PrimaryPart">null</Ref>
            <token name="Material">notanumber</token>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        assert!(root_list[0].properties.is_empty());
        assert_eq!(skipped, 3);
    }

    #[test]
    fn vector3_named_cframe_converts_to_cframe() {
        let xml = r#"<roblox version="4"><Item class="Part"><Properties>
            <string name="Name">TestPart</string>
            <Vector3 name="CFrame"><X>5</X><Y>15</Y><Z>25</Z></Vector3>
        </Properties></Item></roblox>"#;
        let (root_list, skipped) = parse_text(xml).unwrap();
        assert_eq!(skipped, 0);
        let p = &root_list[0].properties;
        let al = |item_name: &str| p.iter().find(|(k, _)| k == item_name).map(|(_, v)| v.clone());
        assert_eq!(
            al("CFrame"),
            Some(PropertyValue::CFrame {
                pos: [5.0, 15.0, 25.0],
                rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            })
        );
    }
}
