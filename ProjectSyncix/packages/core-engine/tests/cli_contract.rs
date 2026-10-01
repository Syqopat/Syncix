//! What the CLI promises: exit codes, the words it prints and the files it produces.
//!
//! A script and an editor both read these. A command that printed an error and still
//! exited 0 made every wrapper around it wrong.

mod support;

use support::{strip_colour, TestCore};
use std::process::Command;

fn run(args: &[&str], cwd: &std::path::Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_syncix-core"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("the CLI should run");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), strip_colour(&text))
}

fn empty_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("syncix-cli-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn help_lists_the_commands_and_exits_zero() {
    let dir = empty_dir("help");
    let (code, output) = run(&["--help"], &dir);
    assert_eq!(code, 0, "{}", output);
    for command in ["serve", "tree", "import", "config", "trash"] {
        assert!(output.contains(command), "{} missing from --help", command);
    }
}

#[test]
fn an_unknown_command_fails_and_suggests_the_real_one() {
    let dir = empty_dir("unknown");
    let (code, output) = run(&["treee"], &dir);
    assert_eq!(code, 1, "{}", output);
    assert!(output.contains("tree"), "the closest command should be offered: {}", output);
}

#[test]
fn an_unknown_option_is_refused_rather_than_read_as_a_value() {
    let dir = empty_dir("flag");
    let (code, output) = run(&["config", "--schemaa"], &dir);
    assert_eq!(code, 1, "{}", output);
    assert!(output.contains("--schema"), "{}", output);
}

#[test]
fn the_schema_is_printed_as_json_without_a_project() {
    let dir = empty_dir("schema");
    let (code, output) = run(&["config", "--schema"], &dir);
    assert_eq!(code, 0, "{}", output);
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
    assert!(parsed["properties"]["files"]["properties"]["sync_dir"].is_object());
}

#[test]
fn a_command_that_needs_the_core_says_so_when_it_is_not_running() {
    let dir = empty_dir("no-core");
    let (code, output) = run(&["tree"], &dir);
    assert_eq!(code, 1, "{}", output);
    assert!(
        output.to_lowercase().contains("core"),
        "the missing core has to be named: {}",
        output
    );
}

#[test]
fn config_shows_the_settings_that_are_actually_in_effect() {
    let core = TestCore::start("cli-config", "\n[sync]\ndebounce_ms = 250\n");
    let (code, output) = core.cli(&["config"]);
    assert_eq!(code, 0, "{}", output);
    assert!(output.contains("debounce_ms    250"), "{}", output);
    assert!(output.contains("job_workers"), "{}", output);
}

/// An older, flat syncix.toml is rewritten into sections the first time the core reads
/// it, and the old file is kept beside it.
#[test]
fn an_old_config_is_migrated_with_a_backup() {
    let dir = empty_dir("migrate");
    std::fs::write(
        dir.join("syncix.toml"),
        "# my settings\nsync_dir = \"game\"  # the tree\nplay_mode = \"queue\"\n",
    )
    .unwrap();

    let (code, output) = run(&["config"], &dir);
    assert_eq!(code, 0, "{}", output);

    let rewritten = std::fs::read_to_string(dir.join("syncix.toml")).unwrap();
    assert!(rewritten.contains("[files]"), "{}", rewritten);
    assert!(rewritten.contains("# my settings"), "comments stay: {}", rewritten);
    assert!(rewritten.contains("# the tree"), "comments stay: {}", rewritten);
    assert!(!rewritten.contains("play_mode"), "{}", rewritten);
    assert!(
        dir.join("syncix.toml.bak").exists(),
        "the file as it was has to be kept"
    );
}
