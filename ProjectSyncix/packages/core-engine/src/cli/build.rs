//! Writing a place file, a sourcemap, and publishing.


use super::*;

pub(crate) fn build_sourcemap(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let dest = output_file(cli_args, "sourcemap.json");

    let reply = match http_request(port, "GET", "/model/sourcemap", None) {
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
pub(crate) fn run_build(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };
    let dest_file = output_file(cli_args, "build.rbxmx");
    let positionals = positional(cli_args);
    let dest_object = positionals.first().cloned().unwrap_or_default();

    let fs_path = if dest_object.is_empty() {
        "/model/export".to_string()
    } else {
        format!("/model/export?target={}", url_encode(&dest_object))
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
pub(crate) fn publish_place(cli_args: &[String]) -> i32 {
    let Some(port) = require_core() else { return 1 };

    let is_confirmed = cli_args.iter().any(|a| a == "--confirm");

    // Get the project root and settings from /health.
    let Some(health_json) = fetch_json(port, "/health") else {
        report_error("Could not reach the core.");
        return 1;
    };
    let _ = &health_json;
    let root_dir = crate::project::ProjectConfig::load().root;

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
    let reply = match http_request(port, "GET", "/model/export", None) {
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
