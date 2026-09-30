//! Commands that change the tree and the questions they ask first.


use super::*;

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
pub(crate) fn confirm_delete(port: u16, dest: &str) -> bool {
    let fs_path = format!("/model/object?target={}", url_encode(dest));
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
