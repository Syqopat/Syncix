//! The HTTP surface, checked against a running core.
//!
//! Every one of these was a real inconsistency: a missing token was answered 200 with an
//! error body, a target that did not exist was 200 too, and the shapes of error replies
//! differed per route. A client could not tell success from failure by the status code.

mod support;

use support::TestCore;

#[test]
fn health_needs_no_token_and_says_what_this_core_is() {
    let core = TestCore::start("health", "");
    let reply = core.get_without_token("/health");
    assert_eq!(reply.status, 200);

    let body = reply.json();
    assert_eq!(body["status"], "Healthy");
    assert_eq!(body["port"], core.port);
    assert!(body["version"].is_string());
    assert!(body["token"].is_string(), "the plugin reads the token here");
    // The project's path carried the user's account name and is not reported any more.
    assert!(body.get("root").is_none(), "/health must not report the path");
    assert!(body["jobs"].is_object(), "the job pool is reported");
}

#[test]
fn every_other_route_refuses_a_request_without_the_token() {
    let core = TestCore::start("token", "");
    for path in ["/model/tree", "/model/verify", "/model/sourcemap", "/model/export"] {
        let reply = core.get_without_token(path);
        assert_eq!(reply.status, 401, "{} must need the token", path);
        assert_eq!(reply.json()["error"], "unauthorized");
    }
    let refused = core.post_without_token("/commands", serde_json::json!({ "event_type": "PULL" }));
    assert_eq!(refused.status, 401);

    // Stopping the core is the one that mattered most: /shutdown used to take any
    // request at all, so any web page could end the user's sync.
    let stop = core.post_without_token("/core/stop", serde_json::json!({}));
    assert_eq!(stop.status, 401);
    assert_eq!(core.get("/health").status, 200, "the core is still running");
}

#[test]
fn the_token_opens_the_protected_routes() {
    let core = TestCore::start("token-ok", "");
    assert_eq!(core.get("/model/tree").status, 200);
    assert_eq!(core.get("/model/verify").json()["ok"], true);
}

#[test]
fn a_status_code_says_what_went_wrong() {
    let core = TestCore::start("codes", "");

    let unknown_event = core.post("/commands", serde_json::json!({ "event_type": "NOPE" }));
    assert_eq!(unknown_event.status, 400);
    assert_eq!(unknown_event.json()["error"], "bad_request");

    let not_an_object = core.post(
        "/commands",
        serde_json::json!({ "event_type": "RENAME_INSTANCE", "data": 7 }),
    );
    assert_eq!(not_an_object.status, 400);

    let missing = core.get("/model/object?target=NoSuchThing");
    assert_eq!(missing.status, 404);
    assert_eq!(missing.json()["error"], "not_found");

    // Accepted work answers 202, not 200: the core has taken it, Studio has not done it yet.
    let accepted = core.post("/commands", serde_json::json!({ "event_type": "PULL" }));
    assert_eq!(accepted.status, 202);
}

#[test]
fn two_objects_of_one_name_are_a_conflict_not_a_guess() {
    let core = TestCore::start("ambiguous", "");
    let service = uuid::Uuid::new_v4().to_string();
    let first = uuid::Uuid::new_v4().to_string();
    let second = uuid::Uuid::new_v4().to_string();

    core.push(
        "FULL_SYNC",
        serde_json::json!({
            "place_key": "test-place",
            "instances": [
                { "syncix_id": service, "class_name": "Workspace", "name": "Workspace", "parent": null },
                { "syncix_id": first, "class_name": "Part", "name": "Twin", "parent": service },
                { "syncix_id": second, "class_name": "Part", "name": "Twin", "parent": service },
            ]
        }),
    );
    core.wait_for_health(|h| h["object_count"].as_u64().unwrap_or(0) >= 3);

    let reply = core.get("/model/object?target=Twin");
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(reply.json()["error"], "conflict");
}

#[test]
fn the_api_describes_itself() {
    let core = TestCore::start("openapi", "");
    let spec = core.get_without_token("/openapi.json");
    assert_eq!(spec.status, 200);

    let body = spec.json();
    assert_eq!(body["openapi"].as_str().map(|v| &v[..1]), Some("3"));
    for path in ["/health", "/sync/poll", "/commands", "/model/tree", "/core/stop"] {
        assert!(body["paths"][path].is_object(), "{} missing from the spec", path);
    }

    let docs = core.get_without_token("/docs/");
    assert!(
        docs.status == 200 || docs.status == 301 || docs.status == 302,
        "the docs page should be served, got {}",
        docs.status
    );
}
