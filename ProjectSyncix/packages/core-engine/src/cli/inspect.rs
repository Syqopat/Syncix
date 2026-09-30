//! Commands that only read: status, tree, ls, find, props, check, tags.


use super::*;

/// "Not found", in the core's words when it can say more: it offers the closest targets.
pub(crate) fn report_not_found(port: u16, dest: &str) {
    let reason = fetch_json(port, &format!("/model/object?target={}", url_encode(dest)))
        .and_then(|o| o.get("error")?.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("Not found: {}", dest));
    report_error(&reason);
}

/// The class `syncix new` creates. A wrong case is corrected (Instance.new is
/// case-sensitive, so "part" failed in Studio after "Created" was printed); a slip close to
/// a known class is refused with it; a name the table does not know at all is passed on,
/// since the table may be older than Studio.
pub(crate) fn class_for_new(typed: &str) -> Result<String, String> {
    if crate::catalog::is_class(typed) {
        return Ok(typed.to_string());
    }
    if let Some(real) = crate::catalog::class_named(typed) {
        print_dim(&format!("  Using {} (class names are case-sensitive).", real));
        return Ok(real.to_string());
    }
    match crate::suggest::hint(typed, crate::catalog::class_names()) {
        Some(hint) => Err(format!("Unknown class: {}. {}", typed, hint)),
        None => Ok(typed.to_string()),
    }
}

/// Whether the instance has the property. `set Box Szie 4,1,2` reached Studio, which
/// refused it, while the core had already stored a property "Szie" beside Size. A name
/// close to a known one is refused; one nothing is close to is let through, because the
/// table behind the check lists only what Syncix carries and may be older than Studio.
pub(crate) fn property_check(object: &serde_json::Value, property: &str) -> PropertyCheck {
    let class = object.get("class_name").and_then(|c| c.as_str()).unwrap_or("");
    let listed = crate::catalog::properties_of(class);
    let mut known: Vec<&str> = object
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    known.extend(listed.iter().flatten().copied());
    // Members the table leaves out on purpose (see UNLISTED_MEMBERS in PatchExecutor).
    known.extend(["Name", "Parent", "Archivable", "Source"]);
    if known.iter().any(|k| k.eq_ignore_ascii_case(property)) {
        return PropertyCheck::Known;
    }
    if let Some(hint) = crate::suggest::hint(property, known.iter().copied()) {
        return PropertyCheck::Misspelt(format!("{} has no property '{}'. {}", class, property, hint));
    }
    match listed {
        Some(_) => PropertyCheck::Unlisted(format!(
            "{} has no property '{}' that Syncix knows; sending it anyway. Studio will refuse it if it does not exist.",
            class, property
        )),
        None => PropertyCheck::Known,
    }
}

/// Files beside a path that does not exist, spelt closest to it.
pub(crate) fn file_hint(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    let typed = path.file_name()?.to_str()?;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    let names: Vec<String> = std::fs::read_dir(dir.unwrap_or(std::path::Path::new(".")))
        .ok()?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    let options: Vec<String> = crate::suggest::closest(typed, names.iter().map(String::as_str))
        .into_iter()
        .map(|name| match dir {
            Some(d) => d.join(name).display().to_string(),
            None => name.to_string(),
        })
        .collect();
    crate::suggest::did_you_mean(&options)
}

pub(crate) fn status_info() -> i32 {
    let Some(port) = core_port() else {
        report_error("Syncix Core is not running.");
        print_dim("  Start it with: syncix up");
        return 1;
    };
    let Some(h) = fetch_json(port, "/health") else {
        report_error("Could not read health info.");
        return 1;
    };

    let al = |k: &str| h.get(k).cloned().unwrap_or(serde_json::Value::Null);
    let text_value = |k: &str| al(k).as_str().unwrap_or("-").to_string();
    let number_value = |k: &str| al(k).as_u64().unwrap_or(0);

    print_info("Syncix Core");
    println!("  version      : {} (protocol {})", text_value("version"), number_value("protocol"));
    println!("  project      : {}", text_value("project"));
    println!("  folder       : {}", crate::project::ProjectConfig::load().root.display());
    println!("  port         : {}", number_value("port"));
    println!("  uptime       : {} seconds", number_value("uptime_seconds"));

    let studio = al("studio_connected").as_bool().unwrap_or(false);
    if studio {
        print_ok("  Studio       : connected");
    } else {
        println!("{}  Studio       : not connected{}", YELLOW, RESET);
        print_dim("    (Is Studio open? The plugin may be waiting on the approval dialog.)");
    }

    print_info("Metrics");
    println!("  inbound from Studio : {}", number_value("inbound_from_studio"));
    println!("  outbound to Studio  : {}", number_value("outbound_to_studio"));
    println!("  plugin queued       : {}", number_value("plugin_queued"));
    println!("  coalesced           : {}", number_value("plugin_coalesced"));

    print_info("Activity log (Studio plugin)");
    println!("  entries        : {} (in {}, out {})",
        number_value("activity_total"), number_value("activity_in"), number_value("activity_out"));
    let conflict = number_value("conflicts");
    if conflict > 0 {
        println!("{}  conflicts      : {} — see the 'Recent changes' tab in the Studio panel{}",
            YELLOW, conflict, RESET);
    } else {
        println!("  conflicts      : 0");
    }

    let cycle = number_value("loops_detected");
    if cycle > 0 {
        println!("{}  echo loops          : {} (check the log file){}", YELLOW, cycle, RESET);
    } else {
        println!("  echo loops          : 0");
    }
    0
}

pub(crate) fn fetch_tree(port: u16) -> Option<Vec<TreeRow>> {
    let v = fetch_json(port, "/model/tree")?;
    let array_value = v.as_array()?;
    Some(
        array_value.iter()
            .map(|n| TreeRow {
                id: n.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                item_name: n.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                class_str: n
                    .get("className")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                parent_ref: n
                    .get("parentId")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
            })
            .collect(),
    )
}

pub(crate) fn print_tree(node_list: &[TreeRow], root_dir: Option<&str>, prefix: &str, depth: usize) {
    if depth > 20 {
        return;
    }
    let mut child_entries: Vec<&TreeRow> = node_list
        .iter()
        .filter(|d| d.parent_ref.as_deref() == root_dir)
        .collect();
    child_entries.sort_by(|a, b| a.item_name.cmp(&b.item_name));

    for (i, c) in child_entries.iter().enumerate() {
        let last_item = i == child_entries.len() - 1;
        let branch = if last_item { "└─ " } else { "├─ " };
        println!(
            "{}{}{}  {}{}{}",
            prefix,
            branch,
            c.item_name,
            DIM,
            c.class_str,
            RESET
        );
        let sub_prefix = format!("{}{}", prefix, if last_item { "   " } else { "│  " });
        print_tree(node_list, Some(&c.id), &sub_prefix, depth + 1);
    }
}

pub(crate) fn tree(dest: Option<&str>) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(node_list) = fetch_tree(port) else {
        report_error("Could not read the tree.");
        return 1;
    };
    if node_list.is_empty() {
        println!("{}Tree is empty. Is Studio connected?{}", YELLOW, RESET);
        return 0;
    }

    match dest {
        None => print_tree(&node_list, None, "", 0),
        Some(h) => {
            let Some(d) = resolve_row(port, &node_list, h) else {
                report_not_found(port, h);
                return 1;
            };
            println!("{}  {}{}{}", d.item_name, DIM, d.class_str, RESET);
            print_tree(&node_list, Some(&d.id), "", 0);
        }
    }
    0
}

/// Finds a row by id, short id or name, and otherwise asks the core, which also
/// understands dotted paths ("Workspace.Tycoons"). Matching names locally never could,
/// so `syncix ls Workspace.Tycoons` said "Not found" while `syncix set` found it.
pub(crate) fn resolve_row<'a>(port: u16, node_list: &'a [TreeRow], dest: &str) -> Option<&'a TreeRow> {
    if let Some(row) = find_node(node_list, dest) {
        return Some(row);
    }
    let object = fetch_json(port, &format!("/model/object?target={}", url_encode(dest)))?;
    let id = object.get("syncix_id")?.as_str()?;
    node_list.iter().find(|row| row.id == id)
}

pub(crate) fn find_node<'a>(node_list: &'a [TreeRow], dest: &str) -> Option<&'a TreeRow> {
    let h = dest.to_lowercase();
    node_list
        .iter()
        .find(|d| d.id == dest)
        .or_else(|| node_list.iter().find(|d| d.id.starts_with(&h)))
        .or_else(|| node_list.iter().find(|d| d.item_name.to_lowercase() == h))
}

pub(crate) fn list_instances(dest: Option<&str>) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(node_list) = fetch_tree(port) else { return 1 };

    let root_dir: Option<String> = match dest {
        None => None,
        Some(h) => match resolve_row(port, &node_list, h) {
            Some(d) => Some(d.id.clone()),
            None => {
                report_not_found(port, h);
                return 1;
            }
        },
    };

    let mut child_entries: Vec<&TreeRow> = node_list
        .iter()
        .filter(|d| d.parent_ref.as_deref() == root_dir.as_deref())
        .collect();
    child_entries.sort_by(|a, b| a.item_name.cmp(&b.item_name));

    if child_entries.is_empty() {
        print_dim("(no children)");
        return 0;
    }
    for c in child_entries {
        println!(
            "  {}{}{}  {:<22} {}",
            DIM,
            &c.id[..8.min(c.id.len())],
            RESET,
            c.item_name,
            c.class_str
        );
    }
    0
}

pub(crate) fn search(word: &str) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(node_list) = fetch_tree(port) else { return 1 };
    let k = word.to_lowercase();

    let mut found_item: Vec<&TreeRow> = node_list
        .iter()
        .filter(|d| d.item_name.to_lowercase().contains(&k) || d.class_str.to_lowercase().contains(&k))
        .collect();
    found_item.sort_by(|a, b| a.item_name.cmp(&b.item_name));

    if found_item.is_empty() {
        println!("{}No match: {}{}", YELLOW, word, RESET);
        let mut known: Vec<&str> = node_list
            .iter()
            .flat_map(|d| [d.item_name.as_str(), d.class_str.as_str()])
            .collect();
        known.sort_unstable();
        known.dedup();
        if let Some(hint) = crate::suggest::hint(word, known) {
            print_hint(&hint);
        }
        return 1;
    }
    for d in found_item {
        println!(
            "  {}{}{}  {:<22} {}",
            DIM,
            &d.id[..8.min(d.id.len())],
            RESET,
            d.item_name,
            d.class_str
        );
    }
    0
}

pub(crate) fn property_list(dest: &str) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let fs_path = format!("/model/object?target={}", url_encode(dest));
    let Some(o) = fetch_json(port, &fs_path) else {
        report_error("Could not read the instance.");
        return 1;
    };

    if let Some(e) = o.get("error").and_then(|x| x.as_str()) {
        report_error(e);
        if let Some(candidate_list) = o.get("candidates").and_then(|x| x.as_array()) {
            print_dim("  Candidates:");
            for a in candidate_list {
                println!(
                    "    {} ({}) -> {}",
                    a.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
                    a.get("className").and_then(|x| x.as_str()).unwrap_or("?"),
                    a.get("shortId").and_then(|x| x.as_str()).unwrap_or("?")
                );
            }
        }
        return 1;
    }

    print_info(&format!(
        "{}  ({})",
        o.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
        o.get("class_name").and_then(|x| x.as_str()).unwrap_or("?")
    ));
    print_dim(&format!(
        "  parent: {}",
        o.get("parent").and_then(|x| x.as_str()).unwrap_or("-")
    ));

    match o.get("properties").and_then(|x| x.as_object()) {
        Some(p) if !p.is_empty() => {
            println!("  properties:");
            let mut key_names: Vec<&String> = p.keys().collect();
            key_names.sort();
            for k in key_names {
                println!("    {:<18} {}", k, p[k]);
            }
        }
        _ => print_dim("  (no synced properties yet)"),
    }

    if let Some(a) = o.get("attributes").and_then(|x| x.as_object()) {
        if !a.is_empty() {
            println!("  attributes:");
            for (k, v) in a {
                println!("    {:<18} {}", k, v);
            }
        }
    }

    if let Some(s) = o.get("source").and_then(|x| x.as_str()) {
        if !s.is_empty() {
            print_dim(&format!("  source: {} lines", s.lines().count()));
        }
    }
    0
}

pub(crate) fn verify_input() -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(v) = fetch_json(port, "/model/verify") else {
        report_error("Verification failed.");
        return 1;
    };
    let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    println!("  total instances : {}", v.get("totalObjects").and_then(|x| x.as_u64()).unwrap_or(0));
    if ok {
        print_ok("  model is consistent");
        0
    } else {
        report_error("  model is inconsistent:");
        eprintln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        1
    }
}

pub(crate) fn show_tags(port: u16, dest: &str) -> i32 {
    let fs_path = format!("/model/object?target={}", url_encode(dest));
    let Some(o) = fetch_json(port, &fs_path) else {
        report_error("Could not read the instance.");
        return 1;
    };
    if let Some(e) = o.get("error").and_then(|x| x.as_str()) {
        report_error(e);
        return 1;
    }
    let tag_list: Vec<&str> = o
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if tag_list.is_empty() {
        print_info("No tags.");
    } else {
        for t in tag_list {
            println!("  {}", t);
        }
    }
    0
}
