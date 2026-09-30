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

mod enums;
mod properties;
mod services;

pub(crate) use enums::*;
pub(crate) use properties::*;
pub(crate) use services::*;

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
            crate::values::pv_to_wire(&PropertyValue::Number(2.0)),
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
            crate::values::pv_to_wire(font.as_ref().unwrap()),
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

