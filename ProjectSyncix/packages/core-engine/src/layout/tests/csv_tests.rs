//! csv tests.

use crate::layout::*;
use crate::model::PropertyValue;

fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
    let mut n = InstanceNode::new(class, name);
    n.parent = parent;
    let id = n.syncix_id;
    m.upsert_instance(n).unwrap();
    id
}

#[test]
fn localizationtable_is_written_to_csv_file() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));

    let fs_path = data_file(&m, "src", &lt).unwrap();
    assert!(fs_path.to_string_lossy().ends_with("Translations.csv"), "{:?}", fs_path);
}

#[test]
fn csv_content_starts_with_header_row() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
    m.get_mut_instance(&lt).unwrap().properties.insert(
        "Contents".into(),
        PropertyValue::String(
            r#"[{"Key":"greeting","Source":"Hello","Context":"","Values":{"es":"Hola"}}]"#
                .into(),
        ),
    );

    let file_content = node_content(m.get_instance(&lt).unwrap());
    let line_list: Vec<&str> = file_content.lines().collect();
    assert_eq!(line_list[0], "Key,Source,Context,Example,es");
    assert!(line_list[1].contains("Hola"), "{}", file_content);
}

/// Contents lives in the .csv file, so it is NOT REPEATED in meta.
#[test]
fn contents_not_repeated_in_meta_file() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
    {
        let n = m.get_mut_instance(&lt).unwrap();
        n.properties
            .insert("Contents".into(), PropertyValue::String("[]".into()));
    }
    // With only Contents there is no need for a meta file
    assert!(meta_file(&m, "src", &lt).is_none());

    m.get_mut_instance(&lt)
        .unwrap()
        .attributes
        .insert("Version".into(), PropertyValue::Number(1.0));
    let file_content = meta_content(m.get_instance(&lt).unwrap());
    assert!(!file_content.contains("Contents"), "Contents must not be in meta: {}", file_content);
}

/// Corrupt Contents must not cause a crash; an empty file should be written.
#[test]
fn broken_contents_does_not_crash() {
    let mut m = DataModel::new();
    let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
    let lt = add_instance(&mut m, "LocalizationTable", "Translations", Some(rs));
    m.get_mut_instance(&lt)
        .unwrap()
        .properties
        .insert("Contents".into(), PropertyValue::String("{broken".into()));

    let file_content = node_content(m.get_instance(&lt).unwrap());
    assert!(file_content.is_empty());
}

#[test]
fn csv_is_managed_extension() {
    assert!(is_managed_file(Path::new("src/Translations.csv")));
}
