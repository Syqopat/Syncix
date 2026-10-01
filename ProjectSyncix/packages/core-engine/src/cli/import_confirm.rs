//! Checking that Studio really created what an import sent.
//!
//! A create is accepted by the core and queued for Studio, so sending it says nothing
//! about whether Studio made it. When Studio refused one (a class it will not create, a
//! parent that never arrived), it told the core, the core dropped it again -- and the
//! import still printed "Imported 1076 instance(s)". The user found out by noticing
//! something missing.
//!
//! So an import now waits for the tree to hold every identity it sent, sends what is
//! missing once more, and names what Studio still did not create.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::*;

/// One create as it was sent, so it can be checked and sent again.
pub(crate) struct Sent {
    pub id: String,
    pub name: String,
    pub class_name: String,
    pub data: serde_json::Value,
}

/// Is Studio there to receive an import at all?
///
/// Without it every create sits in the queue, nothing is created, and the check below
/// would report the whole import as missing. Saying so before anything is sent is the
/// honest answer.
pub(crate) fn studio_is_connected(port: u16) -> bool {
    fetch_json(port, "/health")
        .and_then(|h| h.get("studio_connected").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

/// Waits for Studio to apply the creates, resending once what did not arrive.
/// Returns what is still missing, in the order it was sent.
pub(crate) fn confirm(port: u16, sent: &[Sent]) -> Vec<&Sent> {
    if sent.is_empty() {
        return Vec::new();
    }
    let patience = patience_for(sent.len());

    let missing = wait_for(port, sent, patience);
    if missing.is_empty() {
        return Vec::new();
    }

    // One more attempt. A create whose parent had not arrived yet is the common case,
    // and by now the parent is there.
    print_dim(&format!(
        "  {} instance(s) had not arrived; sending them again...",
        missing.len()
    ));
    for item in &missing {
        send_command(port, "CREATE_INSTANCE", item.data.clone());
    }
    wait_for(port, sent, patience)
}

/// How long to wait for the whole batch. Studio applies a queue in its own time, so the
/// wait grows with the size of the import.
fn patience_for(count: usize) -> Duration {
    let grown = Duration::from_secs(2) + Duration::from_millis(20) * count as u32;
    grown.min(Duration::from_secs(60))
}

fn wait_for<'a>(port: u16, sent: &'a [Sent], patience: Duration) -> Vec<&'a Sent> {
    let started = Instant::now();
    loop {
        let present = ids_in_tree(port);
        let missing: Vec<&Sent> = sent
            .iter()
            .filter(|item| !present.contains(&item.id))
            .collect();
        if missing.is_empty() || started.elapsed() >= patience {
            return missing;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

fn ids_in_tree(port: u16) -> HashSet<String> {
    fetch_tree(port)
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.id)
        .collect()
}

/// Reports what Studio did not create, with the names to look for.
pub(crate) fn report_missing(missing: &[&Sent]) {
    report_error(&format!(
        "{} instance(s) did not reach Studio.",
        missing.len()
    ));
    for item in missing.iter().take(10) {
        print_dim(&format!("  {} ({})", item.name, item.class_name));
    }
    if missing.len() > 10 {
        print_dim(&format!("  ... and {} more", missing.len() - 10));
    }
    print_dim("  Studio refuses some classes outright; the core's log says which.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_grows_with_the_import_and_is_capped() {
        assert_eq!(patience_for(0), Duration::from_secs(2));
        assert_eq!(patience_for(100), Duration::from_secs(4));
        assert_eq!(patience_for(1_000_000), Duration::from_secs(60));
    }
}
