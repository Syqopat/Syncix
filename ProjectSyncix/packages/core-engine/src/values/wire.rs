//! Property values on the wire: JSON in, JSON out, and what only a create can set.

use crate::model::PropertyValue;
use crate::{catalog, model, suggest};

#[allow(unused_imports)]
use super::*;

/// Resolves a command target: full UUID, short UUID prefix or name.
/// If a name matches more than one object, None is returned because it is ambiguous.
pub(crate) fn resolve_id(dm: &model::DataModel, target: &str) -> Option<uuid::Uuid> {
    match dm.resolve_target(target) {
        model::ResolveResult::One(u) => Some(u),
        model::ResolveResult::NotFound => None,
        model::ResolveResult::Ambiguous(candidates) => {
            tracing::warn!(
                "{} instances match the name '{}'; the request was ambiguous so nothing was done. Use the short UUID.",
                target,
                candidates.len()
            );
            None
        }
    }
}

/// Turns a JSON value in wire format into a PropertyValue.
/// Accepts raw scalars (5, "hi", true), {Vector3:{..}}/{Color3:{..}} tables
/// and the serde enum form ({"Number":5}).
/// What a MeshPart has to be CREATED with. Roblox does not let a plugin write MeshId, so
/// the mesh (and the fidelity it is built at) travels with the create and the plugin
/// builds the part from it; sent as a property it would only be refused.
/// Returns (mesh, collision fidelity, render fidelity), all as the text the file held.
pub(crate) fn mesh_create_fields(
    class_name: &str,
    properties: Option<&serde_json::Value>,
) -> (Option<String>, Option<String>, Option<String>) {
    if class_name != "MeshPart" {
        return (None, None, None);
    }
    let Some(props) = properties.and_then(|v| v.as_object()) else {
        return (None, None, None);
    };
    let text = |key: &str| -> Option<String> {
        let raw = props.get(key)?;
        raw.get("Content")
            .or_else(|| raw.get("String"))
            .and_then(|v| v.as_str())
            .or_else(|| raw.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    (
        text("MeshId").or_else(|| text("MeshContent")),
        text("CollisionFidelity"),
        text("RenderFidelity"),
    )
}

/// Is this a value the plugin cannot write, only create the object from?
pub(crate) fn is_creation_only_property(class_name: &str, property: &str) -> bool {
    class_name == "MeshPart" && (property == "MeshId" || property == "MeshContent")
}

pub(crate) fn parse_wire_value(v: &serde_json::Value) -> Option<model::PropertyValue> {
    use model::PropertyValue;
    match v {
        serde_json::Value::String(s) => Some(PropertyValue::String(s.clone())),
        serde_json::Value::Bool(b) => Some(PropertyValue::Boolean(*b)),
        serde_json::Value::Number(n) => n.as_f64().map(PropertyValue::Number),
        serde_json::Value::Object(o) => {
            if let Some(v3) = o.get("Vector3") {
                Some(PropertyValue::Vector3 {
                    x: v3.get("x")?.as_f64()? as f32,
                    y: v3.get("y")?.as_f64()? as f32,
                    z: v3.get("z")?.as_f64()? as f32,
                })
            } else if let Some(c3) = o.get("Color3") {
                Some(PropertyValue::Color3 {
                    r: c3.get("r")?.as_f64()? as f32,
                    g: c3.get("g")?.as_f64()? as f32,
                    b: c3.get("b")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim2") {
                Some(PropertyValue::UDim2 {
                    xs: u.get("xs")?.as_f64()? as f32,
                    xo: u.get("xo")?.as_f64()? as f32,
                    ys: u.get("ys")?.as_f64()? as f32,
                    yo: u.get("yo")?.as_f64()? as f32,
                })
            } else if let Some(v) = o.get("Vector2") {
                Some(PropertyValue::Vector2 {
                    x: v.get("x")?.as_f64()? as f32,
                    y: v.get("y")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim") {
                Some(PropertyValue::UDim {
                    scale: u.get("scale")?.as_f64()? as f32,
                    offset: u.get("offset")?.as_f64()? as f32,
                })
            } else if let Some(c) = o.get("CFrame") {
                let pos = c.get("pos")?.as_array()?;
                let rot = c.get("rot")?.as_array()?;
                if pos.len() != 3 || rot.len() != 9 {
                    return None;
                }
                let mut p = [0f32; 3];
                let mut r = [0f32; 9];
                for (i, v) in pos.iter().enumerate() {
                    p[i] = v.as_f64()? as f32;
                }
                for (i, v) in rot.iter().enumerate() {
                    r[i] = v.as_f64()? as f32;
                }
                Some(PropertyValue::CFrame { pos: p, rot: r })
            } else if let Some(n) = o.get("NumberRange") {
                Some(PropertyValue::NumberRange {
                    min: n.get("min")?.as_f64()? as f32,
                    max: n.get("max")?.as_f64()? as f32,
                })
            } else if let Some(r) = o.get("Ref") {
                Some(PropertyValue::Ref(r.as_str()?.to_string()))
            } else if let Some(b) = o.get("BrickColor") {
                Some(PropertyValue::BrickColor(b.as_str()?.to_string()))
            } else if let Some(c) = o.get("Content") {
                Some(PropertyValue::Content(c.as_str()?.to_string()))
            } else if let Some(a) = o.get("ColorSequence") {
                let mut points = Vec::new();
                for k in a.as_array()? {
                    points.push(model::ColorKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        r: k.get("r")?.as_f64()? as f32,
                        g: k.get("g")?.as_f64()? as f32,
                        b: k.get("b")?.as_f64()? as f32,
                    });
                }
                Some(PropertyValue::ColorSequence(points))
            } else if let Some(a) = o.get("NumberSequence") {
                let mut points = Vec::new();
                for k in a.as_array()? {
                    points.push(model::NumberKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        v: k.get("v")?.as_f64()? as f32,
                        envelope: k.get("envelope").and_then(|e| e.as_f64()).unwrap_or(0.0) as f32,
                    });
                }
                Some(PropertyValue::NumberSequence(points))
            } else if let Some(r) = o.get("Rect") {
                let mn = r.get("min")?.as_array()?;
                let mx = r.get("max")?.as_array()?;
                if mn.len() != 2 || mx.len() != 2 {
                    return None;
                }
                Some(PropertyValue::Rect {
                    min: [mn[0].as_f64()? as f32, mn[1].as_f64()? as f32],
                    max: [mx[0].as_f64()? as f32, mx[1].as_f64()? as f32],
                })
            } else if let Some(f) = o.get("Font") {
                Some(PropertyValue::Font {
                    family: f.get("family")?.as_str()?.to_string(),
                    weight: f.get("weight")?.as_str()?.to_string(),
                    style: f.get("style")?.as_str()?.to_string(),
                })
            } else if let Some(pp) = o.get("PhysicalProperties") {
                Some(PropertyValue::PhysicalProperties {
                    density: pp.get("density")?.as_f64()? as f32,
                    friction: pp.get("friction")?.as_f64()? as f32,
                    elasticity: pp.get("elasticity")?.as_f64()? as f32,
                    friction_weight: pp.get("frictionWeight")?.as_f64()? as f32,
                    elasticity_weight: pp.get("elasticityWeight")?.as_f64()? as f32,
                })
            } else {
                serde_json::from_value(v.clone()).ok()
            }
        }
        _ => None,
    }
}

/// Converts a PropertyValue into the wire format the Studio plugin (PatchExecutor) expects.
/// Scalars are sent raw; Vector3/Color3 are wrapped in tables.
pub fn pv_to_wire(pv: &model::PropertyValue) -> serde_json::Value {
    use model::PropertyValue;
    match pv {
        PropertyValue::String(s) => serde_json::json!(s),
        PropertyValue::Number(n) => serde_json::json!(n),
        PropertyValue::Boolean(b) => serde_json::json!(b),
        PropertyValue::Vector3 { x, y, z } => serde_json::json!({ "Vector3": { "x": x, "y": y, "z": z } }),
        PropertyValue::Color3 { r, g, b } => serde_json::json!({ "Color3": { "r": r, "g": g, "b": b } }),
        PropertyValue::UDim2 { xs, xo, ys, yo } => {
            serde_json::json!({ "UDim2": { "xs": xs, "xo": xo, "ys": ys, "yo": yo } })
        }
        PropertyValue::Vector2 { x, y } => serde_json::json!({ "Vector2": { "x": x, "y": y } }),
        PropertyValue::UDim { scale, offset } => {
            serde_json::json!({ "UDim": { "scale": scale, "offset": offset } })
        }
        PropertyValue::CFrame { pos, rot } => {
            serde_json::json!({ "CFrame": { "pos": pos, "rot": rot } })
        }
        PropertyValue::NumberRange { min, max } => {
            serde_json::json!({ "NumberRange": { "min": min, "max": max } })
        }
        PropertyValue::Ref(id) => serde_json::json!({ "Ref": id }),
        PropertyValue::BrickColor(item_name) => serde_json::json!({ "BrickColor": item_name }),
        PropertyValue::Content(u) => serde_json::json!({ "Content": u }),
        PropertyValue::ColorSequence(k) => serde_json::json!({ "ColorSequence": k }),
        PropertyValue::NumberSequence(k) => serde_json::json!({ "NumberSequence": k }),
        PropertyValue::Rect { min, max } => {
            serde_json::json!({ "Rect": { "min": min, "max": max } })
        }
        PropertyValue::Font {
            family,
            weight,
            style,
        } => serde_json::json!({
            "Font": { "family": family, "weight": weight, "style": style }
        }),
        PropertyValue::PhysicalProperties {
            density,
            friction,
            elasticity,
            friction_weight,
            elasticity_weight,
        } => serde_json::json!({
            "PhysicalProperties": {
                "density": density,
                "friction": friction,
                "elasticity": elasticity,
                "frictionWeight": friction_weight,
                "elasticityWeight": elasticity_weight
            }
        }),
    }
}
