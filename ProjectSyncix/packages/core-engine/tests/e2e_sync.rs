//! The whole round trip, with the test playing the Studio plugin.
//!
//! What a unit test cannot show: that a full sync really lands on disk, that an edit on
//! disk really leaves for Studio, and that the core says no when it has to.

mod support;

use std::time::Duration;
use support::{full_sync_payload, TestCore};

/// The happy path: Studio connects, its tree is written to disk, an edit on disk goes
/// back to Studio.
#[test]
fn a_full_sync_lands_on_disk_and_a_disk_edit_leaves_for_studio() {
    let core = TestCore::start("happy", "");
    let service = uuid::Uuid::new_v4().to_string();
    let script = uuid::Uuid::new_v4().to_string();

    let pushed = core.push(
        "FULL_SYNC",
        full_sync_payload(&service, &script, "print('from studio')"),
    );
    assert!((200..300).contains(&pushed.status), "{}", pushed.body);

    let written = core
        .wait_for_file("ServerScriptService/Main.server.lua")
        .expect("the script should be written to disk");
    assert!(written.contains("from studio"), "{}", written);

    // The tree the editor reads is the same tree.
    let tree = core.get("/model/tree").json();
    let names: Vec<&str> = tree
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(names.contains(&"Main"), "{:?}", names);

    // Now the other direction: the file is edited the way an editor saves it.
    std::fs::write(
        core.sync_dir().join("ServerScriptService").join("Main.server.lua"),
        "print('edited on disk')",
    )
    .unwrap();

    let message = core
        .poll(Duration::from_secs(20))
        .expect("the edit should reach the plugin");
    let text = message.to_string();
    assert!(
        text.contains("edited on disk"),
        "the poll reply should carry the new source: {}",
        text
    );
}

/// A sync folder belongs to ONE place. Two places sharing a folder used to merge
/// silently and duplicate every singleton service.
#[test]
fn another_place_suspends_sync_instead_of_merging() {
    let core = TestCore::start("place", "");
    let service = uuid::Uuid::new_v4().to_string();
    let script = uuid::Uuid::new_v4().to_string();
    core.push("FULL_SYNC", full_sync_payload(&service, &script, "print('first')"));
    core.wait_for_file("ServerScriptService/Main.server.lua")
        .expect("the first place's work should be on disk before the second connects");

    core.push(
        "FULL_SYNC",
        serde_json::json!({
            "place_key": "a-different-place",
            "place_name": "Other",
            "place_id": "123",
            "instances": []
        }),
    );

    let health = core.wait_for_health(|h| h["sync_suspended"] == true);
    assert_eq!(health["sync_suspended"], true, "{}", health);
    assert_eq!(health["place_conflict"]["incoming_name"], "Other");
    // The first place's work is untouched.
    assert!(core
        .sync_dir()
        .join("ServerScriptService/Main.server.lua")
        .exists());
}

/// Regression test for the import that reported success while Studio created nothing.
/// Without a connected Studio the import must refuse, not queue the whole tree.
#[test]
fn an_import_refuses_to_run_without_studio() {
    let core = TestCore::start("import-no-studio", "");
    let model = core.root.join("model");
    std::fs::create_dir_all(&model).unwrap();
    std::fs::write(
        model.join("Box.part.json"),
        serde_json::json!({ "class_name": "Part", "name": "Box" }).to_string(),
    )
    .unwrap();

    let (code, output) = core.cli(&["import", model.to_str().unwrap(), "Workspace"]);
    assert_eq!(code, 1, "{}", output);
    assert!(
        output.contains("Studio is not connected"),
        "the reason has to be said: {}",
        output
    );
}

/// An object that changes every frame is throttled, and the flood count reaches
/// /health so both panels and the CLI can show it.
#[test]
fn a_flood_reported_by_the_plugin_reaches_health() {
    let core = TestCore::start("flood", "");
    core.push(
        "PLUGIN_METRICS",
        serde_json::json!({
            "queued": 4000,
            "coalesced": 3900,
            "floods": 2,
            "plugin_version": env!("CARGO_PKG_VERSION"),
            "activity_total": 10,
            "activity_in": 4,
            "activity_out": 6,
            "conflicts": 0
        }),
    );

    let health = core.wait_for_health(|h| h["plugin_floods"].as_u64().unwrap_or(0) == 2);
    assert_eq!(health["plugin_floods"], 2, "{}", health);
    assert_eq!(health["plugin_coalesced"], 3900);

    let (code, output) = core.cli(&["status"]);
    assert_eq!(code, 0, "{}", output);
    assert!(
        output.contains("flooding objects"),
        "syncix status should name the flood: {}",
        output
    );
}
