//! Split out of file_sync.rs.

#[allow(unused_imports)]
use super::*;

/// Applies a `Name.txt` file to the Value of the matching StringValue.
///
/// A StringValue is written to disk as plain text (see layout::script_ext), so
/// the file's content is the Value itself. That lets the text be opened and edited
/// in the editor without dealing with JSON escape characters.
pub(crate) fn handle_txt_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(file_content) = fs::read_to_string(path) else {
        return;
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "txt", "StringValue")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the txt file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(file_content.clone());
                if node.properties.get("Value") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Value".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Value", "value": file_content }
                }]
            }),
        });
        debug!("Updated Value from the txt file: {:?}", path);
    }
}

/// Applies a `Name.csv` file to the Contents of the matching LocalizationTable.
///
/// Translations are written to disk as CSV so they can be edited as a table; on the
/// Roblox side the counterpart is a JSON string called Contents. The conversion is lossless (localization.rs).
pub(crate) fn handle_csv_file(
    path: &Path,
    tx_to_studio: &StudioOutbox,
    data_model: &crate::model::SharedDataModel,
) {
    let Ok(csv) = fs::read_to_string(path) else {
        return;
    };

    let json = match crate::localization::csv_to_json(&csv) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Could not parse CSV ({:?}): {}", path, e);
            return;
        }
    };

    let uuid = {
        let dm = data_model.blocking_read();
        raw_file_target(&dm, path, "csv", "LocalizationTable")
    };
    let Some(uuid) = uuid else {
        debug!("Could not find the owner of the csv file: {:?}", path);
        return;
    };

    let changed = {
        let mut dm = data_model.blocking_write();
        match dm.get_mut_instance(&uuid) {
            Some(node) => {
                let fresh = crate::model::PropertyValue::String(json.clone());
                if node.properties.get("Contents") == Some(&fresh) {
                    false
                } else {
                    node.properties.insert("Contents".to_string(), fresh);
                    true
                }
            }
            None => false,
        }
    };

    if changed {
        tx_to_studio.push(Payload {
            version: "v1".to_string(),
            event_type: EventType::CompositeUpdate,
            data: serde_json::json!({
                "patches": [{
                    "event_type": "PROPERTY_UPDATE",
                    "data": { "syncix_id": uuid, "property": "Contents", "value": json }
                }]
            }),
        });
        debug!("Updated Contents from the csv file: {:?}", path);
    }
}
