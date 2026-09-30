//! Reading a colour the user typed: names, hex, Color3.new(...), a sequence.

use crate::{model, suggest};

#[allow(unused_imports)]
use super::*;

/// Colour names `syncix set` understands for a colour property, as 0-255 values. They are
/// the CSS values, so "green" is the darker web green and "lime" the bright one.
pub(crate) const COLOR_NAMES: &[(&str, [u8; 3])] = &[
    ("red", [255, 0, 0]),
    ("green", [0, 128, 0]),
    ("blue", [0, 0, 255]),
    ("white", [255, 255, 255]),
    ("black", [0, 0, 0]),
    ("yellow", [255, 255, 0]),
    ("orange", [255, 165, 0]),
    ("purple", [128, 0, 128]),
    ("pink", [255, 192, 203]),
    ("gray", [128, 128, 128]),
    ("grey", [128, 128, 128]),
    ("cyan", [0, 255, 255]),
    ("brown", [165, 42, 42]),
    ("lime", [0, 255, 0]),
    ("navy", [0, 0, 128]),
    ("teal", [0, 128, 128]),
    ("magenta", [255, 0, 255]),
    ("maroon", [128, 0, 0]),
    ("olive", [128, 128, 0]),
    ("silver", [192, 192, 192]),
    ("gold", [255, 215, 0]),
    ("violet", [238, 130, 238]),
    ("indigo", [75, 0, 130]),
    ("beige", [245, 245, 220]),
    ("turquoise", [64, 224, 208]),
    ("coral", [255, 127, 80]),
    ("crimson", [220, 20, 60]),
    ("salmon", [250, 128, 114]),
    ("lavender", [230, 230, 250]),
    ("khaki", [240, 230, 140]),
    ("chocolate", [210, 105, 30]),
    ("skyblue", [135, 206, 235]),
    ("lightblue", [173, 216, 230]),
    ("darkblue", [0, 0, 139]),
    ("darkgreen", [0, 100, 0]),
    ("lightgreen", [144, 238, 144]),
    ("darkred", [139, 0, 0]),
    ("lightgray", [211, 211, 211]),
    ("lightgrey", [211, 211, 211]),
];

/// "Sky Blue", "sky_blue" and "sky-blue" are all "skyblue", the key COLOR_NAMES uses.
pub(crate) fn compact_colour_name(text: &str) -> String {
    text.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-'))
        .collect()
}

/// "'oragne' is not a colour. Did you mean orange?"
pub(crate) fn not_a_colour(typed: &str) -> String {
    match suggest::hint(&compact_colour_name(typed), COLOR_NAMES.iter().map(|(name, _)| *name)) {
        Some(hint) => format!("'{}' is not a colour. {}", typed, hint),
        None => format!("'{}' is not a colour", typed),
    }
}

/// Reads a colour typed in the terminal into Color3 components (0-1); None if the text
/// is no colour.
///
/// Why it is this forgiving: `syncix set Part Color red` reached Studio as a string and was
/// refused ("Color3 expected, got string"), and `... Color 255,136,0` was stored as a
/// Color3 of 255 rather than 1. The accepted forms are the ones people copy from colour
/// pickers, CSS and Luau. Anything else is None, so the caller refuses loudly instead of
/// guessing.
///
/// Three bare numbers are 0-1 when all of them are at most 1, otherwise 0-255
/// ("1,0.5,0" and "255,128,0" are the same orange); rgb(...) follows the same rule.
/// Color3.new and Color3.fromRGB state their scale themselves.
pub(crate) fn parse_color3(text: &str) -> Option<[f32; 3]> {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    let lower = t.to_ascii_lowercase();
    let compact = compact_colour_name(t);
    if let Some(&(_, rgb)) = COLOR_NAMES.iter().find(|(name, _)| *name == compact) {
        return Some(rgb.map(|c| c as f32 / 255.0));
    }
    if let Some(args) = call_arguments(&lower, "color3.fromrgb") {
        let v = three_color_numbers(args)?;
        if v.iter().any(|c| *c > 255.0) {
            return None;
        }
        return Some(v.map(|c| c / 255.0));
    }
    if let Some(args) = call_arguments(&lower, "color3.new") {
        let v = three_color_numbers(args)?;
        return v.iter().all(|c| *c <= 1.0).then_some(v);
    }
    let v = three_color_numbers(call_arguments(&lower, "rgb").unwrap_or(&lower))?;
    if v.iter().any(|c| *c > 255.0) {
        None
    } else if v.iter().all(|c| *c <= 1.0) {
        Some(v)
    } else {
        Some(v.map(|c| c / 255.0))
    }
}

/// "ff8800" or "f80" (without the '#') -> Color3 components.
pub(crate) fn parse_hex_color(hex: &str) -> Option<[f32; 3]> {
    // Checked first: the slicing below counts bytes, which is only safe on ASCII.
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |digits: &str| u8::from_str_radix(digits, 16).ok().map(|v| v as f32 / 255.0);
    match hex.len() {
        6 => Some([channel(&hex[0..2])?, channel(&hex[2..4])?, channel(&hex[4..6])?]),
        // "#f80" is short for "#ff8800": every digit is doubled.
        3 => Some([
            channel(&hex[0..1].repeat(2))?,
            channel(&hex[1..2].repeat(2))?,
            channel(&hex[2..3].repeat(2))?,
        ]),
        _ => None,
    }
}

/// The inside of a call such as "rgb(1, 2, 3)"; `name` must be lowercase, like `text`.
pub(crate) fn call_arguments<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

/// "1, 0.5, 0" -> [1.0, 0.5, 0.0]; None unless there are exactly three finite,
/// non-negative numbers ("nan" and "inf" parse as f32, and are no colour).
pub(crate) fn three_color_numbers(text: &str) -> Option<[f32; 3]> {
    let parts: Vec<&str> = text.split(',').collect();
    let [r, g, b] = parts[..] else {
        return None;
    };
    let number = |s: &str| s.trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
    Some([number(r)?, number(g)?, number(b)?])
}

/// Splits at commas that are not inside parentheses, so "rgb(1, 0, 0), #00f" is two pieces.
pub(crate) fn split_outside_parentheses(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                pieces.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    pieces.push(text[start..].trim());
    pieces
}

/// One colour, or several separated by commas ("#f00, #00f", "red, rgb(0, 0, 255)").
/// The whole text is tried as one colour first, so "1,0,0" is red and not three pieces.
pub(crate) fn parse_color_list(text: &str) -> Option<Vec<[f32; 3]>> {
    if let Some(color) = parse_color3(text) {
        return Some(vec![color]);
    }
    let pieces = split_outside_parentheses(text);
    if pieces.len() < 2 {
        return None;
    }
    pieces.into_iter().map(parse_color3).collect()
}

/// Colours spread at equal intervals along a ColorSequence. A single colour becomes a
/// constant sequence: Roblox refuses a sequence without a point at time 1, so the lone
/// keypoint at 0 that `set Fire Color #f00` used to produce could not be applied.
pub(crate) fn color_sequence(colors: &[[f32; 3]]) -> model::PropertyValue {
    let points: Vec<[f32; 3]> = match colors {
        [only] => vec![*only, *only],
        _ => colors.to_vec(),
    };
    let last_item = points.len().saturating_sub(1).max(1) as f32;
    model::PropertyValue::ColorSequence(
        points
            .iter()
            .enumerate()
            .map(|(i, [r, g, b])| model::ColorKeypoint {
                t: i as f32 / last_item,
                r: *r,
                g: *g,
                b: *b,
            })
            .collect(),
    )
}

/// Whether a property's name promises a colour (Part.Color, BackgroundColor3, TextColor3).
/// Used when the model holds no value to judge the type by, and by the CLI, which never does.
pub(crate) fn is_color_property_name(property: &str) -> bool {
    let lower = property.trim().to_ascii_lowercase();
    lower == "color" || lower.ends_with("color3")
}

/// What kind of colour a property takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColorTarget {
    /// A Color3.
    One,
    /// A ColorSequence (ParticleEmitter, Beam, Trail).
    Sequence,
    /// Known only by the name "Color", which is a Color3 on a Part and a ColorSequence on a
    /// ParticleEmitter: one colour is taken as a Color3, several as a sequence.
    Either,
}

/// The colour a property takes, judged by its current value when the model has one and by
/// its name when it does not. None when it takes no colour.
pub(crate) fn color_target(property: &str, current_value: Option<&model::PropertyValue>) -> Option<ColorTarget> {
    use model::PropertyValue as P;
    match current_value {
        Some(P::Color3 { .. }) => Some(ColorTarget::One),
        Some(P::ColorSequence(_)) => Some(ColorTarget::Sequence),
        // A String under a colour name is not type information: it is what the old parser
        // stored after `set Part Color red`, and trusting it would forward text again.
        None | Some(P::String(_)) if is_color_property_name(property) => {
            if property.trim().eq_ignore_ascii_case("color") {
                Some(ColorTarget::Either)
            } else {
                Some(ColorTarget::One)
            }
        }
        _ => None,
    }
}

/// The colour forms parse_color3 accepts, one line each, for the core's warning and the
/// CLI's error. Kept next to the parser so the two cannot drift apart.
pub(crate) fn color_forms_help() -> Vec<String> {
    let names: Vec<&str> = COLOR_NAMES.iter().map(|(name, _)| *name).collect();
    vec![
        "Colours: #ff8800 | #f80 | 1,0.5,0 (0-1) | 255,136,0 (0-255) | rgb(255, 136, 0)".to_string(),
        "         Color3.fromRGB(255, 136, 0) | Color3.new(1, 0.5, 0)".to_string(),
        format!("Names:   {}", names.join(", ")),
        "Several colours separated by commas (#f00, #00f) make a ColorSequence.".to_string(),
    ]
}
