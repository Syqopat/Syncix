//! Split out of rbxmx_import.rs.

#[allow(unused_imports)]
use super::*;

pub(crate) fn read_property(p: roxmltree::Node) -> Option<(String, PropertyValue)> {
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
