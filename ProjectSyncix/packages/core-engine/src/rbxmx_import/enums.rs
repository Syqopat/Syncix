//! Split out of rbxmx_import.rs.

#[allow(unused_imports)]
use super::*;

/// Turns an Enum token number back into text.
/// Only the values export knows; any other token is imported as its integer
/// (see the "token" branch of read_property).
pub(crate) fn token_to_enum(prop: &str, token: i64) -> Option<String> {
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

pub(crate) fn sub_text(node_entry: roxmltree::Node, tag_text: &str) -> Option<f64> {
    node_entry
        .children()
        .find(|c| c.has_tag_name(tag_text))
        .and_then(|c| c.text())
        .and_then(|t| t.trim().parse::<f64>().ok())
}

/// Enum.FontWeight's items by the number Roblox XML stores in <Weight>.
pub(crate) const FONT_WEIGHTS: &[(u32, &str)] = &[
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
pub(crate) fn read_font(p: roxmltree::Node) -> Option<PropertyValue> {
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
pub(crate) fn member_name(xml_name: &str) -> Option<&str> {
    match xml_name {
        "size" => Some("Size"),
        "shape" => Some("Shape"),
        "Color3uint8" => Some("Color"),
        "formFactorRaw" | "formFactor" => None,
        other => Some(other),
    }
}
