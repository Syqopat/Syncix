//! Reading arguments and flags.


use super::*;

/// The options each command takes; None for commands whose values are free text.
/// A misspelt option used to be read as a plain argument: `rm Box --yse` asked for
/// confirmation, `attr Box Hp --delet` set Hp to "--delet", `tag Box --nnoe` added a tag
/// called "--nnoe".
pub(crate) fn known_flags(command: &str) -> Option<&'static [&'static str]> {
    Some(match command {
        "rm" | "del" | "delete" => &["--yes"],
        "trash" => &["--files", "--in", "--class", "--since"],
        "restore" => &["--in", "--class", "--since", "--dry-run", "--all"],
        "bind" => &["--studio", "--disk"],
        "upload" => &["--confirm"],
        "build" | "sourcemap" => &["--output"],
        "attr" => &["--delete"],
        "tag" | "tags" => &["--none"],
        "set" | "rename" | "rn" => return None,
        _ => &[],
    })
}

/// False, after saying so, when an argument looks like an option the command does not take.
pub(crate) fn flags_ok(cli_args: &[String]) -> bool {
    let command = cli_args[0].as_str();
    let Some(known) = known_flags(command) else { return true };
    for a in cli_args.iter().skip(1).filter(|a| a.starts_with("--")) {
        if known.contains(&a.as_str()) {
            continue;
        }
        let hint = crate::suggest::hint(a, known.iter().copied());
        // An attribute value may start with dashes; only a slip of --delete is refused.
        if command == "attr" && hint.is_none() {
            continue;
        }
        report_error(&format!("Unknown option {} for syncix {}.", a, command));
        match hint {
            Some(hint) => print_hint(&hint),
            None if known.is_empty() => print_dim(&format!("  syncix {} takes no options.", command)),
            None => print_dim(&format!("  Options: {}", known.join(", "))),
        }
        return false;
    }
    true
}

/// Resolves the -o flag and the default file name.
pub(crate) fn output_file(cli_args: &[String], fallback_value: &str) -> String {
    for (i, a) in cli_args.iter().enumerate() {
        if (a == "-o" || a == "--output") && i + 1 < cli_args.len() {
            return cli_args[i + 1].clone();
        }
    }
    fallback_value.to_string()
}

/// Returns the remaining positional arguments with -o and its value removed.
pub(crate) fn positional(cli_args: &[String]) -> Vec<String> {
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

/// The value after a flag: `--class part` -> "part".
pub(crate) fn flag_arg(cli_args: &[String], flag: &str) -> Option<String> {
    cli_args.iter().position(|a| a == flag).and_then(|i| cli_args.get(i + 1)).cloned()
}
