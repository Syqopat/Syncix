//! Finding the running core: the port file first, then a scan of the range.

use std::path::PathBuf;

use super::*;

/// Looks for a .syncix/port file upwards from the working directory.
pub(crate) fn from_port_file() -> Option<u16> {
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
pub(crate) fn core_port() -> Option<u16> {
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

pub(crate) fn require_core() -> Option<u16> {
    match core_port() {
        Some(p) => Some(p),
        None => {
            report_error("Syncix Core is not running.");
            print_dim("  Start it with: syncix up   (or Syncix: Restart Core in VS Code)");
            None
        }
    }
}
