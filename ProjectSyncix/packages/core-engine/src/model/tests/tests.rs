//! tests.

use crate::model::*;

/// Test helper: adds a named node under the parent and returns its id.
fn add(model: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
    let mut node = InstanceNode::new(class, name);
    node.parent = parent;
    let id = node.syncix_id;
    model.upsert_instance(node).expect("upsert failed");
    id
}

/// A new identity (Team Create: two plugins named one object) carries the parent's
/// list, the children and references along, and refuses an identity in use.
#[test]
fn rekey_moves_links_and_references() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let model = add(&mut m, "Model", "Door", Some(ws));
    let part = add(&mut m, "Part", "Hinge", Some(model));
    let value = add(&mut m, "ObjectValue", "Target", Some(ws));
    m.get_mut_instance(&value)
        .unwrap()
        .properties
        .insert("Value".into(), PropertyValue::Ref(model.to_string()));

    let new = Uuid::new_v4();
    m.rekey(&model, &new).expect("rekey failed");

    assert!(m.get_instance(&model).is_none());
    assert_eq!(m.get_instance(&new).unwrap().syncix_id, new);
    assert!(m.get_instance(&ws).unwrap().children.contains(&new));
    assert!(!m.get_instance(&ws).unwrap().children.contains(&model));
    assert_eq!(m.get_instance(&part).unwrap().parent, Some(new));
    assert!(matches!(
        m.get_instance(&value).unwrap().properties.get("Value"),
        Some(PropertyValue::Ref(r)) if *r == new.to_string()
    ));
    assert!(matches!(m.rekey(&new, &part), Err(ModelError::IdentityTaken(_))));
}

/// When a node is deleted its WHOLE subtree must go too (Destroy() behaviour in Studio).
#[test]
fn test_cascade_delete_removes_descendants() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let folder = add(&mut m, "Folder", "Container", Some(ws));
    let part = add(&mut m, "Part", "Box", Some(folder));
    let decal = add(&mut m, "Decal", "Texture", Some(part));

    assert!(m.remove_instance(&folder).is_some());

    assert!(m.get_instance(&folder).is_none(), "the folder must be deleted");
    assert!(m.get_instance(&part).is_none(), "the child must be deleted too");
    assert!(m.get_instance(&decal).is_none(), "the grandchild must be deleted too");
    assert!(m.get_instance(&ws).is_some(), "the parent must stay");
    assert!(
        !m.get_instance(&ws).unwrap().children.contains(&folder),
        "the parent's children list must be cleaned"
    );
}

/// Studio's tree holds the workspace only; the folder and the model inside it are
/// still on their way, so they are put back, parent first, even when listed child first.
#[test]
fn carry_over_keeps_what_studio_has_not_applied() {
    let mut old = DataModel::new();
    let ws = add(&mut old, "Workspace", "Workspace", None);
    let folder = add(&mut old, "Folder", "Decor", Some(ws));
    let tree = add(&mut old, "Model", "Tree", Some(folder));
    let refused = add(&mut old, "Part", "Refused", Some(ws));

    let mut fresh = DataModel::new();
    let mut workspace = old.get_instance(&ws).unwrap().clone();
    workspace.children.clear();
    fresh.upsert_instance(workspace).unwrap();

    let in_flight = crate::transport::InFlight {
        creates: [tree, folder].into_iter().collect(),
        ..Default::default()
    };
    let kept = fresh.carry_over(&old, &in_flight);

    assert_eq!(kept.len(), 2);
    assert_eq!(fresh.get_instance(&folder).unwrap().children, vec![tree]);
    assert!(fresh.get_instance(&ws).unwrap().children.contains(&folder));
    assert!(fresh.get_instance(&refused).is_none(), "not on its way: Studio removed it");
}

#[test]
fn carry_over_needs_a_parent() {
    let mut old = DataModel::new();
    let ws = add(&mut old, "Workspace", "Workspace", None);
    let folder = add(&mut old, "Folder", "Gone", Some(ws));
    let child = add(&mut old, "Part", "Child", Some(folder));
    let mut fresh = DataModel::new();
    let in_flight = crate::transport::InFlight {
        creates: [child].into_iter().collect(),
        ..Default::default()
    };
    assert!(fresh.carry_over(&old, &in_flight).is_empty());
    assert!(fresh.get_instance(&child).is_none());
}

#[test]
fn carry_over_keeps_a_property_change_on_its_way() {
    let mut old = DataModel::new();
    let ws = add(&mut old, "Workspace", "Workspace", None);
    let part = add(&mut old, "Part", "Box", Some(ws));
    old.get_mut_instance(&part)
        .unwrap()
        .properties
        .insert("Transparency".into(), PropertyValue::Number(0.5));

    let mut fresh = DataModel::new();
    let mut workspace = old.get_instance(&ws).unwrap().clone();
    workspace.children.clear();
    fresh.upsert_instance(workspace).unwrap();
    let mut stale = old.get_instance(&part).unwrap().clone();
    stale.properties.insert("Transparency".into(), PropertyValue::Number(0.0));
    fresh.upsert_instance(stale).unwrap();

    let in_flight = crate::transport::InFlight {
        properties: [(part, "Transparency".to_string())].into_iter().collect(),
        ..Default::default()
    };
    fresh.carry_over(&old, &in_flight);
    assert_eq!(
        fresh.get_instance(&part).unwrap().properties.get("Transparency"),
        Some(&PropertyValue::Number(0.5))
    );
}

/// Studio's tree still has the old name, the old parent and a part the core has
/// already deleted; all three changes are still on their way and must survive.
#[test]
fn carry_over_keeps_renames_moves_and_deletions_on_their_way() {
    let mut old = DataModel::new();
    let ws = add(&mut old, "Workspace", "Workspace", None);
    let rs = add(&mut old, "ReplicatedStorage", "ReplicatedStorage", None);
    let part = add(&mut old, "Part", "NewName", Some(rs));
    let gone = Uuid::new_v4();

    let mut fresh = DataModel::new();
    for id in [ws, rs] {
        let mut service = old.get_instance(&id).unwrap().clone();
        service.children.clear();
        fresh.upsert_instance(service).unwrap();
    }
    let mut stale = old.get_instance(&part).unwrap().clone();
    stale.name = "OldName".into();
    stale.parent = Some(ws);
    fresh.upsert_instance(stale).unwrap();
    let mut deleted = InstanceNode::new("Part", "Deleted");
    deleted.syncix_id = gone;
    deleted.parent = Some(ws);
    fresh.upsert_instance(deleted).unwrap();

    let in_flight = crate::transport::InFlight {
        properties: [(part, "Name".to_string())].into_iter().collect(),
        reparents: [part].into_iter().collect(),
        destroys: [gone].into_iter().collect(),
        ..Default::default()
    };
    fresh.carry_over(&old, &in_flight);

    let node = fresh.get_instance(&part).unwrap();
    assert_eq!(node.name, "NewName");
    assert_eq!(node.parent, Some(rs));
    assert!(fresh.get_instance(&rs).unwrap().children.contains(&part));
    assert!(!fresh.get_instance(&ws).unwrap().children.contains(&part));
    assert!(fresh.get_instance(&gone).is_none());
}

/// A stored children list can be stale (a folder restored from the trash): only
/// children that exist and point back are kept, and a child arriving later joins.
#[test]
fn upsert_drops_children_that_do_not_exist() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let mut folder = InstanceNode::new("Folder", "Obby");
    folder.parent = Some(ws);
    folder.children = vec![Uuid::new_v4(), Uuid::new_v4()];
    let folder_id = folder.syncix_id;
    m.upsert_instance(folder).unwrap();
    assert!(m.get_instance(&folder_id).unwrap().children.is_empty());

    let part = add(&mut m, "Part", "Part1", Some(folder_id));
    assert_eq!(m.get_instance(&folder_id).unwrap().children, vec![part]);
}

/// Writing a node again keeps the children the model knows of, and a new parent
/// takes it out of the old parent's list.
#[test]
fn upsert_keeps_children_and_leaves_old_parent() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let rs = add(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let folder = add(&mut m, "Folder", "Obby", Some(ws));
    let part = add(&mut m, "Part", "Part1", Some(folder));

    let mut again = m.get_instance(&folder).unwrap().clone();
    again.children.clear();
    again.parent = Some(rs);
    again.last_updated += 1;
    m.upsert_instance(again).unwrap();

    assert_eq!(m.get_instance(&folder).unwrap().children, vec![part]);
    assert!(!m.get_instance(&ws).unwrap().children.contains(&folder));
    assert!(m.get_instance(&rs).unwrap().children.contains(&folder));
}

/// Move: it must leave the old parent, join the new one, and the model must stay consistent.
#[test]
fn test_reparent_updates_both_parents() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let a = add(&mut m, "Folder", "A", Some(ws));
    let b = add(&mut m, "Folder", "B", Some(ws));
    let part = add(&mut m, "Part", "Box", Some(a));

    let (old, new) = m.reparent(&part, Some(b)).expect("reparent failed");
    assert_eq!(old, Some(a));
    assert_eq!(new, Some(b));
    assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
    assert!(!m.get_instance(&a).unwrap().children.contains(&part));
    assert!(m.get_instance(&b).unwrap().children.contains(&part));
    assert!(m.verify_consistency().is_ok(), "the model must stay consistent");
}

/// IDENTITY RULE: a UUID is generated only at CREATE time. A rename,
/// a move or a repeated FULL_SYNC must NEVER change it.
/// If this rule breaks, the two sides take the same object for two different objects and
/// sync silently duplicates it; that is why each case is tested separately.
#[test]
fn uuid_survives_rename() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let part = add(&mut m, "Part", "OldName", Some(ws));

    m.get_mut_instance(&part).unwrap().name = "NewName".to_string();

    assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
    match m.resolve_target("NewName") {
        ResolveResult::One(u) => assert_eq!(u, part, "the new name must resolve to the same UUID"),
        _ => panic!("renamed object not found"),
    }
    assert!(matches!(m.resolve_target("OldName"), ResolveResult::NotFound));
}

#[test]
fn uuid_survives_move() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let a = add(&mut m, "Folder", "A", Some(ws));
    let b = add(&mut m, "Folder", "B", Some(ws));
    let part = add(&mut m, "Part", "Box", Some(a));

    m.reparent(&part, Some(b)).expect("reparent failed");

    assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
    assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
}

/// FULL_SYNC resends the whole tree on every reconnect.
/// A node arriving with the same UUID must update the existing object, not create a new one.
#[test]
fn repeated_full_sync_does_not_duplicate() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let part = add(&mut m, "Part", "Box", Some(ws));
    let prior_count = m.get_instance(&ws).unwrap().children.len();

    // Studio reconnected: same UUID, arriving again with an updated name.
    let mut again = InstanceNode::new("Part", "BoxNewName");
    again.syncix_id = part;
    again.parent = Some(ws);
    m.upsert_instance(again).expect("repeat upsert failed");

    assert_eq!(
        m.get_instance(&ws).unwrap().children.len(),
        prior_count,
        "the same UUID must not create a second child"
    );
    assert_eq!(m.get_instance(&part).unwrap().name, "BoxNewName");
    assert!(m.verify_consistency().is_ok());
}

/// Target resolution: dotted path, short UUID and name.
#[test]
fn test_resolve_target_path_shortuuid_and_name() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let folder = add(&mut m, "Folder", "Decor", Some(ws));
    let part = add(&mut m, "Part", "Pillar", Some(folder));

    // By path
    match m.resolve_target("Workspace.Decor.Pillar") {
        ResolveResult::One(id) => assert_eq!(id, part),
        _ => panic!("path could not be resolved"),
    }
    // By short UUID
    let short = &part.to_string()[0..8];
    match m.resolve_target(short) {
        ResolveResult::One(id) => assert_eq!(id, part),
        _ => panic!("short uuid could not be resolved"),
    }
    // By name (single match)
    match m.resolve_target("Pillar") {
        ResolveResult::One(id) => assert_eq!(id, part),
        _ => panic!("the name could not be resolved"),
    }
    // Non-existent target
    assert!(matches!(m.resolve_target("NonExistent"), ResolveResult::NotFound));
}

/// With two siblings of the same name, name resolution must be ambiguous (never pick the wrong one).
#[test]
fn test_resolve_target_ambiguous_name() {
    let mut m = DataModel::new();
    let ws = add(&mut m, "Workspace", "Workspace", None);
    let _p1 = add(&mut m, "Part", "Box", Some(ws));
    let _p2 = add(&mut m, "Part", "Box", Some(ws));

    match m.resolve_target("Box") {
        ResolveResult::Ambiguous(list) => assert_eq!(list.len(), 2),
        _ => panic!("ambiguity should have been detected"),
    }
}

#[tokio::test]
async fn test_upsert_and_conflict_resolution() {
    let mut model = DataModel::new();
    let mut node = InstanceNode::new("Part", "TestPart");
    let id = node.syncix_id;

    // The first insert must succeed
    assert!(model.upsert_instance(node.clone()).is_ok());

    // Try an update with an older version (conflict)
    node.last_updated -= 1000;
    node.name = "OldName".to_string();
    let result = model.upsert_instance(node.clone());
    assert!(matches!(result, Err(ModelError::VersionConflict { .. })));

    // Check that the name did not change
    assert_eq!(model.get_instance(&id).unwrap().name, "TestPart");

    // Update with a newer version
    node.last_updated += 2000;
    node.name = "NewName".to_string();
    assert!(model.upsert_instance(node).is_ok());
    assert_eq!(model.get_instance(&id).unwrap().name, "NewName");
}

#[tokio::test]
async fn test_thread_safety() {
    let shared_model = create_shared_model();
    let node = InstanceNode::new("Part", "ConcurrentPart");
    let id = node.syncix_id;

    // Thread 1: inserts
    let model_clone1 = shared_model.clone();
    let node_clone = node.clone();
    let t1 = tokio::spawn(async move {
        let mut lock = model_clone1.write().await;
        lock.upsert_instance(node_clone).unwrap();
    });

    // Thread 2: reads (after waiting for thread 1 to finish)
    let model_clone2 = shared_model.clone();
    let t2 = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let lock = model_clone2.read().await;
        assert!(lock.get_instance(&id).is_some());
    });

    let _ = tokio::join!(t1, t2);
}
