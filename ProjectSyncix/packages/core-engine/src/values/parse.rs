//! Turning typed text into a property value of the type the property already has.

use crate::model::PropertyValue;
use crate::{catalog, model, suggest};

#[allow(unused_imports)]
use super::*;

/// Turns a string value from the CLI into the matching PropertyValue.
/// "true"/"false" -> Boolean, "x,y,z" -> Vector3, number -> Number, otherwise -> String.
pub(crate) fn parse_property_value(s: &str) -> model::PropertyValue {
    use model::PropertyValue;
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        return PropertyValue::Boolean(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return PropertyValue::Boolean(false);
    }

    // Hex colour: "#ff8800" -> Color3
    // (It used to reach Studio as plain text and be rejected.)
    // Only the six-digit form here: with no colour target to judge by, a short "#abc" is
    // as likely a tag in a text as a colour. parse_color3 takes "#f80" for colour targets.
    if let Some(hex) = t.strip_prefix('#').filter(|h| h.len() == 6) {
        if let Some([r, g, b]) = parse_hex_color(hex) {
            return PropertyValue::Color3 { r, g, b };
        }
    }
    let parts: Vec<&str> = t.split(',').collect();
    if parts.len() == 3 {
        if let (Ok(x), Ok(y), Ok(z)) = (
            parts[0].trim().parse::<f32>(),
            parts[1].trim().parse::<f32>(),
            parts[2].trim().parse::<f32>(),
        ) {
            return PropertyValue::Vector3 { x, y, z };
        }
    }
    if let Ok(n) = t.parse::<f64>() {
        return PropertyValue::Number(n);
    }
    PropertyValue::String(t.to_string())
}

/// Fits text typed in the terminal to the type of the property's CURRENT value in the
/// model. If it cannot, returns None and the caller falls back to the general parser.
///
/// Why it exists: `syncix set <part> BrickColor "Really red"` produced plain text and
/// the assignment silently failed in Studio. The same problem existed for CFrame, UDim2,
/// NumberRange and similar types — they could not be set from the terminal at all.
pub(crate) fn coerce_to_existing_type(current_value: &model::PropertyValue, text_value: &str) -> Option<model::PropertyValue> {
    use model::PropertyValue as P;
    let t = text_value.trim();

    /// "1, 2, 3" -> [1.0, 2.0, 3.0]; None if anything is not a number.
    fn numbers(t: &str) -> Option<Vec<f32>> {
        t.split(',')
            .map(|p| p.trim().parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()
    }

    match current_value {
        P::BrickColor(_) => Some(P::BrickColor(t.to_string())),
        // A Ref target may be a UUID or a short handle; resolution happens on the Studio side.
        P::Ref(_) => Some(P::Ref(t.to_string())),
        P::String(_) => Some(P::String(t.to_string())),
        // Asset ids are written as text: "rbxassetid://123".
        P::Content(_) => Some(P::Content(t.to_string())),
        P::Vector2 { .. } => match numbers(t)?[..] {
            [x, y] => Some(P::Vector2 { x, y }),
            _ => None,
        },
        P::UDim { .. } => match numbers(t)?[..] {
            [scale, offset] => Some(P::UDim { scale, offset }),
            _ => None,
        },
        P::NumberRange { .. } => match numbers(t)?[..] {
            [min, max] => Some(P::NumberRange { min, max }),
            // A single number pins the range to that point.
            [single] => Some(P::NumberRange { min: single, max: single }),
            _ => None,
        },
        P::UDim2 { .. } => match numbers(t)?[..] {
            [xs, xo, ys, yo] => Some(P::UDim2 { xs, xo, ys, yo }),
            _ => None,
        },
        P::CFrame { rot, .. } => {
            let s = numbers(t)?;
            match s.len() {
                // Only a position was given: the current rotation is kept. This is the most common request.
                3 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: *rot,
                }),
                12 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: [s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11]],
                }),
                _ => None,
            }
        }
        // Every form parse_color3 knows: hex, 0-1 or 0-255 triplets, rgb(), names.
        P::Color3 { .. } => parse_color3(t).map(|[r, g, b]| P::Color3 { r, g, b }),
        P::Rect { .. } => match numbers(t)?[..] {
            [x0, y0, x1, y1] => Some(P::Rect {
                min: [x0, y0],
                max: [x1, y1],
            }),
            _ => None,
        },
        P::PhysicalProperties { .. } => match numbers(t)?[..] {
            [d, f, e, fw, ew] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: fw,
                elasticity_weight: ew,
            }),
            // Three values is the most common form; the weights stay at Roblox's defaults.
            [d, f, e] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: 1.0,
                elasticity_weight: 1.0,
            }),
            _ => None,
        },
        // Curve types cannot be typed point by point in the terminal; the given values
        // are spread at EQUAL intervals along the time axis. "1,0" = fade out from start to end.
        P::NumberSequence(_) => {
            let v = numbers(t)?;
            if v.is_empty() {
                return None;
            }
            let last_item = (v.len() - 1).max(1) as f32;
            Some(P::NumberSequence(
                v.iter()
                    .enumerate()
                    .map(|(i, raw_value)| model::NumberKeypoint {
                        t: i as f32 / last_item,
                        v: *raw_value,
                        envelope: 0.0,
                    })
                    .collect(),
            ))
        }
        // Like "#ff0000,#0000ff": colours are spread at equal intervals. If even one piece
        // is not a colour the whole value is rejected: a curve applied halfway is more
        // confusing than one not applied at all.
        P::ColorSequence(_) => parse_color_list(t).map(|colors| color_sequence(&colors)),
        // Only the family changes; weight and style keep their current values.
        P::Font { weight, style, .. } => Some(P::Font {
            family: t.to_string(),
            weight: weight.clone(),
            style: style.clone(),
        }),
        // The rest (Vector3, Number, Boolean) already come out right in the general parser.
        _ => None,
    }
}

/// The instance's own spelling of a property typed in another case ("size" -> "Size").
///
/// Studio's property names are case-sensitive: `set Box size 4,1,2` sent "size", Studio
/// refused it, and the core had already stored a second property "size" beside "Size".
/// A name that matches no property, or more than one, is returned as typed: guessing
/// between two would be worse than Studio's error.
pub(crate) fn canonical_property_name(
    properties: &std::collections::BTreeMap<String, model::PropertyValue>,
    typed: &str,
) -> String {
    if properties.contains_key(typed) {
        return typed.to_string();
    }
    // Name lives on the instance, not in the property map.
    if typed.eq_ignore_ascii_case("Name") {
        return "Name".to_string();
    }
    let mut matches = properties.keys().filter(|k| k.eq_ignore_ascii_case(typed));
    match (matches.next(), matches.next()) {
        (Some(only), None) => only.clone(),
        _ => typed.to_string(),
    }
}

/// Turns text typed in the terminal into the value the property needs.
///
/// Err means the text cannot be what the property takes and the property must be left
/// alone: forwarding it anyway is how `set Part Color red` reached Studio as a string, was
/// refused there ("Color3 expected, got string"), and the terminal still printed success.
/// Only colour targets can fail today; other types keep the general parser's best guess.
///
/// `class_name` is the instance's class when known: it tells an enum an import left as its
/// bare number from a real number, so a name may still be typed for it.
pub(crate) fn value_for_class(
    class_name: Option<&str>,
    property: &str,
    current_value: Option<&model::PropertyValue>,
    text_value: &str,
) -> Result<model::PropertyValue, String> {
    let typed = text_value.trim();
    if let Some(target) = color_target(property, current_value) {
        let colors = parse_color_list(text_value).ok_or_else(|| not_a_colour(typed))?;
        return match (target, colors.as_slice()) {
            (ColorTarget::Sequence, _) => Ok(color_sequence(&colors)),
            (_, [[r, g, b]]) => Ok(model::PropertyValue::Color3 { r: *r, g: *g, b: *b }),
            (ColorTarget::One, _) => Err(format!(
                "'{}' is several colours, and {} takes one",
                text_value.trim(),
                property
            )),
            (ColorTarget::Either, _) => Ok(color_sequence(&colors)),
        };
    }
    if let Some(value) = enum_from_text(current_value, typed)? {
        return Ok(value);
    }
    let parsed = current_value
        .and_then(|current| coerce_to_existing_type(current, text_value))
        .unwrap_or_else(|| parse_property_value(text_value));

    // A value that cannot be what the property holds went to Studio as text and was
    // refused there, after the terminal had printed success: `set Box Anchored ture`.
    let enum_property = class_name.is_some_and(|c| catalog::is_enum_property(c, property));
    let enum_text = matches!(&parsed, model::PropertyValue::String(s) if s.starts_with("Enum."));
    if let Some(current) = current_value.filter(|_| !enum_property && !enum_text) {
        if let Some(expected) = expected_form(current) {
            if std::mem::discriminant(current) != std::mem::discriminant(&parsed) {
                let mut reason = format!("'{}' is not {}", typed, expected);
                if matches!(current, model::PropertyValue::Boolean(_)) {
                    if let Some(hint) = suggest::hint(typed, ["true", "false"]) {
                        reason = format!("{}. {}", reason, hint);
                    }
                }
                return Err(reason);
            }
        }
    }
    Ok(parsed)
}

#[cfg(test)]
pub(crate) fn value_from_text(
    property: &str,
    current_value: Option<&model::PropertyValue>,
    text_value: &str,
) -> Result<model::PropertyValue, String> {
    value_for_class(None, property, current_value, text_value)
}

/// How a value of this type is typed, for the types whose text the general parser can
/// only turn into a string Studio refuses. None for types that take any text.
pub(crate) fn expected_form(current: &model::PropertyValue) -> Option<&'static str> {
    use model::PropertyValue as P;
    Some(match current {
        P::Boolean(_) => "true or false",
        P::Number(_) => "a number",
        P::Vector3 { .. } => "x,y,z (three numbers)",
        P::Vector2 { .. } => "x,y (two numbers)",
        P::UDim { .. } => "scale,offset",
        P::UDim2 { .. } => "xScale,xOffset,yScale,yOffset",
        P::CFrame { .. } => "x,y,z (or twelve numbers: position and rotation)",
        P::NumberRange { .. } => "min,max or a single number",
        P::Rect { .. } => "four numbers (min x, min y, max x, max y)",
        P::PhysicalProperties { .. } => "density,friction,elasticity (three or five numbers)",
        P::NumberSequence(_) => "numbers separated by commas",
        _ => return None,
    })
}

/// An enum value typed in the terminal, checked against the enum the property holds now
/// (a model value "Enum.Material.Plastic" says it takes a Material). A slip in the enum's
/// name, or in an item of an enum Syncix lists, is refused with the closest spelling; a
/// listed item in the wrong case is written the way Roblox spells it, since
/// `Enum.Material.neon` fails in Studio. Ok(None): not an enum value, or not one to judge.
pub(crate) fn enum_from_text(
    current_value: Option<&model::PropertyValue>,
    typed: &str,
) -> Result<Option<model::PropertyValue>, String> {
    let Some(model::PropertyValue::String(current)) = current_value else {
        return Ok(None);
    };
    let Some(enum_name) = current.strip_prefix("Enum.").and_then(|rest| rest.split('.').next()) else {
        return Ok(None);
    };
    let item = match typed.strip_prefix("Enum.") {
        Some(rest) => {
            let (typed_enum, item) = rest.split_once('.').unwrap_or((rest, ""));
            if !typed_enum.eq_ignore_ascii_case(enum_name) {
                let hint = if item.is_empty() {
                    String::new()
                } else {
                    format!(" Did you mean Enum.{}.{}?", enum_name, item)
                };
                return Err(format!("'{}' is not an Enum.{} value.{}", typed, enum_name, hint));
            }
            item
        }
        // A bare word ("Neon") names an item; anything else goes on as typed.
        None if typed.starts_with(|c: char| c.is_ascii_alphabetic())
            && typed.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            typed
        }
        None => return Ok(None),
    };
    let Some(items) = catalog::enum_items(enum_name) else {
        return Ok(None);
    };
    if let Some(real) = items.iter().find(|i| i.eq_ignore_ascii_case(item)) {
        return Ok(Some(model::PropertyValue::String(format!("Enum.{}.{}", enum_name, real))));
    }
    match suggest::hint(item, items.iter().copied()) {
        Some(hint) => Err(format!("{} is not an Enum.{} item. {}", item, enum_name, hint)),
        // Possibly newer than the list; Studio has the last word.
        None => Ok(None),
    }
}
