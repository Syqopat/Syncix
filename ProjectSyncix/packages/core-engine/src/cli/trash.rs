//! The trash: listing what was deleted and putting it back.


use super::*;

/// Near spellings for a trash search that found nothing: names, runs and classes the
/// trash really holds.
pub(crate) fn print_trash_hint(entries: &[crate::layout::TrashEntry], filter: &crate::layout::TrashFilter) {
    if let Some(name) = &filter.name {
        let mut known = crate::layout::trash_names(entries);
        known.extend(entries.iter().map(|e| e.run.clone()));
        known.sort();
        known.dedup();
        if let Some(hint) = crate::suggest::hint(name, known.iter().map(String::as_str)) {
            print_hint(&hint);
        }
    }
    if let Some(class) = &filter.class_name {
        let known = crate::layout::trash_classes(entries);
        if let Some(hint) = crate::suggest::hint(class, known.iter().map(String::as_str)) {
            print_hint(&format!("--class: {}", hint));
        }
    }
}

/// The first argument that is neither a flag nor a flag's value.
pub(crate) fn trash_name_arg(cli_args: &[String]) -> Option<String> {
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
pub(crate) fn since_timestamp(text: &str) -> Option<String> {
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

pub(crate) fn trash_filter(cli_args: &[String]) -> Result<crate::layout::TrashFilter, String> {
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

pub(crate) fn trash_list(cli_args: &[String]) -> i32 {
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
            print_trash_hint(&entries, &filter);
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

pub(crate) fn trash_restore(cli_args: &[String]) -> i32 {
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
pub(crate) fn trash_restore_selected(cli_args: &[String], sync_dir: &str) -> i32 {
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
        print_trash_hint(&entries, &filter);
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
