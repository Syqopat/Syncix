//! mesh creation tests.

use crate::model::InstanceNode;
use crate::transport::{EventType, Payload};
use crate::values::*;
use crate::{model, transport};

#[allow(unused_imports)]
use crate::runtime::resend_payloads;

use super::*;

/// A MeshPart is created FROM its mesh; the mesh must reach the plugin with the
/// create and never as a property, whichever way the file spelled it.
#[test]
fn a_meshpart_carries_its_mesh_to_the_create() {
    let props = serde_json::json!({
        "MeshId": { "Content": "rbxassetid://123" },
        "CollisionFidelity": { "String": "Enum.CollisionFidelity.Box" },
        "RenderFidelity": { "String": "Enum.RenderFidelity.Precise" },
        "Anchored": { "Boolean": true }
    });
    let (mesh, collision, render) = mesh_create_fields("MeshPart", Some(&props));
    assert_eq!(mesh.as_deref(), Some("rbxassetid://123"));
    assert_eq!(collision.as_deref(), Some("Enum.CollisionFidelity.Box"));
    assert_eq!(render.as_deref(), Some("Enum.RenderFidelity.Precise"));
    assert!(is_creation_only_property("MeshPart", "MeshId"));
    assert!(!is_creation_only_property("MeshPart", "Anchored"));
    // Only a MeshPart is built this way; a Part named MeshId keeps its property.
    assert_eq!(mesh_create_fields("Part", Some(&props)), (None, None, None));
    assert!(!is_creation_only_property("Part", "MeshId"));
}

/// Older files were written before Syncix read MeshId at all, and a mesh that is
/// there but empty is no mesh: neither may turn into a create the plugin cannot use.
#[test]
fn a_meshpart_without_a_mesh_is_created_plainly() {
    let empty = serde_json::json!({ "MeshId": { "Content": "" }, "Anchored": { "Boolean": true } });
    assert_eq!(mesh_create_fields("MeshPart", Some(&empty)).0, None);
    let missing = serde_json::json!({ "Anchored": { "Boolean": true } });
    assert_eq!(mesh_create_fields("MeshPart", Some(&missing)).0, None);
    assert_eq!(mesh_create_fields("MeshPart", None), (None, None, None));
    // The newer Content field is accepted under its own name too.
    let newer = serde_json::json!({ "MeshContent": "rbxassetid://9" });
    assert_eq!(mesh_create_fields("MeshPart", Some(&newer)).0.as_deref(), Some("rbxassetid://9"));
}
