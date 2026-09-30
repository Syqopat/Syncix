//! txt tests.

use crate::layout::*;
use crate::model::PropertyValue;

fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
    let mut n = InstanceNode::new(class, name);
    n.parent = parent;
    let id = n.syncix_id;
    m.upsert_instance(n).unwrap();
    id
}

/// A StringValue is written to disk as .txt, not as .json.
#[test]
fn stringvalue_is_written_as_txt() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
    m.get_mut_instance(&sv)
        .unwrap()
        .properties
        .insert("Value".into(), PropertyValue::String("hello".into()));

    let fs_path = data_file(&m, "src", &sv).unwrap();
    assert!(fs_path.to_string_lossy().ends_with("Message.txt"), "{:?}", fs_path);
}

/// The file's content is the Value itself; no JSON wrapper.
#[test]
fn txt_content_is_the_value() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
    m.get_mut_instance(&sv)
        .unwrap()
        .properties
        .insert("Value".into(), PropertyValue::String("line1\nline2".into()));

    let file_content = node_content(m.get_instance(&sv).unwrap());
    assert_eq!(file_content, "line1\nline2");
}

/// The Value lives in the .txt file, so it is NOT REPEATED in the meta file;
/// keeping it in two places risks the two drifting apart.
#[test]
fn value_not_repeated_in_meta_file() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let sv = add_instance(&mut m, "StringValue", "Message", Some(rs));
    m.get_mut_instance(&sv)
        .unwrap()
        .properties
        .insert("Value".into(), PropertyValue::String("hello".into()));

    // With only a Value there is no need for a meta file
    assert!(meta_file(&m, "src", &sv).is_none());

    // Adding an attribute creates a meta file, but it does not contain Value
    m.get_mut_instance(&sv)
        .unwrap()
        .attributes
        .insert("Dil".into(), PropertyValue::String("es".into()));
    assert!(meta_file(&m, "src", &sv).is_some());

    let file_content = meta_content(m.get_instance(&sv).unwrap());
    assert!(!file_content.contains("Value"), "Value must not be in meta: {}", file_content);
    assert!(file_content.contains("Dil"));
}

/// In scripts Source is a separate field, so properties stay in meta.
#[test]
fn script_properties_stay_in_meta() {
    let mut m = DataModel::new();
    let sss = add_instance(&mut m, "ServerScriptService", "ServerScriptService", None);
    let s = add_instance(&mut m, "Script", "Main", Some(sss));
    m.get_mut_instance(&s)
        .unwrap()
        .properties
        .insert("Disabled".into(), PropertyValue::Boolean(true));

    let file_content = meta_content(m.get_instance(&s).unwrap());
    assert!(file_content.contains("Disabled"));
}

/// ValueBase classes other than StringValue still use .json:
/// turning a numeric value into plain text would lose its type.
#[test]
fn intvalue_stays_json() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let iv = add_instance(&mut m, "IntValue", "Sayac", Some(rs));

    let fs_path = data_file(&m, "src", &iv).unwrap();
    assert!(fs_path.to_string_lossy().ends_with("Sayac.intvalue.json"), "{:?}", fs_path);
}

#[test]
fn object_extension_includes_class_name() {
    let mut m = DataModel::new();
    let ws = add_instance(&mut m, "Workspace", "Workspace", None);
    let p = add_instance(&mut m, "Part", "Box", Some(ws));
    let mdl = add_instance(&mut m, "Model", "House", Some(ws));
    add_instance(&mut m, "Part", "Roof", Some(mdl));

    let p_path = data_file(&m, "src", &p).unwrap();
    assert!(p_path.to_string_lossy().ends_with("Box.part.json"), "{:?}", p_path);

    let mdl_path = data_file(&m, "src", &mdl).unwrap();
    assert!(mdl_path.to_string_lossy().ends_with("init.model.json"), "{:?}", mdl_path);

    assert!(is_managed_file(Path::new("src/Box.part.json")));
    assert!(is_managed_file(Path::new("src/House/init.model.json")));
}
