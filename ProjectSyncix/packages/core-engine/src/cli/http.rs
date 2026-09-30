//! The minimal HTTP client the CLI talks to the core with, and the project token.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use super::*;

/// The project's access token, read once per process.
///
/// Every call except /health needs it. The file is the fast path; when the CLI runs
/// outside the project folder the core hands the token out through /health, which is
/// the same door the Studio plugin uses.
pub(crate) fn access_token(port: u16) -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| token_from_file().unwrap_or_else(|| token_from_health(port)))
        .clone()
}

pub(crate) fn token_from_file() -> Option<String> {
    let mut directory: PathBuf = std::env::current_dir().ok()?;
    for _ in 0..6 {
        let path = directory.join(".syncix").join("token");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let token = text.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
        if !directory.pop() {
            break;
        }
    }
    None
}

pub(crate) fn token_from_health(port: u16) -> String {
    match http_call(port, "GET", "/health", None, false) {
        Ok(reply) => serde_json::from_str::<serde_json::Value>(&reply.body)
            .ok()
            .and_then(|v| v.get("token").and_then(|t| t.as_str()).map(String::from))
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

pub(crate) fn http_request(port: u16, http_method: &str, fs_path: &str, body: Option<&str>) -> Result<HttpReply, String> {
    http_call(port, http_method, fs_path, body, true)
}

pub(crate) fn http_call(
    port: u16,
    http_method: &str,
    fs_path: &str,
    body: Option<&str>,
    with_token: bool,
) -> Result<HttpReply, String> {
    let address = format!("127.0.0.1:{}", port);
    let mut flow = TcpStream::connect(&address).map_err(|e| format!("could not connect to {}: {}", address, e))?;
    flow.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
    flow.set_write_timeout(Some(std::time::Duration::from_secs(15))).ok();

    let mut raw = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        http_method, fs_path, port
    );
    if with_token {
        raw.push_str(&format!("X-Syncix-Token: {}
", access_token(port)));
    }
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
pub(crate) fn parse_headers(raw: &str) -> std::collections::HashMap<String, String> {
    let mut lookup = std::collections::HashMap::new();
    // The first line is the status line (HTTP/1.1 200 OK), not a header.
    for line_text in raw.lines().skip(1) {
        if let Some((item_name, raw_value)) = line_text.split_once(':') {
            lookup.insert(item_name.trim().to_lowercase(), raw_value.trim().to_string());
        }
    }
    lookup
}

pub(crate) fn url_encode(s: &str) -> String {
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

pub(crate) fn fetch_json(port: u16, fs_path: &str) -> Option<serde_json::Value> {
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
pub(crate) fn send_command(port: u16, event_type: &str, data: serde_json::Value) -> bool {
    let body = serde_json::json!({ "event_type": event_type, "data": data }).to_string();
    match http_request(port, "POST", "/commands", Some(&body)) {
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
