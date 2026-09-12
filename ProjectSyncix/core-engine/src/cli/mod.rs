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

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use crate::project::{DEFAULT_PORT, PORT_SCAN_SPAN};

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";

fn report_error(m: &str) {
    eprintln!("{}{}{}", RED, m, RESET);
}
fn print_ok(m: &str) {
    println!("{}{}{}", GREEN, m, RESET);
}
fn print_info(m: &str) {
    println!("{}{}{}", CYAN, m, RESET);
}
fn print_dim(m: &str) {
    println!("{}{}{}", DIM, m, RESET);
}

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

fn http_request(port: u16, http_method: &str, fs_path: &str, body: Option<&str>) -> Result<HttpReply, String> {
    let address = format!("127.0.0.1:{}", port);
    let mut flow = TcpStream::connect(&address).map_err(|e| format!("could not connect to {}: {}", address, e))?;
    flow.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
    flow.set_write_timeout(Some(std::time::Duration::from_secs(15))).ok();

    let mut raw = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        http_method, fs_path, port
    );
    if let Some(b) = body {
        raw.push_str("Content-Type: application/json\r\n");
        raw.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    raw.push_str("\r\n");
    if let Some(b) = body {
        raw.push_str(b);
    }

    flow.write_all(raw.as_bytes()).map_err(|e| e.to_string())?;

    let mut buffer = Vec::new();
    flow.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
    let text_value = String::from_utf8_lossy(&buffer).to_string();

    let (headers, body_string) = match text_value.find("\r\n\r\n") {
        Some(i) => (&text_value[..i], text_value[i + 4..].to_string()),
        None => (text_value.as_str(), String::new()),
    };

    let status_info = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    Ok(HttpReply {
        status_info,
        body: body_string,
        headers: parse_headers(headers),
    })
}

/// Parses HTTP response headers.
///
/// It is a separate function for testability: the header path matters
/// for `syncix upload` (the skipped-Enum count comes from it) and must be verifiable
/// without opening a TCP connection.
fn parse_headers(raw: &str) -> std::collections::HashMap<String, String> {
    let mut lookup = std::collections::HashMap::new();
    // The first line is the status line (HTTP/1.1 200 OK), not a header.
    for line_text in raw.lines().skip(1) {
        if let Some((item_name, raw_value)) = line_text.split_once(':') {
            lookup.insert(item_name.trim().to_lowercase(), raw_value.trim().to_string());
        }
    }
    lookup
}

fn url_encode(s: &str) -> String {
    let mut out_text = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out_text.push(b as char)
            }
            _ => out_text.push_str(&format!("%{:02X}", b)),
        }
    }
    out_text
}

// ---------------------------------------------------------------------------
// Core'u bulma
// ---------------------------------------------------------------------------

/// Looks for a .syncix/port file upwards from the working directory.
fn from_port_file() -> Option<u16> {
    let mut directory: PathBuf = std::env::current_dir().ok()?;
    for _ in 0..6 {
        let p = directory.join(".syncix").join("port");
        if let Ok(text_value) = std::fs::read_to_string(&p) {
            if let Ok(port) = text_value.trim().parse::<u16>() {
                return Some(port);
            }
        }
        if !directory.pop() {
            break;
        }
    }
    None
}

/// Finds the running core's port: the port file first, then a range scan.
fn core_port() -> Option<u16> {
    if let Some(p) = from_port_file() {
        if http_request(p, "GET", "/health", None).map(|c| c.status_info == 200).unwrap_or(false) {
            return Some(p);
        }
    }
    (DEFAULT_PORT..DEFAULT_PORT + PORT_SCAN_SPAN).find(|&p| {
        http_request(p, "GET", "/health", None)
            .map(|c| c.status_info == 200)
            .unwrap_or(false)
    })
}

fn require_core() -> Option<u16> {
    match core_port() {
        Some(p) => Some(p),
        None => {
            report_error("Syncix Core is not running.");
            print_dim("  Start it with: syncix up   (or Syncix: Restart Core in VS Code)");
            None
        }
    }
}

fn fetch_json(port: u16, fs_path: &str) -> Option<serde_json::Value> {
    match http_request(port, "GET", fs_path, None) {
        Ok(c) => serde_json::from_str(&c.body).ok(),
        Err(e) => {
            report_error(&format!("Request failed: {}", e));
            None
        }
    }
}

/// Sends a command to the core; on error, shows the server's message as is
/// (ambiguous-target warnings come from here).
fn send_command(port: u16, event_type: &str, data: serde_json::Value) -> bool {
    let body = serde_json::json!({ "event_type": event_type, "data": data }).to_string();
    match http_request(port, "POST", "/command", Some(&body)) {
        Ok(c) if c.status_info == 200 => true,
        Ok(c) => {
            report_error(&format!("Rejected ({}):", c.status_info));
            eprintln!("{}", c.body.trim());
            false
        }
        Err(e) => {
            report_error(&format!("Could not send: {}", e));
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Komutlar
// ---------------------------------------------------------------------------

fn print_help() {
    println!(
        r#"Syncix CLI  (version {})

  Status
    syncix status                 core and Studio connection, metrics
    syncix config                 show the settings in effect
    syncix bind                   show a place/folder mismatch
    syncix bind --studio|--disk   resolve it (which side is right)
    syncix verify                 check model/disk consistency
    syncix trash [--files]        what the reconciler removed (runs, or single files)
    syncix restore [run]          put a whole trash run back (newest by default)
    syncix restore <name> [--in path] [--class c] [--since 2h] [--dry-run] [--all]
                                  put single instances back (newest copy of each)
    syncix pull                   ask Studio to resend the tree (source of truth)
    syncix selftest               run an end-to-end scenario against real Studio

  Output
    syncix sourcemap [-o file]    generate sourcemap.json for luau-lsp
    syncix build [-o file] [target]
                                  export the tree as Roblox XML (.rbxmx)
    syncix import <file> [parent] import a .rbxmx/.rbxlx file into the tree
    syncix upload                 show what would be published (does nothing)
    syncix upload --confirm       actually publish to Roblox

  Inspect
    syncix tree [target]          show the tree
    syncix ls [target]            list a node's children
    syncix find <word>            search by name or class
    syncix props <target>         show every property of an instance

  Edit
    syncix new <class> [name] [parent]
    syncix rename <target> <new name>
    syncix rm <target> [--yes]    asks for confirmation unless --yes
    syncix mv <target> <new parent>
    syncix set <target> <property> <value>
    syncix attr <target> <name> <value>
    syncix attr <target> <name> --delete    remove an attribute
    syncix tag <target>           show CollectionService tags
    syncix tag <target> <tag>...  replace the tag list (--none clears it)

  Process
    syncix up [port]              start the core in the background
    syncix serve [port]           run the core in this terminal
    syncix down                   stop the core
    syncix init                   create syncix.toml and the sync folder

  Without a port the value from syncix.toml is used, otherwise 8080.
  If that port is taken the next one is tried and the Studio plugin finds it.
  To pin a specific port: syncix serve 25565, then type 25565 into the port
  field in the Studio plugin panel.

  Targets: full UUID, 8-char short UUID, name, or dot path
           (e.g. Workspace.Simulator.SellPad)
  Values:  5 | true | "text" | 0,0.5,-60 (Vector3) | #ff8800 (color)
           | Enum.Material.Neon
"#,
        crate::project::VERSION
    );
}

fn status_info() -> i32 {
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
    println!("  folder       : {}", text_value("root"));
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

struct TreeRow {
    id: String,
    item_name: String,
    class_str: String,
    parent_ref: Option<String>,
}

fn fetch_tree(port: u16) -> Option<Vec<TreeRow>> {
    let v = fetch_json(port, "/tree")?;
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

fn print_tree(node_list: &[TreeRow], root_dir: Option<&str>, prefix: &str, depth: usize) {
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

fn tree(dest: Option<&str>) -> i32 {
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
                report_error(&format!("Not found: {}", h));
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
fn resolve_row<'a>(port: u16, node_list: &'a [TreeRow], dest: &str) -> Option<&'a TreeRow> {
    if let Some(row) = find_node(node_list, dest) {
        return Some(row);
    }
    let object = fetch_json(port, &format!("/object?target={}", url_encode(dest)))?;
    let id = object.get("syncix_id")?.as_str()?;
    node_list.iter().find(|row| row.id == id)
}

fn find_node<'a>(node_list: &'a [TreeRow], dest: &str) -> Option<&'a TreeRow> {
    let h = dest.to_lowercase();
    node_list
        .iter()
        .find(|d| d.id == dest)
        .or_else(|| node_list.iter().find(|d| d.id.starts_with(&h)))
        .or_else(|| node_list.iter().find(|d| d.item_name.to_lowercase() == h))
}

fn list_instances(dest: Option<&str>) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(node_list) = fetch_tree(port) else { return 1 };

    let root_dir: Option<String> = match dest {
        None => None,
        Some(h) => match resolve_row(port, &node_list, h) {
            Some(d) => Some(d.id.clone()),
            None => {
                report_error(&format!("Not found: {}", h));
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

fn search(word: &str) -> i32 {
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

fn property_list(dest: &str) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let fs_path = format!("/object?target={}", url_encode(dest));
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

fn verify_input() -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(v) = fetch_json(port, "/verify") else {
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

fn init_project() -> i32 {
    let root_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = root_dir.join("syncix.toml");
    if cfg.exists() {
        println!("{}syncix.toml already exists: {}{}", YELLOW, cfg.display(), RESET);
    } else {
        let file_content = format!(
            "# Syncix project settings\nsync_dir = \"src\"\nport = {}\n",
            DEFAULT_PORT
        );
        if let Err(e) = std::fs::write(&cfg, file_content) {
            report_error(&format!("Could not write syncix.toml: {}", e));
            return 1;
        }
        print_ok(&format!("Created syncix.toml: {}", cfg.display()));
    }

    let syncing = root_dir.join("src");
    if !syncing.exists() {
        if let Err(e) = std::fs::create_dir_all(&syncing) {
            report_error(&format!("Could not create sync folder: {}", e));
            return 1;
        }
        print_ok(&format!("Created sync folder: {}", syncing.display()));
    }

    // The .syncix working folder must stay out of version control.
    let gitignore = root_dir.join(".gitignore");
    let current_value = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if !current_value.contains(".syncix") {
        let fresh = format!("{}\n# Syncix runtime files\n.syncix/\nsyncix-core.log\n", current_value);
        let _ = std::fs::write(&gitignore, fresh);
        print_dim("  .gitignore updated (.syncix/)");
    }

    print_dim("\nNext: open the folder in VS Code; the Syncix core starts automatically.");
    0
}

fn launch(port: Option<u16>) -> i32 {
    // If a specific port was requested, check whether a core already exists there;
    // otherwise any core will do.
    match port {
        Some(p) => {
            if http_request(p, "GET", "/health", None).map(|c| c.status_info == 200).unwrap_or(false) {
                println!("{}A core is already running on port {}.{}", YELLOW, p, RESET);
                return 0;
            }
        }
        None => {
            if core_port().is_some() {
                println!("{}The core is already running.{}", YELLOW, RESET);
                return 0;
            }
        }
    }

    let Ok(own) = std::env::current_exe() else {
        report_error("Could not locate my own executable.");
        return 1;
    };

    // Starts itself in server mode in the background.
    let mut command_name = std::process::Command::new(&own);
    command_name.arg("serve");
    if let Some(p) = port {
        command_name.arg(p.to_string());
    }
    command_name
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    match command_name.spawn() {
        Ok(_) => {
            // Wait for it to come up
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if let Some(p) = core_port() {
                    print_ok(&format!("Syncix Core started (port {}).", p));
                    return 0;
                }
            }
            report_error("The core started but did not respond. Check syncix-core.log.");
            1
        }
        Err(e) => {
            report_error(&format!("Could not start: {}", e));
            1
        }
    }
}

fn stop_core() -> i32 {
    let Some(port) = core_port() else {
        println!("{}The core is not running.{}", YELLOW, RESET);
        return 0;
    };
    match http_request(port, "POST", "/shutdown", Some("{}")) {
        Ok(_) => {
            print_ok("Syncix Core stopped.");
            0
        }
        Err(_) => {
            // The server may close the connection without replying; that is expected.
            print_ok("Syncix Core stopped.");
            0
        }
    }
}

/// Asks Studio for the tree again and waits for the reply.
///
/// This function is the backbone of selftest. Verification used to read /object;
/// but the core updates the model ALREADY while sending the command, so
/// reading the model does NOT PROVE the command reached Studio. Here Studio is
/// made to speak: the snapshot that comes back is the single source of truth.
///
/// That a reply arrived is seen from the "messages from Studio" counter on /health
/// going up.
fn wait_for_fresh_snapshot(port: u16) -> bool {
    let prior = fetch_json(port, "/health")
        .and_then(|h| h.get("inbound_from_studio").and_then(|x| x.as_u64()))
        .unwrap_or(0);

    if !send_command(port, "FULL_SYNC", serde_json::json!({})) {
        return false;
    }

    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let current_time = fetch_json(port, "/health")
            .and_then(|h| h.get("inbound_from_studio").and_then(|x| x.as_u64()))
            .unwrap_or(0);
        if current_time > prior {
            // A short wait so the model settles after the snapshot is processed.
            std::thread::sleep(std::time::Duration::from_millis(400));
            return true;
        }
    }
    false
}

/// End-to-end scenario: do the core and Studio really work together?
///
/// This command exists because of the Position bug: values looked right in the core
/// but never reached Studio. Here, after every step, the value is READ BACK
/// from Studio's state; there is no "I sent it, so it happened" assumption.
fn selftest() -> i32 {
    let Some(port) = require_core() else { return 1 };

    let Some(h) = fetch_json(port, "/health") else { return 1 };
    if !h.get("studio_connected").and_then(|x| x.as_bool()).unwrap_or(false) {
        report_error("Studio is not connected; the end-to-end test cannot run.");
        print_dim("  Open Roblox Studio, wait for the Syncix plugin to connect, then retry.");
        return 1;
    }

    let item_name = "SyncixSelftestParcasi";
    let mut failed = 0;
    let mut step = |no: usize, title: &str, ok: bool, detail: String| {
        if ok {
            println!("  {}[{}] {}{}", GREEN, no, title, RESET);
        } else {
            println!("  {}[{}] {} -> {}{}", RED, no, title, detail, RESET);
            failed += 1;
        }
    };

    print_info("Syncix end-to-end test");
    print_dim("  After each step the tree is re-requested from Studio;");
    print_dim("  verification uses Studio's reply, not the core's own model.");

    // 1. Create
    let was_created = send_command(
        port,
        "CREATE_INSTANCE",
        serde_json::json!({ "className": "Part", "name": item_name, "parentId": "Workspace" }),
    );
    wait_for_fresh_snapshot(port);
    let tree1 = fetch_tree(port).unwrap_or_default();
    let was_found = tree1.iter().find(|d| d.item_name == item_name).map(|d| d.id.clone());
    step(1, "create instance", was_created && was_found.is_some(), "instance did not appear in the tree".into());

    let Some(id) = was_found else {
        report_error("Test aborted: could not create the instance.");
        return 1;
    };

    // 2. Vector3 position — the exact scenario of the Position bug
    send_command(
        port,
        "SET_PROPERTY",
        serde_json::json!({ "id": id, "property": "Position", "value": "12,7,-34" }),
    );
    wait_for_fresh_snapshot(port);
    let position_ok = fetch_json(port, &format!("/object?target={}", id))
        .and_then(|o| o.get("properties")?.get("Position")?.get("Vector3").cloned())
        .map(|v| {
            (v.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) - 12.0).abs() < 0.01
                && (v.get("z").and_then(|x| x.as_f64()).unwrap_or(0.0) + 34.0).abs() < 0.01
        })
        .unwrap_or(false);
    step(2, "Vector3 position", position_ok, "position was not applied in Studio".into());

    // 3. Colour — hex conversion
    send_command(
        port,
        "SET_PROPERTY",
        serde_json::json!({ "id": id, "property": "Color", "value": "#ff8800" }),
    );
    wait_for_fresh_snapshot(port);
    let color_ok = fetch_json(port, &format!("/object?target={}", id))
        .and_then(|o| o.get("properties")?.get("Color")?.get("Color3").cloned())
        .map(|v| (v.get("r").and_then(|x| x.as_f64()).unwrap_or(0.0) - 1.0).abs() < 0.02)
        .unwrap_or(false);
    step(3, "hex color", color_ok, "color was not applied".into());

    // 4. Rename — the UUID must not change
    let renamed_to = "SyncixSelftestYeniAd";
    send_command(
        port,
        "RENAME_INSTANCE",
        serde_json::json!({ "id": id, "newName": renamed_to }),
    );
    wait_for_fresh_snapshot(port);
    let tree2 = fetch_tree(port).unwrap_or_default();
    let name_ok = tree2.iter().any(|d| d.id == id && d.item_name == renamed_to);
    step(4, "rename (UUID preserved)", name_ok, "name did not change or UUID drifted".into());

    // 5. Delete
    send_command(port, "DELETE_INSTANCE", serde_json::json!({ "id": id }));
    wait_for_fresh_snapshot(port);
    let tree3 = fetch_tree(port).unwrap_or_default();
    let delete_ok = !tree3.iter().any(|d| d.id == id);
    step(5, "delete", delete_ok, "instance is still in the tree".into());

    println!();
    if failed == 0 {
        print_ok("All steps passed. Studio and the core are genuinely in sync.");
        0
    } else {
        report_error(&format!("{} step(s) failed.", failed));
        print_dim("  Check the [Syncix] warnings in the Studio Output window.");
        1
    }
}

/// Resolves the -o flag and the default file name.
fn output_file(cli_args: &[String], fallback_value: &str) -> String {
    for (i, a) in cli_args.iter().enumerate() {
        if (a == "-o" || a == "--output") && i + 1 < cli_args.len() {
            return cli_args[i + 1].clone();
        }
    }
    fallback_value.to_string()
}

/// Returns the remaining positional arguments with -o and its value removed.
fn positional(cli_args: &[String]) -> Vec<String> {
    let mut out_text = Vec::new();
    let mut to_skip = false;
    for a in cli_args.iter().skip(1) {
        if to_skip {
            to_skip = false;
            continue;
        }
        if a == "-o" || a == "--output" {
            to_skip = true;
            continue;
        }
        out_text.push(a.clone());
    }
    out_text
}

/// Generates sourcemap.json so luau-lsp can offer autocompletion.
///
/// Normally the core refreshes this file itself on every sync (the `sourcemap`
/// setting in syncix.toml). This command is for one-off generation or CI.
/// Files removed by the reconciler are moved to the trash.
/// This command shows what is there; without it the user would never learn that a
/// deleted file can be restored.
/// Shows the object to delete and its subtree and asks for confirmation.
/// If the terminal is not interactive (piped input), the deletion is refused:
/// treating an unanswered question as "yes" is, for a deletion, the wrong side to err on.
fn confirm_delete(port: u16, dest: &str) -> bool {
    let fs_path = format!("/object?target={}", url_encode(dest));
    match fetch_json(port, &fs_path).filter(|d| d.get("error").is_none()) {
        Some(d) => {
            let child_entry = d
                .get("children")
                .and_then(|c| c.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let item_name = d.get("name").and_then(|v| v.as_str()).unwrap_or(dest);
            let class_str = d.get("class_name").and_then(|v| v.as_str()).unwrap_or("?");
            if child_entry > 0 {
                // Deletion cascades: not just the direct children, everything below goes.
                println!(
                    "Delete {} ({}) and everything inside it ({} direct child object(s))?",
                    item_name, class_str, child_entry
                );
            } else {
                println!("Delete {} ({})?", item_name, class_str);
            }
        }
        None => {
            println!("Delete {}?", dest);
        }
    }
    print!("Type 'y' to confirm: ");
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut reply = String::new();
    if std::io::stdin().read_line(&mut reply).is_err() {
        return false;
    }
    let c = reply.trim().to_lowercase();
    c == "y" || c == "yes"
}

fn show_tags(port: u16, dest: &str) -> i32 {
    let fs_path = format!("/object?target={}", url_encode(dest));
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

/// Shows the settings in effect.
///
/// Why it is needed: "I wrote the setting but nothing changed" is the most common complaint.
/// Whether the place you wrote the setting and the place the program reads are the same — the answer is here.
/// Shows a place conflict and resolves it with --studio / --disk.
///
/// Why a command: both options can lose data. For Syncix to pick one on its
/// own would mean deleting one side without the user knowing.
/// So the decision is made here, explicitly.
fn bind_cmd(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let Some(health_json) = fetch_json(port, "/health") else {
        report_error("Could not read the core status.");
        return 1;
    };

    let place_clash = health_json.get("place_conflict");
    let direction = if cli_args.iter().any(|a| a == "--studio") {
        Some("studio")
    } else if cli_args.iter().any(|a| a == "--disk") {
        Some("disk")
    } else {
        None
    };

    let Some(c) = place_clash.filter(|x| !x.is_null()) else {
        let is_bound = crate::project::ProjectConfig::load().linked_place();
        match is_bound {
            Some(k) => print_ok(&format!("No conflict. This folder is bound to place {}.", k)),
            None => print_info("No conflict. This folder is not bound to a place yet."),
        }
        return 0;
    };

    let al = |item_name: &str| c.get(item_name).and_then(|x| x.as_str()).unwrap_or("?").to_string();

    let Some(direction) = direction else {
        // Show what happened before a decision is made.
        report_error("This folder belongs to a different place. Sync is on hold.");
        println!();
        println!("  folder is bound to : {}", al("folder_place"));
        println!(
            "  place connecting   : {} (\"{}\", id {})",
            al("incoming_place"),
            al("incoming_name"),
            al("incoming_place_id")
        );
        println!();
        println!("Choose one:");
        println!("  syncix bind --studio   this place is right; the folder is rewritten from it");
        println!("  syncix bind --disk     the folder is right; its contents go into this place");
        print_dim("  Files the reconciler removes go to the trash (syncix trash).");
        return 1;
    };

    if !send_command(port, "BIND", serde_json::json!({ "side": direction })) {
        return 1;
    }
    if direction == "studio" {
        print_ok("Bound to the connected place. The folder is being rewritten from Studio.");
    } else {
        print_ok("Bound to this folder. Its contents will be pushed into the connected place.");
    }
    0
}

fn show_config() -> i32 {
    let c = crate::project::ProjectConfig::load();

    println!("Project: {}", c.name);
    println!("Root:    {}", c.root.display());
    println!("Config:  {}", c.root.join("syncix.toml").display());
    println!();

    println!("[sync]");
    println!("  mode           {}", c.mode_value.name_of());
    println!("  debounce_ms    {}", c.debounce_ms);
    println!("  ask_permission {}", c.prompt_permission);
    println!("  undo           {}", c.restore_cmd);
    println!();

    println!("[files]");
    println!("  sync_dir       {}", c.sync_dir);
    println!("  meta_files     {}", c.meta_files);
    println!(
        "  ignore         {}",
        if c.ignore.is_empty() {
            "(none)".to_string()
        } else {
            c.ignore.join(", ")
        }
    );
    println!();

    println!("[safety]");
    println!("  trash            {}", c.safety_settings.trash_enabled);
    println!("  trash_keep       {}", c.safety_settings.trash_keep_runs);
    println!("  delete_grace_ms  {}", c.safety_settings.delete_grace_ms);
    println!("  confirm_delete   {}", c.safety_settings.confirm_delete);
    println!();

    let entry_list = |v: &Vec<String>| {
        if v.is_empty() {
            "(default)".to_string()
        } else {
            v.join(", ")
        }
    };
    println!("[scope]");
    println!("  services           {}", entry_list(&c.scope_settings.service_list));
    println!("  ignore_classes     {}", entry_list(&c.scope_settings.class_ignore_list));
    println!("  ignore_properties  {}", entry_list(&c.scope_settings.property_ignore_list));
    println!();

    println!("[server]");
    println!("  port           {}", c.wanted_port);
    println!();
    println!("[editor]");
    println!("  sourcemap      {}", c.sourcemap);
    println!();
    // The running core may have started with an old setting; that is the most misleading state.
    if let Some(port) = core_port() {
        if port != c.wanted_port {
            print_dim(&format!(
                "  Note: a core is running on port {}, which differs from the configured port.",
                port
            ));
        }
        print_dim("  Settings are read at startup; restart the core after editing (syncix down && syncix up).");
    }
    0
}

/// Flags of `trash`/`restore` that take a value.
const TRASH_VALUE_FLAGS: [&str; 3] = ["--in", "--class", "--since"];

/// The value after a flag: `--class part` -> "part".
fn flag_arg(cli_args: &[String], flag: &str) -> Option<String> {
    cli_args.iter().position(|a| a == flag).and_then(|i| cli_args.get(i + 1)).cloned()
}

/// The first argument that is neither a flag nor a flag's value.
fn trash_name_arg(cli_args: &[String]) -> Option<String> {
    let mut to_skip = false;
    for a in cli_args.iter().skip(1) {
        if to_skip {
            to_skip = false;
        } else if TRASH_VALUE_FLAGS.contains(&a.as_str()) {
            to_skip = true;
        } else if !a.starts_with("--") {
            return Some(a.clone());
        }
    }
    None
}

/// "30m", "2h", "1d", "45s" -> the run-name timestamp that long ago.
fn since_timestamp(text: &str) -> Option<String> {
    let t = text.trim();
    let (amount, unit) = t.split_at(t.len().checked_sub(1)?);
    let n: i64 = amount.parse().ok()?;
    let seconds = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86400,
        _ => return None,
    };
    Some((chrono::Utc::now() - chrono::Duration::seconds(seconds)).format("%Y%m%d-%H%M%S").to_string())
}

fn trash_filter(cli_args: &[String]) -> Result<crate::layout::TrashFilter, String> {
    let mut filter = crate::layout::TrashFilter {
        scope: flag_arg(cli_args, "--in"),
        class_name: flag_arg(cli_args, "--class"),
        ..Default::default()
    };
    if let Some(s) = flag_arg(cli_args, "--since") {
        let since = since_timestamp(&s).ok_or_else(|| format!("--since expects a duration such as 30m, 2h or 1d, not {}", s))?;
        filter.since = Some(since);
    }
    // A name with a slash is a place in the tree.
    match trash_name_arg(cli_args) {
        Some(n) if (n.contains('/') || n.contains('\\')) && filter.scope.is_none() => filter.scope = Some(n),
        other => filter.name = other,
    }
    Ok(filter)
}

fn trash_list(cli_args: &[String]) -> i32 {
    let settings_data = crate::project::ProjectConfig::load();
    if cli_args.iter().any(|a| a == "--files") {
        let filter = match trash_filter(cli_args) {
            Ok(f) => f,
            Err(e) => {
                report_error(&e);
                return 1;
            }
        };
        let entries = crate::layout::trash_entries(&settings_data.sync_dir);
        let picked = crate::layout::select_entries(&entries, &filter);
        if picked.is_empty() {
            print_info("No removed file matches.");
            return 0;
        }
        println!("Removed files (newest copy of each), newest first:");
        for e in &picked {
            println!("  {}  {}", e.run, e.rel);
        }
        println!();
        println!("Restore one with: syncix restore <name>   (--dry-run shows what it would do)");
        return 0;
    }

    let runs = crate::layout::trash_runs(&settings_data.sync_dir);
    if runs.is_empty() {
        print_info("Trash is empty; no files have been removed by the reconciler.");
        return 0;
    }
    println!("Removed files, newest first:");
    for (run_name, amount) in &runs {
        println!("  {}  {} file(s)", run_name, amount);
    }
    println!();
    println!("Restore a whole run with: syncix restore <run>");
    println!("Single files:            syncix trash --files, then syncix restore <name>");
    0
}

fn trash_restore(cli_args: &[String]) -> i32 {
    let settings_data = crate::project::ProjectConfig::load();
    let runs = crate::layout::trash_runs(&settings_data.sync_dir);
    let name_arg = trash_name_arg(cli_args);
    let has_filters = cli_args.iter().any(|a| TRASH_VALUE_FLAGS.contains(&a.as_str()));

    // Selective restore: a name that is not a run, or any filter. A whole run is too
    // coarse when it also holds things that were removed on purpose.
    let is_run = |n: &str| runs.iter().any(|(t, _)| t == n);
    if has_filters || name_arg.as_deref().is_some_and(|n| !is_run(n)) {
        return trash_restore_selected(cli_args, &settings_data.sync_dir);
    }

    // Without a name, the newest run is restored; that is the most common request.
    let selected = match name_arg {
        Some(t) => t,
        None => match runs.first() {
            Some((t, _)) => t.clone(),
            None => {
                print_info("Trash is empty; there is nothing to restore.");
                return 0;
            }
        },
    };
    let (restored_count, skipped) = crate::layout::restore_from_trash(&settings_data.sync_dir, &selected);
    print_ok(&format!("Restored {} file(s) from {}.", restored_count, selected));
    if skipped > 0 {
        // Overwriting would turn restoring into a data loss of its own.
        print_info(&format!(
            "{} file(s) were skipped because a file already exists at that path.",
            skipped
        ));
    }
    0
}

/// Puts single instances back: the newest copy of every file the filter picks.
fn trash_restore_selected(cli_args: &[String], sync_dir: &str) -> i32 {
    let filter = match trash_filter(cli_args) {
        Ok(f) => f,
        Err(e) => {
            report_error(&e);
            return 1;
        }
    };
    let entries = crate::layout::trash_entries(sync_dir);
    let picked = crate::layout::select_entries(&entries, &filter);
    if picked.is_empty() {
        print_info("Nothing in the trash matches.");
        print_dim("  See what is there: syncix trash --files");
        return 0;
    }

    let dry_run = cli_args.iter().any(|a| a == "--dry-run");
    // One name, several instances (an import had made copies): bringing all of them
    // back is rarely what was meant, so ask which one.
    if !dry_run && !cli_args.iter().any(|a| a == "--all") {
        if let Some(name) = &filter.name {
            let found = crate::layout::instances_named(&picked, name);
            if found.len() > 1 {
                print_info(&format!("{} different instances are called {}:", found.len(), name));
                for k in &found {
                    println!("  {}", k);
                }
                println!();
                println!("Pick one with --in, e.g.  syncix restore {} --in {}", name, found[0]);
                println!("or bring them all back with --all.");
                return 1;
            }
        }
    }

    if dry_run {
        println!("Would restore {} file(s):", picked.len());
        for e in &picked {
            println!("  {}  {}", e.run, e.rel);
        }
        return 0;
    }

    let (restored_count, skipped) = crate::layout::restore_entries(sync_dir, &picked);
    print_ok(&format!("Restored {} file(s).", restored_count));
    for e in picked.iter().take(20) {
        print_dim(&format!("  {}", e.rel));
    }
    if picked.len() > 20 {
        print_dim(&format!("  ... and {} more", picked.len() - 20));
    }
    if skipped > 0 {
        print_info(&format!(
            "{} file(s) were skipped because a file already exists at that path.",
            skipped
        ));
    }
    0
}

fn build_sourcemap(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let dest = output_file(cli_args, "sourcemap.json");

    let reply = match http_request(port, "GET", "/sourcemap", None) {
        Ok(c) if c.status_info == 200 => c.body,
        Ok(c) => {
            report_error(&format!("Could not fetch sourcemap ({})", c.status_info));
            return 1;
        }
        Err(e) => {
            report_error(&format!("Could not fetch sourcemap: {}", e));
            return 1;
        }
    };

    if let Err(e) = std::fs::write(&dest, &reply) {
        report_error(&format!("Could not write {}: {}", dest, e));
        return 1;
    }

    let number_value = reply.matches("\"className\"").count();
    print_ok(&format!("Wrote {} ({} instances).", dest, number_value));
    print_dim("  Once luau-lsp reads this file, paths like game.ReplicatedStorage.X get");
    print_dim("  autocomplete and type checking.");
    0
}

/// Writes the tree to a file as Roblox XML (the counterpart of rojo build).
fn run_build(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let dest_file = output_file(cli_args, "build.rbxmx");
    let positionals = positional(cli_args);
    let dest_object = positionals.first().cloned().unwrap_or_default();

    let fs_path = if dest_object.is_empty() {
        "/build".to_string()
    } else {
        format!("/build?target={}", url_encode(&dest_object))
    };

    let reply = match http_request(port, "GET", &fs_path, None) {
        Ok(c) if c.status_info == 200 => c,
        Ok(c) => {
            report_error(&format!("Build failed ({}):", c.status_info));
            eprintln!("{}", c.body.trim());
            return 1;
        }
        Err(e) => {
            report_error(&format!("Build failed: {}", e));
            return 1;
        }
    };

    if let Err(e) = std::fs::write(&dest_file, &reply.body) {
        report_error(&format!("Could not write {}: {}", dest_file, e));
        return 1;
    }

    let object_total = reply.body.matches("<Item ").count();
    print_ok(&format!(
        "Wrote {} ({} instances, {} bytes).",
        dest_file,
        object_total,
        reply.body.len()
    ));
    if dest_object.is_empty() {
        // A whole-place export carries the services; say what importing it back does.
        print_dim("  This is the whole place. syncix import merges its services into the");
        print_dim("  existing ones; to export one model, name it: syncix build <target>.");
    } else {
        print_dim("  In Studio: right click > Insert from File...");
    }
    0
}

/// Publishing to Roblox. BY DEFAULT IT PUBLISHES NOTHING.
///
/// Publishing is an external action that cannot be undone: the published version is the
/// one players see. So the command first explains what it would do and stops;
/// actually publishing requires an explicit confirmation flag.
fn publish_place(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };

    let is_confirmed = cli_args.iter().any(|a| a == "--confirm");

    // Get the project root and settings from /health.
    let Some(health_json) = fetch_json(port, "/health") else {
        report_error("Could not reach the core.");
        return 1;
    };
    let root_dir = std::path::PathBuf::from(
        health_json.get("root").and_then(|x| x.as_str()).unwrap_or("."),
    );

    // Safety gate: the key must not have been written into the project.
    if crate::upload::has_key_leak(&root_dir) {
        report_error("syncix.toml contains something that looks like an API key.");
        print_dim("  Keys must NOT live in project files; the first commit makes them public.");
        print_dim("  Remove it and use the SYNCIX_API_KEY environment variable instead.");
        return 1;
    }

    let cfg = crate::upload::UploadConfig::load(&root_dir);
    let universe = cfg.universe_id;
    let place = cfg.place_id;

    let (Some(universe_id), Some(place_id)) = (universe, place) else {
        report_error("No publish target configured.");
        print_dim("  Add this to syncix.toml:");
        print_dim("");
        print_dim("    [upload]");
        print_dim("    universe_id = 1234567890");
        print_dim("    place_id    = 9876543210");
        return 1;
    };

    // Build the place file from the live model.
    let reply = match http_request(port, "GET", "/build", None) {
        Ok(c) if c.status_info == 200 => c,
        Ok(c) => {
            report_error(&format!("Could not build the place file ({}).", c.status_info));
            return 1;
        }
        Err(e) => {
            report_error(&format!("Could not build the place file: {}", e));
            return 1;
        }
    };

    let dest_file = root_dir.join(".syncix").join("upload.rbxlx");
    if let Some(d) = dest_file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&dest_file, &reply.body) {
        report_error(&format!("Could not write {}: {}", dest_file.display(), e));
        return 1;
    }

    let plan = crate::upload::UploadPlan {
        universe_id,
        place_id,
        file_path: dest_file,
        byte_count: reply.body.len(),
        object_total: reply.body.matches("<Item ").count(),
        skipped_enums: reply
            .headers
            .get("x-syncix-skipped-enums")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0),
    };

    print_info("About to publish");
    println!("  universe : {}", plan.universe_id);
    println!("  place    : {}", plan.place_id);
    println!("  file     : {}", plan.file_path.display());
    println!("  contents : {} instances, {} bytes", plan.object_total, plan.byte_count);
    println!("  endpoint : {}", crate::upload::target_url(plan.universe_id, plan.place_id));

    // If Enums were skipped the file to publish is INCOMPLETE; the user must know
    // that before publishing, not after.
    if plan.skipped_enums > 0 {
        println!();
        println!(
            "{}WARNING: {} enum value(s) could not be exported.{}",
            YELLOW, plan.skipped_enums, RESET
        );
        print_dim("  The published file will be missing settings like Material and Shape.");
        print_dim("  If that is not acceptable, say so before publishing and we will extend the table.");
    }

    let has_key = std::env::var("SYNCIX_API_KEY").is_ok();
    if !has_key {
        println!();
        report_error("The SYNCIX_API_KEY environment variable is not set.");
        print_dim("  Get an Open Cloud key at: create.roblox.com > Creator Hub > API Keys");
        print_dim("  Grant it the 'universe-places:write' permission.");
        print_dim("  Then: $env:SYNCIX_API_KEY = \"...\"   (PowerShell)");
        return 1;
    }

    if !is_confirmed {
        println!();
        println!("{}Nothing was published.{}", YELLOW, RESET);
        print_dim("  Publishing cannot be undone: the published version is what players see.");
        print_dim("  If you are sure:  syncix upload --confirm");
        print_dim("");
        print_dim("  Or run it yourself:");
        for line_text in crate::upload::curl_command(&plan).lines() {
            print_dim(&format!("    {}", line_text));
        }
        return 0;
    }

    // Confirmation given: send with the system's curl.
    print_info("Publishing...");
    let out_text = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg(crate::upload::target_url(plan.universe_id, plan.place_id))
        .arg("-H")
        .arg(format!(
            "x-api-key: {}",
            std::env::var("SYNCIX_API_KEY").unwrap_or_default()
        ))
        .arg("-H")
        .arg("Content-Type: application/xml")
        .arg("--data-binary")
        .arg(format!("@{}", plan.file_path.display()))
        .output();

    match out_text {
        Ok(c) if c.status.success() => {
            let body = String::from_utf8_lossy(&c.stdout);
            if body.contains("versionNumber") {
                print_ok("Published.");
                println!("  {}", body.trim());
                0
            } else {
                report_error("Roblox did not return the expected response:");
                eprintln!("{}", body.trim());
                1
            }
        }
        Ok(c) => {
            report_error("Publish failed:");
            eprintln!("{}", String::from_utf8_lossy(&c.stderr).trim());
            1
        }
        Err(e) => {
            report_error(&format!("Could not run curl: {}", e));
            print_dim("  curl ships with Windows 10+, macOS and most Linux distributions.");
            print_dim("  Otherwise run the command above with your own tool.");
            1
        }
    }
}

/// Brings a .rbxmx / .rbxlx file into the tree (the last item Rojo had and Syncix lacked).
///
/// For every node a CREATE_INSTANCE is sent first, then its properties. Top-down
/// creation order matters: a child cannot be sent before its parent exists.
fn import_rbxmx(cli_args: &[String]) -> i32 {
    let Some(file_path) = cli_args.get(1) else {
        report_error("Usage: syncix import <file.rbxmx> [parent]");
        return 1;
    };
    let Some(port) = require_core() else { return 1 };
    let parent_ref = cli_args.get(2).cloned().unwrap_or_else(|| "Workspace".to_string());

    let xml = match std::fs::read_to_string(file_path) {
        Ok(x) => x,
        Err(e) => {
            report_error(&format!("Could not read {}: {}", file_path, e));
            return 1;
        }
    };

    let (root_list, skipped) = match crate::rbxmx_import::parse_text(&xml) {
        Ok(v) => v,
        Err(e) => {
            report_error(&format!("Could not parse {}: {}", file_path, e));
            return 1;
        }
    };

    let total_count = crate::rbxmx_import::tally(&root_list);
    if total_count == 0 {
        report_error("The file contains no instances.");
        return 1;
    }

    // The parent is resolved to an identity ONCE, before anything is created. Looked up by
    // name for every root, it became ambiguous as soon as the import itself created a
    // second instance with that name (a place file carrying its own ReplicatedStorage).
    let parent_id = match fetch_json(port, &format!("/object?target={}", url_encode(&parent_ref))) {
        Some(o) => match o.get("syncix_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                let reason = o.get("error").and_then(|e| e.as_str()).unwrap_or("not found");
                report_error(&format!("Parent {}: {}", parent_ref, reason));
                return 1;
            }
        },
        None => return 1,
    };
    // Snapshot of the tree, used to find the existing services a place file merges into.
    let tree = fetch_tree(port).unwrap_or_default();

    print_info(&format!("Importing {} instance(s) into {}", total_count, parent_ref));
    if root_list.iter().any(|n| crate::rbxmx_import::is_service(&n.class_name)) {
        print_dim("  The file holds services (a whole place): their contents merge into the");
        print_dim("  matching services of this place; the parent applies only to the rest.");
    }
    if skipped > 0 {
        print_dim(&format!(
            "  {} property value(s) use types Syncix does not model and were skipped.",
            skipped
        ));
    }

    #[derive(Default)]
    struct Outcome {
        created: usize,
        failed: usize,
        merged: usize,
        settings_kept: usize,
        out_of_scope: usize,
    }

    /// The existing instance a singleton maps onto. A service is looked up among the
    /// top-level services, any other singleton among the children of `parent`.
    fn existing_singleton<'a>(tree: &'a [TreeRow], class_name: &str, parent: Option<&str>) -> Option<&'a str> {
        let top_level = |r: &TreeRow| match r.parent_ref.as_deref() {
            None => true,
            Some(p) => tree.iter().any(|q| q.id == p && q.class_str == "DataModel"),
        };
        tree.iter()
            .find(|r| {
                r.class_str == class_name
                    && match parent {
                        None => top_level(r),
                        Some(p) => r.parent_ref.as_deref() == Some(p),
                    }
            })
            .map(|r| r.id.as_str())
    }

    // Recursive creation. Each node is created first, then its properties are written.
    fn generate(
        port: u16,
        tree: &[TreeRow],
        node_entry: &crate::rbxmx_import::ImportedNode,
        parent_ref: &str,
        outcome: &mut Outcome,
    ) {
        // Services and singleton containers are never created: Studio refuses to, and
        // the core used to keep a phantom copy that made names ambiguous. Their
        // contents go into the instance that already exists; their own settings are
        // left as they are, an import must not retune the place's Lighting.
        if crate::rbxmx_import::is_singleton(&node_entry.class_name) {
            let lookup_parent = if crate::rbxmx_import::is_service(&node_entry.class_name) {
                None
            } else {
                Some(parent_ref)
            };
            match existing_singleton(tree, &node_entry.class_name, lookup_parent) {
                Some(existing) => {
                    outcome.merged += 1;
                    outcome.settings_kept += node_entry.properties.len();
                    for child_entry in &node_entry.children {
                        generate(port, tree, child_entry, existing, outcome);
                    }
                }
                None => outcome.out_of_scope += crate::rbxmx_import::tally(std::slice::from_ref(node_entry)),
            }
            return;
        }

        // The identity is generated IN ADVANCE, so the created object can be targeted by
        // identity rather than by name. Targeting by name failed with an ambiguity error
        // when the imported tree repeated an existing name.
        let identity = uuid::Uuid::new_v4().to_string();
        let ok = send_command(
            port,
            "CREATE_INSTANCE",
            serde_json::json!({
                "id": identity,
                "className": node_entry.class_name,
                "name": node_entry.name,
                "parentId": parent_ref
            }),
        );
        if !ok {
            outcome.failed += 1;
            return;
        }
        outcome.created += 1;
        std::thread::sleep(std::time::Duration::from_millis(120));

        for (item_name, raw_value) in &node_entry.properties {
            // The value is not turned into text: text loses the type, and types like CFrame,
            // UDim and NumberRange were dropped entirely on import.
            // The wire format already carries the type, so it is sent directly.
            let value_as_json = crate::pv_to_wire(raw_value);
            send_command(
                port,
                "SET_PROPERTY",
                serde_json::json!({ "id": identity, "property": item_name, "value": value_as_json }),
            );
            std::thread::sleep(std::time::Duration::from_millis(60));
        }

        if let Some(origin) = &node_entry.source {
            send_command(
                port,
                "SET_PROPERTY",
                serde_json::json!({ "id": identity, "property": "Source", "value": origin }),
            );
            std::thread::sleep(std::time::Duration::from_millis(60));
        }

        for child_entry in &node_entry.children {
            generate(port, tree, child_entry, &identity, outcome);
        }
    }

    let mut outcome = Outcome::default();
    for k in &root_list {
        generate(port, &tree, k, &parent_id, &mut outcome);
    }

    if outcome.merged > 0 {
        print_dim(&format!(
            "  {} service/container item(s) merged into the existing ones; their own settings ({} value(s)) were left unchanged.",
            outcome.merged, outcome.settings_kept
        ));
    }
    if outcome.out_of_scope > 0 {
        print_info(&format!(
            "{} instance(s) skipped: their service is not part of this sync (see scope in syncix.toml).",
            outcome.out_of_scope
        ));
    }
    if outcome.failed > 0 {
        report_error(&format!("{} instance(s) could not be created.", outcome.failed));
        print_dim("  Check the result with syncix tree.");
        return 1;
    }

    print_ok(&format!("Imported {} instance(s).", outcome.created));
    print_dim("  Run syncix pull to confirm the result from Studio.");
    0
}

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
                let current = fetch_json(port, &format!("/object?target={}", url_encode(h)))
                    .and_then(|o| {
                        o.get("properties")?
                            .as_object()?
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(p))
                            .map(|(_, v)| v.clone())
                    })
                    .and_then(|v| serde_json::from_value::<crate::model::PropertyValue>(v).ok());
                if let Err(reason) = crate::value_from_text(p, current.as_ref(), &raw_value) {
                    report_error(&format!("{}; {}.{} was not changed.", reason, h, p));
                    for line in crate::color_forms_help() {
                        print_dim(&format!("  {}", line));
                    }
                    if p.eq_ignore_ascii_case("Color") {
                        print_dim("  A BrickColor name such as \"Really red\" belongs to the BrickColor property.");
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
            Some(class_str) if crate::rbxmx_import::is_singleton(class_str) => {
                // The engine refuses it too; saying so here beats a "Created" that did nothing.
                report_error(&format!("{} exists once per place and cannot be created.", class_str));
                1
            }
            Some(class_str) => {
                let Some(port) = require_core() else { return Some(1) };
                let item_name = arg(2).unwrap_or(class_str);
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
        "import" => import_rbxmx(cli_args),
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
