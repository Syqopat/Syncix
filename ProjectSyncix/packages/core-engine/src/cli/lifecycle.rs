//! Commands that start, stop, bind or test a project.

use std::path::PathBuf;

use super::*;

pub(crate) fn init_project() -> i32 {
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

pub(crate) fn launch(port: Option<u16>) -> i32 {
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
            report_error("The core started but did not respond. Check .syncix/syncix-core.log.");
            1
        }
        Err(e) => {
            report_error(&format!("Could not start: {}", e));
            1
        }
    }
}

pub(crate) fn stop_core() -> i32 {
    let Some(port) = core_port() else {
        println!("{}The core is not running.{}", YELLOW, RESET);
        return 0;
    };
    match http_request(port, "POST", "/core/stop", Some("{}")) {
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
pub(crate) fn wait_for_fresh_snapshot(port: u16) -> bool {
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
pub(crate) fn selftest() -> i32 {
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
    let position_ok = fetch_json(port, &format!("/model/object?target={}", id))
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
    let color_ok = fetch_json(port, &format!("/model/object?target={}", id))
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

/// Shows the settings in effect.
///
/// Why it is needed: "I wrote the setting but nothing changed" is the most common complaint.
/// Whether the place you wrote the setting and the place the program reads are the same — the answer is here.
/// Shows a place conflict and resolves it with --studio / --disk.
///
/// Why a command: both options can lose data. For Syncix to pick one on its
/// own would mean deleting one side without the user knowing.
/// So the decision is made here, explicitly.
pub(crate) fn bind_cmd(cli_args: &[String]) -> i32 {
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

pub(crate) fn show_config() -> i32 {
    let c = crate::project::ProjectConfig::load();

    println!("Project: {}", c.name);
    println!("Root:    {}", c.root.display());
    println!("Config:  {}", c.root.join("syncix.toml").display());
    println!();
    let warnings = crate::project::ProjectConfig::warnings();
    for warning in &warnings {
        println!("{}  {}{}", YELLOW, warning, RESET);
    }
    if !warnings.is_empty() {
        println!();
    }

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
