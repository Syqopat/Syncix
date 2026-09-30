//! wire format tests.

use crate::model::InstanceNode;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, transport};

#[allow(unused_imports)]
use crate::runtime::resend_payloads;

use super::*;
use model::{ColorKeypoint, NumberKeypoint, PropertyValue as P};

/// A type has only really been carried once it can be both written and read.
/// Types added in one direction only were the source of
/// "sent, but lost on the other side".
fn round_trip(pv: P) {
    let on_wire = pv_to_wire(&pv);
    let restored_count = parse_wire_value(&on_wire);
    assert_eq!(restored_count, Some(pv.clone()), "wire format: {:?}", on_wire);
}

#[test]
fn asset_id_round_trip() {
    round_trip(P::Content("rbxassetid://123456".into()));
}

#[test]
fn color_sequence_round_trip() {
    round_trip(P::ColorSequence(vec![
        ColorKeypoint { t: 0.0, r: 1.0, g: 0.0, b: 0.0 },
        ColorKeypoint { t: 1.0, r: 0.0, g: 0.0, b: 1.0 },
    ]));
}

/// The envelope is Roblox's randomness margin; if it drops, particle effects flatten.
#[test]
fn number_sequence_keeps_envelope() {
    let pv = P::NumberSequence(vec![
        NumberKeypoint { t: 0.0, v: 1.0, envelope: 0.25 },
        NumberKeypoint { t: 1.0, v: 0.0, envelope: 0.0 },
    ]);
    let restored_count = parse_wire_value(&pv_to_wire(&pv));
    match restored_count {
        Some(P::NumberSequence(k)) => assert_eq!(k[0].envelope, 0.25),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn rect_and_font_round_trip() {
    round_trip(P::Rect { min: [4.0, 4.0], max: [12.0, 12.0] });
    round_trip(P::Font {
        family: "rbxasset://fonts/families/SourceSansPro.json".into(),
        weight: "Enum.FontWeight.Bold".into(),
        style: "Enum.FontStyle.Normal".into(),
    });
}

#[test]
fn physical_properties_round_trip() {
    round_trip(P::PhysicalProperties {
        density: 0.7,
        friction: 0.3,
        elasticity: 0.5,
        friction_weight: 1.0,
        elasticity_weight: 2.0,
    });
}

/// A value sent to Studio must NOT be in serde's tagged form.
///
/// This is a regression test: for a while the disk side put `PropertyValue` straight
/// into JSON, and serde wrote it as {"Number":0.5}. The plugin expects the plain
/// form, so it rejected it with "unsupported table value for property: Transparency"
/// — caught in live use.
#[test]
fn wire_format_has_no_serde_tag() {
    for pv in [
        P::Number(0.5),
        P::String("hello".into()),
        P::Boolean(true),
    ] {
        let on_wire = pv_to_wire(&pv);
        assert!(
            !on_wire.is_object(),
            "a primitive value must go plain, not as a table: {:?} -> {}",
            pv,
            on_wire
        );
        // Serde's form really must be different; the test verifies its own assumption.
        let serde_form = serde_json::to_value(&pv).unwrap();
        assert!(serde_form.is_object(), "serde must write the tagged form: {}", serde_form);
        assert_ne!(on_wire, serde_form);
    }
}

/// Previously added types must not break either: new branches join an ordered if/else
/// chain, and a branch added in the wrong place can shadow an earlier one.
#[test]
fn legacy_types_still_work() {
    round_trip(P::BrickColor("Really red".into()));
    round_trip(P::Ref("11111111-2222-4333-8444-555555555555".into()));
    round_trip(P::CFrame {
        pos: [1.0, 2.0, 3.0],
        rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    });
    round_trip(P::NumberRange { min: 1.0, max: 5.0 });
    round_trip(P::UDim { scale: 0.5, offset: 10.0 });
    round_trip(P::Vector2 { x: 1.0, y: 2.0 });
}
