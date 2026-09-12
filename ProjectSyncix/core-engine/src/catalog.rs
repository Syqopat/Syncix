//! Roblox's classes and the properties Syncix carries, read from the table generated for
//! the Studio plugin (tools/gen-properties.py -> PropertyTable.lua) and embedded at build
//! time. The CLI uses it to say "Did you mean Part?" for `syncix new Prat` and "Did you
//! mean Size?" for `syncix set Box Szie 4,1,2` without asking Studio.
//!
//! The table lists only properties of types Syncix can carry, and a class newer than the
//! table is missing from it. So a name it does not know is not proof of a mistake: callers
//! refuse only a name that is close to a known one, and let anything else through.

use std::collections::HashMap;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../../studio-plugin/src/Observer/PropertyTable.lua");

struct ClassEntry {
    superclass: Option<&'static str>,
    /// Property name and its type code: p primitive, d data type, e enum, r reference.
    properties: Vec<(&'static str, char)>,
}

fn classes() -> &'static HashMap<&'static str, ClassEntry> {
    static CLASSES: OnceLock<HashMap<&'static str, ClassEntry>> = OnceLock::new();
    CLASSES.get_or_init(|| parse(TABLE))
}

/// One class per line: `["Part"]={u="FormFactorPart",p={["Shape"]="e",["Size"]="d"}},`
fn parse(text: &'static str) -> HashMap<&'static str, ClassEntry> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("[\"") else { continue };
        let Some((name, rest)) = rest.split_once("\"]=") else { continue };
        let (head, props) = rest.split_once("p={").unwrap_or((rest, ""));
        let superclass = head.split_once("u=\"").and_then(|(_, r)| r.split_once('"')).map(|(s, _)| s);
        let properties = props
            .split("[\"")
            .skip(1)
            .filter_map(|p| {
                let (prop, kind) = p.split_once("\"]=\"")?;
                Some((prop, kind.chars().next()?))
            })
            .collect();
        out.insert(name, ClassEntry { superclass, properties });
    }
    out
}

pub fn is_class(name: &str) -> bool {
    classes().contains_key(name)
}

pub fn class_names() -> impl Iterator<Item = &'static str> {
    classes().keys().copied()
}

/// The table's spelling of a class typed in another case ("part" -> "Part").
/// `Instance.new` is case-sensitive, so "part" would fail in Studio.
pub fn class_named(typed: &str) -> Option<&'static str> {
    class_names().find(|name| name.eq_ignore_ascii_case(typed))
}

/// The class and its superclasses, nearest first; empty for a class the table does not know.
fn lineage(class: &str) -> Vec<&'static ClassEntry> {
    let mut out = Vec::new();
    let mut current = classes().get(class);
    while let Some(entry) = current {
        out.push(entry);
        if out.len() > 64 {
            break;
        }
        current = entry.superclass.and_then(|s| classes().get(s));
    }
    out
}

/// Every property Syncix knows on the class, inherited ones and Name included.
/// None for a class the table does not know.
pub fn properties_of(class: &str) -> Option<Vec<&'static str>> {
    let lineage = lineage(class);
    if lineage.is_empty() {
        return None;
    }
    let mut out = vec!["Name"];
    for entry in lineage {
        out.extend(entry.properties.iter().map(|(name, _)| *name));
    }
    Some(out)
}

/// Whether the table says the property holds an enum. An enum can come back from an
/// import as its bare number, so a number in the model says nothing about what may be typed.
pub fn is_enum_property(class: &str, property: &str) -> bool {
    lineage(class)
        .iter()
        .flat_map(|entry| entry.properties.iter())
        .any(|(name, kind)| name.eq_ignore_ascii_case(property) && *kind == 'e')
}

/// Items of the enums a place is usually built with, complete for each enum listed, so a
/// slip in one is caught before Studio refuses it. The table above holds no enum items; an
/// enum missing here is passed on as typed.
const ENUM_ITEMS: &[(&str, &[&str])] = &[
    (
        "Material",
        &[
            "Plastic", "SmoothPlastic", "Neon", "Wood", "WoodPlanks", "Marble", "Basalt", "Slate",
            "CrackedLava", "Concrete", "Limestone", "Granite", "Pavement", "Brick", "Pebble",
            "Cobblestone", "Rock", "Sandstone", "CorrodedMetal", "DiamondPlate", "Foil", "Metal",
            "Grass", "LeafyGrass", "Sand", "Fabric", "Snow", "Mud", "Ground", "Asphalt", "Salt",
            "Ice", "Glacier", "Glass", "ForceField", "Air", "Water", "Cardboard", "Carpet",
            "CeramicTiles", "ClayRoofTiles", "RoofShingles", "Leather", "Plaster", "Rubber",
        ],
    ),
    ("PartType", &["Ball", "Block", "Cylinder", "Wedge", "CornerWedge"]),
    (
        "SurfaceType",
        &["Smooth", "Glue", "Weld", "Studs", "Inlet", "Universal", "Hinge", "Motor", "SteppingMotor", "SmoothNoOutlines"],
    ),
    ("NormalId", &["Top", "Bottom", "Front", "Back", "Right", "Left"]),
    ("TextXAlignment", &["Left", "Right", "Center"]),
    ("TextYAlignment", &["Top", "Center", "Bottom"]),
    ("SizeConstraint", &["RelativeXY", "RelativeXX", "RelativeYY"]),
    ("AutomaticSize", &["None", "X", "Y", "XY"]),
    ("ScaleType", &["Stretch", "Slice", "Tile", "Fit", "Crop"]),
    ("BorderMode", &["Outline", "Middle", "Inset"]),
    ("ZIndexBehavior", &["Global", "Sibling"]),
    ("HumanoidRigType", &["R6", "R15"]),
];

pub fn enum_items(enum_name: &str) -> Option<&'static [&'static str]> {
    ENUM_ITEMS.iter().find(|(name, _)| *name == enum_name).map(|(_, items)| *items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_table_is_read() {
        assert!(classes().len() > 500, "only {} classes", classes().len());
        assert!(is_class("Part"));
        assert_eq!(class_named("part"), Some("Part"));
        let part = properties_of("Part").unwrap();
        for wanted in ["Name", "Size", "Anchored", "Color", "Material", "Transparency"] {
            assert!(part.contains(&wanted), "Part lacks {}", wanted);
        }
        assert!(properties_of("NoSuchClass").is_none());
        assert!(is_enum_property("Part", "Material"));
        assert!(!is_enum_property("Part", "Size"));
    }

    #[test]
    fn a_misspelt_class_finds_the_real_one() {
        assert!(crate::suggest::closest("Prat", class_names()).contains(&"Part"));
        assert!(crate::suggest::closest("ModuelScript", class_names()).contains(&"ModuleScript"));
    }
}
