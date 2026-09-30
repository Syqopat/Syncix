//! set value tests.

use crate::model::InstanceNode;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, transport};

#[allow(unused_imports)]
use crate::runtime::resend_payloads;

use super::*;
use model::PropertyValue as P;
use std::collections::BTreeMap;

fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn color3([r, g, b]: [f32; 3]) -> P {
    P::Color3 { r, g, b }
}

fn keypoint(t: f32, [r, g, b]: [f32; 3]) -> model::ColorKeypoint {
    model::ColorKeypoint { t, r, g, b }
}

#[test]
fn hex_long_and_short() {
    assert_eq!(parse_color3("#ff8800"), Some(rgb(255, 136, 0)));
    assert_eq!(parse_color3("#FF8800"), Some(rgb(255, 136, 0)));
    assert_eq!(parse_color3("#f80"), Some(rgb(255, 136, 0)));
    assert_eq!(parse_color3("  #0a0  "), Some(rgb(0, 170, 0)));
}

/// Non-ASCII input is in the list on purpose: the hex slicing counts bytes and would
/// panic on it if the digits were not checked first.
#[test]
fn broken_hex_is_refused() {
    for bad in ["#", "#ff88", "#ff880", "#ff88000", "#gg8800", "#ffé", "ff8800"] {
        assert_eq!(parse_color3(bad), None, "{bad}");
    }
}

/// All values at most 1 read as 0-1, otherwise 0-255: "255,136,0" used to be stored
/// as a Color3 of 255.
#[test]
fn triplets_pick_their_scale() {
    assert_eq!(parse_color3("1,0.5,0"), Some([1.0, 0.5, 0.0]));
    assert_eq!(parse_color3("255, 136, 0"), Some(rgb(255, 136, 0)));
    assert_eq!(parse_color3("1,1,1"), Some([1.0, 1.0, 1.0]));
    assert_eq!(parse_color3("0,0,255"), Some(rgb(0, 0, 255)));
    // One value above 1 switches the whole triplet to 0-255.
    assert_eq!(parse_color3("1,0,128"), Some(rgb(1, 0, 128)));
}

#[test]
fn triplets_out_of_range_are_refused() {
    for bad in ["256,0,0", "-1,0,0", "1,2", "1,2,3,4", "a,b,c", "inf,0,0", "NaN,0,0", ""] {
        assert_eq!(parse_color3(bad), None, "{bad}");
    }
}

#[test]
fn rgb_and_luau_forms() {
    assert_eq!(parse_color3("rgb(255, 136, 0)"), Some(rgb(255, 136, 0)));
    assert_eq!(parse_color3("RGB( 0 , 128 , 0 )"), Some(rgb(0, 128, 0)));
    assert_eq!(parse_color3("rgb(1, 0.5, 0)"), Some([1.0, 0.5, 0.0]));
    assert_eq!(parse_color3("Color3.fromRGB(255, 136, 0)"), Some(rgb(255, 136, 0)));
    // fromRGB states its scale: 1 is nearly black, not full intensity.
    assert_eq!(parse_color3("Color3.fromRGB(1, 1, 1)"), Some(rgb(1, 1, 1)));
    assert_eq!(parse_color3("Color3.new(1, 0.5, 0)"), Some([1.0, 0.5, 0.0]));
    assert_eq!(parse_color3("Color3.new(255, 0, 0)"), None);
    assert_eq!(parse_color3("rgb(255, 0)"), None);
    assert_eq!(parse_color3("rgba(255, 0, 0, 1)"), None);
}

#[test]
fn colour_names() {
    assert_eq!(parse_color3("red"), Some(rgb(255, 0, 0)));
    assert_eq!(parse_color3("Grey"), Some(rgb(128, 128, 128)));
    assert_eq!(parse_color3("GRAY"), Some(rgb(128, 128, 128)));
    assert_eq!(parse_color3("lime"), Some(rgb(0, 255, 0)));
    assert_eq!(parse_color3("navy"), Some(rgb(0, 0, 128)));
    for name in [
        "red", "green", "blue", "white", "black", "yellow", "orange", "purple", "pink", "gray", "grey",
        "cyan", "brown", "lime", "navy", "teal",
    ] {
        assert!(parse_color3(name).is_some(), "{name}");
    }
    // BrickColor names are not Color3 names.
    assert_eq!(parse_color3("Really red"), None);
}

#[test]
fn colour_lists() {
    assert_eq!(parse_color_list("1,0,0"), Some(vec![[1.0, 0.0, 0.0]]));
    assert_eq!(parse_color_list("#f00, #00f"), Some(vec![rgb(255, 0, 0), rgb(0, 0, 255)]));
    assert_eq!(
        parse_color_list("red, rgb(0, 0, 255)"),
        Some(vec![rgb(255, 0, 0), rgb(0, 0, 255)])
    );
    assert_eq!(parse_color_list("1,0"), None);
    assert_eq!(parse_color_list("red, nope"), None);
}

#[test]
fn colour_property_names() {
    for name in ["Color", "color", "BackgroundColor3", "TextColor3", "Color3", "ImageColor3"] {
        assert!(is_color_property_name(name), "{name}");
    }
    for name in ["BrickColor", "TeamColor", "Name", "Size", "Colorful"] {
        assert!(!is_color_property_name(name), "{name}");
    }
    // The current value's type wins over the name.
    assert_eq!(color_target("Color", Some(&P::BrickColor("White".into()))), None);
    assert_eq!(color_target("Color", None), Some(ColorTarget::Either));
    assert_eq!(color_target("TextColor3", None), Some(ColorTarget::One));
    assert_eq!(color_target("Ambient", Some(&color3([0.0; 3]))), Some(ColorTarget::One));
}

#[test]
fn property_name_case_is_forgiven() {
    let mut props = BTreeMap::new();
    props.insert("Size".to_string(), P::Vector3 { x: 1.0, y: 1.0, z: 1.0 });
    props.insert("Color".to_string(), color3([0.0; 3]));
    assert_eq!(canonical_property_name(&props, "size"), "Size");
    assert_eq!(canonical_property_name(&props, "COLOR"), "Color");
    assert_eq!(canonical_property_name(&props, "Size"), "Size");
    assert_eq!(canonical_property_name(&props, "name"), "Name");
    // Unknown: passed on as typed, so Studio's error names what was typed.
    assert_eq!(canonical_property_name(&props, "Anchored"), "Anchored");
}

#[test]
fn two_case_matches_are_not_guessed() {
    let mut props = BTreeMap::new();
    props.insert("Value".to_string(), P::Number(1.0));
    props.insert("VALUE".to_string(), P::Number(2.0));
    assert_eq!(canonical_property_name(&props, "value"), "value");
}

#[test]
fn colour_target_takes_any_colour_form() {
    let current = color3([0.0; 3]);
    assert_eq!(value_from_text("Color", Some(&current), "red"), Ok(color3(rgb(255, 0, 0))));
    assert_eq!(value_from_text("Color", Some(&current), "#f80"), Ok(color3(rgb(255, 136, 0))));
    assert_eq!(
        value_from_text("Color", Some(&current), "255, 136, 0"),
        Ok(color3(rgb(255, 136, 0)))
    );
    assert_eq!(
        coerce_to_existing_type(&current, "rgb(255, 136, 0)"),
        Some(color3(rgb(255, 136, 0)))
    );
}

/// The incident: Studio answered "Color3 expected, got string". Text that is no colour
/// must be refused here, not forwarded for Studio to refuse.
#[test]
fn colour_target_refuses_other_text() {
    let current = color3([0.0; 3]);
    assert!(value_from_text("Color", Some(&current), "Really red").is_err());
    assert!(value_from_text("Color", Some(&current), "#f00, #00f").is_err());
    // Judged by name when the model has no value yet.
    assert!(value_from_text("BackgroundColor3", None, "hello").is_err());
    assert!(value_from_text("BackgroundColor3", None, "#f00, #00f").is_err());
    assert_eq!(value_from_text("BackgroundColor3", None, "#fff"), Ok(color3([1.0, 1.0, 1.0])));
}

/// A String stored under a colour name is a leftover of the old parser, not a type.
#[test]
fn leftover_string_does_not_make_a_colour_text() {
    let leftover = P::String("red".into());
    assert_eq!(value_from_text("Color", Some(&leftover), "blue"), Ok(color3(rgb(0, 0, 255))));
    assert!(value_from_text("Color", Some(&leftover), "Really red").is_err());
}

#[test]
fn colour_sequence_target() {
    let current = P::ColorSequence(vec![]);
    let red = rgb(255, 0, 0);
    let blue = rgb(0, 0, 255);
    let red_to_blue = P::ColorSequence(vec![keypoint(0.0, red), keypoint(1.0, blue)]);
    // One colour is a constant sequence, with the point at time 1 Roblox requires.
    assert_eq!(
        value_from_text("Color", Some(&current), "red"),
        Ok(P::ColorSequence(vec![keypoint(0.0, red), keypoint(1.0, red)]))
    );
    assert_eq!(
        coerce_to_existing_type(&current, "#ff0000"),
        Some(P::ColorSequence(vec![keypoint(0.0, red), keypoint(1.0, red)]))
    );
    assert_eq!(value_from_text("Color", Some(&current), "#f00, #00f"), Ok(red_to_blue.clone()));
    assert!(value_from_text("Color", Some(&current), "fire").is_err());
    // No current value: several colours can only be a sequence.
    assert_eq!(value_from_text("Color", None, "#f00, #00f"), Ok(red_to_blue));
}

/// Colour names must not leak into other properties: "red" for a text stays "red".
#[test]
fn other_targets_are_unchanged() {
    assert_eq!(
        value_from_text("Text", Some(&P::String("x".into())), "red"),
        Ok(P::String("red".into()))
    );
    assert_eq!(
        value_from_text("BrickColor", Some(&P::BrickColor("White".into())), "Really red"),
        Ok(P::BrickColor("Really red".into()))
    );
    assert_eq!(value_from_text("Material", None, "red"), Ok(P::String("red".into())));

    // Slips are refused with the closest spelling, before anything reaches Studio.
    let black = P::Color3 { r: 0.0, g: 0.0, b: 0.0 };
    let err = value_from_text("Color", Some(&black), "oragne").unwrap_err();
    assert!(err.contains("Did you mean orange?"), "{}", err);
    assert_eq!(
        value_from_text("Color", Some(&black), "Sky Blue"),
        Ok(P::Color3 { r: 135.0 / 255.0, g: 206.0 / 255.0, b: 235.0 / 255.0 })
    );
    let err = value_from_text("Anchored", Some(&P::Boolean(false)), "ture").unwrap_err();
    assert!(err.contains("Did you mean true?"), "{}", err);
    assert!(value_from_text("Transparency", Some(&P::Number(0.0)), "half").is_err());
    assert!(value_from_text("Size", Some(&P::Vector3 { x: 1.0, y: 1.0, z: 1.0 }), "4,1").is_err());

    let plastic = P::String("Enum.Material.Plastic".into());
    assert_eq!(value_from_text("Material", Some(&plastic), "neon"), Ok(P::String("Enum.Material.Neon".into())));
    assert_eq!(
        value_from_text("Material", Some(&plastic), "Enum.Material.Glass"),
        Ok(P::String("Enum.Material.Glass".into()))
    );
    let err = value_from_text("Material", Some(&plastic), "Neno").unwrap_err();
    assert!(err.contains("Did you mean Neon?"), "{}", err);
    let err = value_from_text("Material", Some(&plastic), "Enum.Materail.Neon").unwrap_err();
    assert!(err.contains("Did you mean Enum.Material.Neon?"), "{}", err);
    // An enum an import left as its bare number still takes a name.
    assert_eq!(
        value_for_class(Some("TextLabel"), "TextXAlignment", Some(&P::Number(0.0)), "Left"),
        Ok(P::String("Left".into()))
    );
    assert_eq!(value_from_text("Transparency", None, "0.5"), Ok(P::Number(0.5)));
    assert_eq!(
        value_from_text("Position", None, "0,5,-60"),
        Ok(P::Vector3 { x: 0.0, y: 5.0, z: -60.0 })
    );
}
