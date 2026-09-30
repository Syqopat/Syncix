//! type coercion tests.

use crate::model::InstanceNode;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, transport};

#[allow(unused_imports)]
use crate::runtime::resend_payloads;

use super::*;
use model::PropertyValue as P;

/// This was the real bug: `set <part> BrickColor "Really red"` produced a plain String,
/// and the assignment silently failed in Studio.
#[test]
fn brickcolor_text_stays_brickcolor() {
    let current_value = P::BrickColor("Medium stone grey".into());
    assert_eq!(
        coerce_to_existing_type(&current_value, "Really red"),
        Some(P::BrickColor("Really red".into()))
    );
}

/// With three numbers the rotation must be kept: the user only wants to move it.
#[test]
fn cframe_three_numbers_keep_rotation() {
    let rot = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let current_value = P::CFrame {
        pos: [1.0, 2.0, 3.0],
        rot,
    };
    assert_eq!(
        coerce_to_existing_type(&current_value, "10, 0, -5"),
        Some(P::CFrame {
            pos: [10.0, 0.0, -5.0],
            rot
        })
    );
}

#[test]
fn udim2_four_numbers() {
    let current_value = P::UDim2 {
        xs: 0.0,
        xo: 0.0,
        ys: 0.0,
        yo: 0.0,
    };
    assert_eq!(
        coerce_to_existing_type(&current_value, "0.5,10,0.25,-4"),
        Some(P::UDim2 {
            xs: 0.5,
            xo: 10.0,
            ys: 0.25,
            yo: -4.0
        })
    );
}

/// A number-like value must be writable to a text property:
/// the general parser would turn "5" into a Number and Studio would reject the text.
#[test]
fn string_property_does_not_become_number() {
    let current_value = P::String("hello".into());
    assert_eq!(
        coerce_to_existing_type(&current_value, "5"),
        Some(P::String("5".into()))
    );
}

/// Input that cannot be fitted must return None so the caller falls back to the general parser.
#[test]
fn bad_input_returns_none() {
    let current_value = P::Vector2 { x: 0.0, y: 0.0 };
    assert_eq!(coerce_to_existing_type(&current_value, "abc"), None);
    assert_eq!(coerce_to_existing_type(&current_value, "1,2,3"), None);
}

#[test]
fn color3_accepts_hex_and_triplet() {
    let current_value = P::Color3 {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };
    assert_eq!(
        coerce_to_existing_type(&current_value, "#ff0000"),
        Some(P::Color3 {
            r: 1.0,
            g: 0.0,
            b: 0.0
        })
    );
    assert_eq!(
        coerce_to_existing_type(&current_value, "0,0.5,1"),
        Some(P::Color3 {
            r: 0.0,
            g: 0.5,
            b: 1.0
        })
    );
}
