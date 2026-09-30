//! Syncix command-line interface.
//!
//! Why it lives inside the core binary:
//!  1. Portability. The old CLI was syncix.ps1; it depended on PowerShell and did not
//!     run on macOS. With one binary acting as both server and client, no extra runtime
//!     (PowerShell, Node) is needed.
//!  2. Correctness. The hex colour bug came precisely from the conversion logic living in
//!     the CLI and not in the core. Value parsing now lives in one place: parse_property_value.
//!
//! Usage: `syncix-core <command> [arguments]`. Run without arguments, it starts the server.


use crate::project::{DEFAULT_PORT, PORT_SCAN_SPAN};

mod args;
mod build;
mod discovery;
mod edit;
mod help;
mod http;
mod import;
mod inspect;
mod lifecycle;
mod style;
mod trash;

use args::*;
use build::*;
use discovery::*;
use edit::*;
use help::*;
use http::*;
use import::*;
use inspect::*;
use lifecycle::*;
use style::*;
use trash::*;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";


// ---------------------------------------------------------------------------
// Minimal HTTP client
//
// We only send JSON requests to 127.0.0.1; adding a full HTTP client library
// (reqwest + a TLS chain) for that would be needless weight.
// "Connection: close" is sent, so reading the response to the end of the stream is enough.
// ---------------------------------------------------------------------------

struct HttpReply {
    status_info: u16,
    body: String,
    /// Response headers (with lowercased names).
    /// Currently needed only for x-syncix-skipped-enums: publishing without knowing how many
    /// Enum values were skipped in the place file would mean sending
    /// an incomplete build to players.
    headers: std::collections::HashMap<String, String>,
}


// ---------------------------------------------------------------------------
// Core'u bulma
// ---------------------------------------------------------------------------


/// Every command name and alias, for "Did you mean" after an unknown one.
const COMMAND_NAMES: &[&str] = &[
    "help", "version", "status", "st", "tree", "ls", "list", "find", "search", "props", "show",
    "cat", "set", "attr", "new", "create", "mk", "rename", "rn", "rm", "del", "delete", "mv",
    "move", "upload", "sourcemap", "build", "import", "pull", "resync", "verify", "check", "tag",
    "tags", "bind", "config", "settings", "trash", "restore", "selftest", "init", "up", "start",
    "down", "stop", "serve",
];


enum PropertyCheck {
    Known,
    /// Close to a known property: refused.
    Misspelt(String),
    /// Unknown, and nothing is close: sent anyway, with a warning.
    Unlisted(String),
}


// ---------------------------------------------------------------------------
// Komutlar
// ---------------------------------------------------------------------------


struct TreeRow {
    id: String,
    item_name: String,
    class_str: String,
    parent_ref: Option<String>,
}


/// Flags of `trash`/`restore` that take a value.
const TRASH_VALUE_FLAGS: [&str; 3] = ["--in", "--class", "--since"];


// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Handles the arguments. Returns None when the server mode should run.
pub fn execute_run(cli_args: &[String]) -> Option<i32> {
    let Some(command_name) = cli_args.first().map(|s| s.as_str()) else {
        return None; // no arguments -> server mode
    };
    // `syncix serve` or `syncix serve 25565`: server mode.
    // A given port overrides the value in syncix.toml.
    if command_name == "serve" {
        return None;
    }

    let arg = |i: usize| cli_args.get(i).map(|s| s.as_str());
    // Values may contain spaces (e.g. `set Box Position 0, 5, -60`); all remaining
    // arguments are joined.
    let remaining = |i: usize| cli_args[i.min(cli_args.len())..].join(" ");
    if COMMAND_NAMES.contains(&command_name) && !flags_ok(cli_args) {
        return Some(1);
    }

    let outcome = match command_name {
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        "version" | "--version" | "-V" => {
            println!("syncix {}", crate::project::VERSION);
            0
        }
        "status" | "st" => status_info(),
        "tree" => tree(arg(1)),
        "ls" | "list" => list_instances(arg(1)),
        "find" | "search" => match arg(1) {
            Some(k) => search(k),
            None => {
                report_error("Usage: syncix find <word>");
                1
            }
        },
        "props" | "show" | "cat" => match arg(1) {
            Some(h) => property_list(h),
            None => {
                report_error("Usage: syncix props <target>");
                1
            }
        },
        "set" => match (arg(1), arg(2)) {
            (Some(h), Some(p)) if cli_args.len() > 3 => {
                let raw_value = remaining(3);
                let Some(port) = require_core() else { return Some(1) };
                // Checked before sending, with the parser the core uses and the property's
                // current value from the core (so a Beam's ColorSequence "Color" is not
                // judged like a Part's Color3). The core accepts the command and only logs
                // a refused colour, so without this `set Part Color "Really red"` printed
                // success here and nothing changed.
                // A missing target is left to the core's reply, which offers close ones.
                let object = fetch_json(port, &format!("/model/object?target={}", url_encode(h)))
                    .filter(|o| o.get("error").is_none());
                let class_name = object
                    .as_ref()
                    .and_then(|o| o.get("class_name")?.as_str().map(str::to_string));
                if let Some(o) = &object {
                    match property_check(o, p) {
                        PropertyCheck::Known => {}
                        PropertyCheck::Misspelt(message) => {
                            report_error(&format!("{}.{} was not changed: {}", h, p, message));
                            return Some(1);
                        }
                        PropertyCheck::Unlisted(message) => print_hint(&message),
                    }
                }
                let current = object
                    .as_ref()
                    .and_then(|o| {
                        o.get("properties")?
                            .as_object()?
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(p))
                            .map(|(_, v)| v.clone())
                    })
                    .and_then(|v| serde_json::from_value::<crate::model::PropertyValue>(v).ok());
                if let Err(reason) = crate::values::value_for_class(class_name.as_deref(), p, current.as_ref(), &raw_value) {
                    report_error(&format!("{}.{} was not changed: {}", h, p, reason));
                    if crate::values::color_target(p, current.as_ref()).is_some() {
                        for line in crate::values::color_forms_help() {
                            print_dim(&format!("  {}", line));
                        }
                        if p.eq_ignore_ascii_case("Color") {
                            print_dim("  A BrickColor name such as \"Really red\" belongs to the BrickColor property.");
                        }
                    }
                    return Some(1);
                }
                if send_command(
                    port,
                    "SET_PROPERTY",
                    serde_json::json!({ "id": h, "property": p, "value": raw_value }),
                ) {
                    print_ok(&format!("{}.{} = {}", h, p, raw_value));
                    0
                } else {
                    1
                }
            }
            _ => {
                report_error("Usage: syncix set <target> <property> <value>");
                1
            }
        },
        "attr" => match (arg(1), arg(2)) {
            // Delete: `syncix attr <target> <name> --delete`
            (Some(h), Some(n)) if arg(3) == Some("--delete") => {
                let Some(port) = require_core() else { return Some(1) };
                if send_command(
                    port,
                    "SET_ATTRIBUTE",
                    serde_json::json!({ "id": h, "name": n, "value": serde_json::Value::Null }),
                ) {
                    print_ok(&format!("{} @{} removed", h, n));
                    0
                } else {
                    1
                }
            }
            (Some(h), Some(n)) if cli_args.len() > 3 => {
                let Some(port) = require_core() else { return Some(1) };
                let raw_value = remaining(3);
                if send_command(
                    port,
                    "SET_ATTRIBUTE",
                    serde_json::json!({ "id": h, "name": n, "value": raw_value }),
                ) {
                    print_ok(&format!("{} @{} = {}", h, n, raw_value));
                    0
                } else {
                    1
                }
            }
            _ => {
                report_error("Usage: syncix attr <target> <name> <value>");
                1
            }
        },
        "new" | "create" | "mk" => match arg(1) {
            Some(typed_class) => {
                let class_str = match class_for_new(typed_class) {
                    Ok(c) => c,
                    Err(message) => {
                        report_error(&message);
                        return Some(1);
                    }
                };
                if crate::rbxmx_import::is_singleton(&class_str) {
                    // The engine refuses it too; saying so here beats a "Created" that did nothing.
                    report_error(&format!("{} exists once per place and cannot be created.", class_str));
                    return Some(1);
                }
                let Some(port) = require_core() else { return Some(1) };
                let item_name = arg(2).unwrap_or(&class_str);
                let parent_ref = arg(3).unwrap_or("Workspace");
                if send_command(
                    port,
                    "CREATE_INSTANCE",
                    serde_json::json!({ "className": class_str, "name": item_name, "parentId": parent_ref }),
                ) {
                    print_ok(&format!("Created {} ({}) in {}", item_name, class_str, parent_ref));
                    0
                } else {
                    1
                }
            }
            None => {
                report_error("Usage: syncix new <class> [name] [parent]");
                1
            }
        },
        "rename" | "rn" => match (arg(1), arg(2)) {
            (Some(h), Some(fresh)) => {
                let Some(port) = require_core() else { return Some(1) };
                if send_command(
                    port,
                    "RENAME_INSTANCE",
                    serde_json::json!({ "id": h, "newName": fresh }),
                ) {
                    print_ok(&format!("{} -> {}", h, fresh));
                    0
                } else {
                    1
                }
            }
            _ => {
                report_error("Usage: syncix rename <target> <new name>");
                1
            }
        },
        "rm" | "del" | "delete" => match arg(1) {
            Some(h) => {
                let Some(port) = require_core() else { return Some(1) };
                // Deletion takes the children along, and although Studio can undo it,
                // the editor side has no way back. So what would be deleted is SHOWN first and
                // confirmation is asked; --yes skips this step in scripts.
                let confirmed = cli_args.iter().any(|a| a == "--yes" || a == "-y")
                    || !crate::project::ProjectConfig::load().safety_settings.confirm_delete;
                if !confirmed && !confirm_delete(port, h) {
                    print_info("Cancelled; nothing was deleted.");
                    return Some(0);
                }
                if send_command(port, "DELETE_INSTANCE", serde_json::json!({ "id": h })) {
                    print_ok(&format!("{} deleted", h));
                    0
                } else {
                    1
                }
            }
            None => {
                report_error("Usage: syncix rm <target> [--yes]");
                1
            }
        },
        "mv" | "move" => match (arg(1), arg(2)) {
            (Some(h), Some(fresh)) => {
                let Some(port) = require_core() else { return Some(1) };
                if send_command(
                    port,
                    "REPARENT_INSTANCE",
                    serde_json::json!({ "id": h, "newParentId": fresh }),
                ) {
                    print_ok(&format!("Moved {} into {}", h, fresh));
                    0
                } else {
                    1
                }
            }
            _ => {
                report_error("Usage: syncix mv <target> <new parent>");
                1
            }
        },
        "upload" => publish_place(cli_args),
        "sourcemap" => build_sourcemap(cli_args),
        "build" => run_build(cli_args),
        "import" => import_target(cli_args),
        "pull" | "resync" => {
            let Some(port) = require_core() else { return Some(1) };
            if send_command(port, "FULL_SYNC", serde_json::json!({})) {
                print_ok("Asked Studio to resend the tree.");
                print_dim("  The reply is processed within a few seconds; then try syncix tree.");
                0
            } else {
                1
            }
        }
        "verify" | "check" => verify_input(),
        "tag" | "tags" => match arg(1) {
            Some(h) => {
                let Some(port) = require_core() else { return Some(1) };
                // A call without arguments only lists: "empty list" and "listing" are kept
                // apart so tags are not deleted by accident.
                if cli_args.len() <= 2 {
                    show_tags(port, h)
                } else {
                    // "--none" on its own means "clear all". There is no other way to send an
                    // empty list: a call without arguments means
                    // listing.
                    let tag_list: Vec<String> = if cli_args[2..] == ["--none".to_string()] {
                        Vec::new()
                    } else {
                        cli_args[2..].iter().map(|s| s.to_string()).collect()
                    };
                    if send_command(
                        port,
                        "SET_TAGS",
                        serde_json::json!({ "id": h, "tags": tag_list }),
                    ) {
                        if tag_list.is_empty() {
                            print_ok(&format!("{} tags cleared", h));
                        } else {
                            print_ok(&format!("{} tags: {}", h, tag_list.join(", ")));
                        }
                        0
                    } else {
                        1
                    }
                }
            }
            None => {
                report_error("Usage: syncix tag <target> [tag ...]   (no tags = show)");
                print_dim("  Clear every tag with: syncix tag <target> --none");
                1
            }
        },
        "bind" => bind_cmd(cli_args),
        "config" | "settings" => show_config(),
        "trash" => trash_list(cli_args),
        "restore" => trash_restore(cli_args),
        "selftest" => selftest(),
        "init" => init_project(),
        "up" | "start" => launch(arg(1).and_then(|p| p.parse::<u16>().ok())),
        "down" | "stop" => stop_core(),
        unknown => {
            report_error(&format!("Unknown command: {}", unknown));
            if let Some(hint) = crate::suggest::hint(unknown, COMMAND_NAMES.iter().copied()) {
                print_hint(&hint);
            }
            print_dim("  Run syncix help to see the command list.");
            1
        }
    };

    Some(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encoding_of_spaces_and_dot_paths() {
        assert_eq!(url_encode("Workspace.Simulator"), "Workspace.Simulator");
        assert_eq!(url_encode("two words"), "two%20words");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn missing_port_file_does_not_crash() {
        // Only checks that it does not panic; it may return Some/None depending on the environment.
        let _ = from_port_file();
    }

    #[test]
    fn find_node_by_name_and_short_uuid() {
        let node_list = vec![
            TreeRow {
                id: "aabbccdd-1111-2222-3333-444455556666".into(),
                item_name: "Box".into(),
                class_str: "Part".into(),
                parent_ref: None,
            },
        ];
        assert!(find_node(&node_list, "Box").is_some());
        assert!(find_node(&node_list, "box").is_some());
        assert!(find_node(&node_list, "aabbccdd").is_some());
        assert!(find_node(&node_list, "missing").is_none());
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;

    #[test]
    fn headers_are_lowercased() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nX-Syncix-Skipped-Enums: 3";
        let h = parse_headers(raw);
        assert_eq!(h.get("content-type").unwrap(), "text/xml");
        assert_eq!(h.get("x-syncix-skipped-enums").unwrap(), "3");
    }

    /// The status line is not a header; if it got into the map by mistake
    /// a meaningless key like "http/1.1 200 ok" would appear.
    #[test]
    fn status_line_is_not_a_header() {
        let h = parse_headers("HTTP/1.1 404 Not Found\r\nX-A: 1");
        assert_eq!(h.len(), 1);
        assert!(h.contains_key("x-a"));
    }

    #[test]
    fn colon_in_value_is_kept() {
        let h = parse_headers("HTTP/1.1 200 OK\r\nLocation: https://a.b/c:1");
        assert_eq!(h.get("location").unwrap(), "https://a.b/c:1");
    }

    #[test]
    fn no_headers_gives_empty_map() {
        assert!(parse_headers("HTTP/1.1 200 OK").is_empty());
        assert!(parse_headers("").is_empty());
    }
}
