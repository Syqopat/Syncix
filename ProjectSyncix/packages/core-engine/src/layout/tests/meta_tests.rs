//! meta tests.

use crate::layout::*;
use crate::model::PropertyValue;

fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
    let mut n = InstanceNode::new(class, name);
    n.parent = parent;
    let id = n.syncix_id;
    m.upsert_instance(n).unwrap();
    id
}

/// NO meta file is produced for a script without properties.
/// Otherwise an empty .meta.json would pile up next to every script.
#[test]
fn script_without_properties_has_no_meta() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    assert!(meta_file(&m, "src", &s).is_none());
}

/// With a property or attribute, the meta file is created NEXT TO the script.
#[test]
fn meta_file_created_next_to_script_with_properties() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    m.get_mut_instance(&s)
        .unwrap()
        .properties
        .insert("Disabled".into(), PropertyValue::Boolean(true));

    let meta = meta_file(&m, "src", &s).expect("a meta file was expected");
    let script = data_file(&m, "src", &s).unwrap();
    assert_eq!(meta.parent(), script.parent(), "must be in the same folder");
    assert!(meta.to_string_lossy().ends_with("Main.meta.json"), "{:?}", meta);
}

#[test]
fn attribute_alone_produces_meta() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    m.get_mut_instance(&s)
        .unwrap()
        .attributes
        .insert("Version".into(), PropertyValue::Number(3.0));

    assert!(meta_file(&m, "src", &s).is_some());
}

/// A script with children becomes a container folder; its meta name becomes init.meta.json too.
#[test]
fn container_script_uses_init_meta() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    add_instance(&mut m, "ModuleScript", "Sub", Some(s));
    m.get_mut_instance(&s)
        .unwrap()
        .properties
        .insert("Disabled".into(), PropertyValue::Boolean(true));

    let meta = meta_file(&m, "src", &s).unwrap();
    assert!(meta.to_string_lossy().ends_with("init.meta.json"), "{:?}", meta);
}

/// NO meta is produced for non-script objects: their own .json file
/// already holds properties and attributes; a second file would be confusing.
#[test]
fn non_script_object_has_no_meta() {
    let mut m = DataModel::new();
    let ws = add_instance(&mut m, "Workspace", "Workspace", None);
    let p = add_instance(&mut m, "Part", "Box", Some(ws));
    m.get_mut_instance(&p)
        .unwrap()
        .properties
        .insert("Anchored".into(), PropertyValue::Boolean(true));

    assert!(meta_file(&m, "src", &p).is_none());
}

/// Meta content must be able to round-trip.
#[test]
fn meta_content_round_trip() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    {
        let n = m.get_mut_instance(&s).unwrap();
        n.properties.insert("Disabled".into(), PropertyValue::Boolean(true));
        n.properties.insert(
            "RunContext".into(),
            PropertyValue::String("Enum.RunContext.Server".into()),
        );
        n.attributes.insert("Version".into(), PropertyValue::Number(2.0));
    }

    let text_value = meta_content(m.get_instance(&s).unwrap());
    let restored_count: ScriptMeta = serde_json::from_str(&text_value).expect("could not parse");
    assert_eq!(restored_count.properties.len(), 2);
    assert_eq!(
        restored_count.properties.get("RunContext"),
        Some(&PropertyValue::String("Enum.RunContext.Server".into()))
    );
    assert_eq!(restored_count.attributes.get("Version"), Some(&PropertyValue::Number(2.0)));
}
