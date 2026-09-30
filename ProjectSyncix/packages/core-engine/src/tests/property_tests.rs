//! property tests.

use crate::model::InstanceNode;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, transport};

#[allow(unused_imports)]
use crate::runtime::resend_payloads;

use super::*;
use model::PropertyValue;

/// Wire-format round trip: pv -> wire -> pv must give the same value.
/// If a type is lost in this loop, sync silently loses data.
#[test]
fn wire_format_round_trip_all_types() {
    let samples = vec![
        PropertyValue::String("Hello".into()),
        PropertyValue::Number(42.5),
        PropertyValue::Boolean(true),
        PropertyValue::Boolean(false),
        PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 },
        PropertyValue::Color3 { r: 0.35, g: 0.66, b: 0.2 },
        PropertyValue::UDim2 { xs: 0.5, xo: 10.0, ys: 1.0, yo: -4.0 },
    ];

    for sample in samples {
        let wire = pv_to_wire(&sample);
        let restored_count = parse_wire_value(&wire)
            .unwrap_or_else(|| panic!("could not parse wire value: {:?} -> {}", sample, wire));
        assert_eq!(restored_count, sample, "round-trip broken: {}", wire);
    }
}

/// Vector3 must NEVER be plain text on the wire.
/// Sent as plain text, Studio rejects it with "Vector3 expected" and
/// the object stays at 0,0,0 — exactly what the Position bug did.
#[test]
fn vector3_goes_as_table_on_wire() {
    let wire = pv_to_wire(&PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 });
    assert!(wire.is_object(), "Vector3 must be a table, not a plain value: {}", wire);
    assert!(wire.get("Vector3").is_some(), "the Vector3 key must be present: {}", wire);
    assert_eq!(wire["Vector3"]["y"].as_f64().unwrap(), 2.0);
}

#[test]
fn color3_goes_as_table_on_wire() {
    let wire = pv_to_wire(&PropertyValue::Color3 { r: 1.0, g: 0.0, b: 0.5 });
    assert!(wire.get("Color3").is_some(), "the Color3 key must be present: {}", wire);
}

/// Text from the CLI and HTTP must be converted to the right type.
/// If "0,0.5,-60" stays a String the position is not applied; this is the input side of the bug.
#[test]
fn resend_covers_what_was_delivered_and_kept() {
    let mut dm = model::DataModel::new();
    let ws = InstanceNode::new("Workspace", "Workspace");
    let ws_id = ws.syncix_id;
    dm.upsert_instance(ws).unwrap();
    let mut part = InstanceNode::new("Part", "Box");
    part.parent = Some(ws_id);
    part.properties.insert("Transparency".into(), model::PropertyValue::Number(0.5));
    let part_id = part.syncix_id;
    dm.upsert_instance(part).unwrap();

    let delivered = transport::InFlight {
        creates: [part_id].into_iter().collect(),
        properties: [(part_id, "Transparency".to_string())].into_iter().collect(),
        ..Default::default()
    };
    let payloads = resend_payloads(&dm, &[part_id], &delivered);
    assert!(payloads
        .iter()
        .any(|p| p.event_type == EventType::PushUpdate && p.data["syncix_id"] == part_id.to_string()));
    assert!(payloads.iter().any(|p| p.event_type == EventType::CompositeUpdate));

    // A create that was only queued (not delivered) is not sent again.
    assert!(resend_payloads(&dm, &[part_id], &transport::InFlight::default()).is_empty());
}

#[test]
fn text_parses_as_vector3() {
    assert_eq!(
        parse_property_value("0,0.5,-60"),
        PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 }
    );
    // Negative values actually used in the map
    assert_eq!(
        parse_property_value("-20, 0.5, -6"),
        PropertyValue::Vector3 { x: -20.0, y: 0.5, z: -6.0 }
    );
    // The zero vector must not be read as a Number
    assert_eq!(
        parse_property_value("0,0,0"),
        PropertyValue::Vector3 { x: 0.0, y: 0.0, z: 0.0 }
    );
}

/// The hex colour bug: if "#5aa832" stays a String, Studio rejects it.
#[test]
fn hex_color_parses_as_color3() {
    match parse_property_value("#5aa832") {
        PropertyValue::Color3 { r, g, b } => {
            assert_eq!((r * 255.0).round() as u8, 0x5a);
            assert_eq!((g * 255.0).round() as u8, 0xa8);
            assert_eq!((b * 255.0).round() as u8, 0x32);
        }
        other => panic!("a hex colour must become Color3, got: {:?}", other),
    }
}

/// Without a hash it is not hex: "abcdef" could be a name, "123456" is a number.
/// Without this distinction, name fields would accidentally turn into colours.
#[test]
fn text_without_hash_is_not_a_color() {
    assert_eq!(parse_property_value("abcdef"), PropertyValue::String("abcdef".into()));
    assert_eq!(parse_property_value("123456"), PropertyValue::Number(123456.0));
}

#[test]
fn scalar_types_parse_correctly() {
    assert_eq!(parse_property_value("true"), PropertyValue::Boolean(true));
    assert_eq!(parse_property_value("False"), PropertyValue::Boolean(false));
    assert_eq!(parse_property_value("5"), PropertyValue::Number(5.0));
    assert_eq!(parse_property_value("-3.5"), PropertyValue::Number(-3.5));
    assert_eq!(parse_property_value("Box"), PropertyValue::String("Box".into()));
}

/// Enum values must pass as text; the Studio side does the resolving.
/// If they turned into a number or another type here, properties such as Material/Font
/// would silently not apply.
#[test]
fn enum_text_passes_through() {
    assert_eq!(
        parse_property_value("Enum.Material.Neon"),
        PropertyValue::String("Enum.Material.Neon".into())
    );
    let wire = pv_to_wire(&PropertyValue::String("Enum.Material.Neon".into()));
    assert_eq!(wire.as_str(), Some("Enum.Material.Neon"));
}

/// Studio may send a raw scalar or the serde enum form.
/// If either were not accepted, updates coming from Studio would be dropped.
#[test]
fn raw_and_serde_forms_are_accepted() {
    assert_eq!(
        parse_wire_value(&serde_json::json!(7)),
        Some(PropertyValue::Number(7.0))
    );
    assert_eq!(
        parse_wire_value(&serde_json::json!("text")),
        Some(PropertyValue::String("text".into()))
    );
    assert_eq!(
        parse_wire_value(&serde_json::json!(true)),
        Some(PropertyValue::Boolean(true))
    );
    assert_eq!(
        parse_wire_value(&serde_json::json!({"Number": 3.0})),
        Some(PropertyValue::Number(3.0))
    );
}
