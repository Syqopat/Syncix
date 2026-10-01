//! A real core in a temporary project, for the tests that have to cross a process
//! boundary.
//!
//! The unit tests cover the pieces; these cover what the pieces do together: HTTP
//! status codes, the token, what reaches disk, and what the CLI prints. The core runs
//! as the shipped binary, so the test exercises exactly what a user runs.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

pub struct TestCore {
    pub root: PathBuf,
    pub port: u16,
    pub token: String,
    child: Child,
}

/// Ports are handed out from a high range, one per test, so tests can run at the same
/// time without landing on each other's core.
fn next_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static NEXT: AtomicU16 = AtomicU16::new(21_500);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

impl TestCore {
    /// Starts a core in a fresh project folder. `extra_config` is appended to
    /// syncix.toml, for a test that needs a particular setting.
    pub fn start(name: &str, extra_config: &str) -> TestCore {
        let root = std::env::temp_dir().join(format!("syncix-it-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("syncix.toml"),
            format!("[files]\nsync_dir = \"src\"\n\n[editor]\nsourcemap = false\n{}", extra_config),
        )
        .unwrap();

        let port = next_port();
        let child = Command::new(env!("CARGO_BIN_EXE_syncix-core"))
            .arg("serve")
            .arg(port.to_string())
            .current_dir(&root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the core binary should start");

        let token = wait_for_token(&root).unwrap_or_else(|| {
            panic!("the core did not write .syncix/token in time (port {})", port)
        });
        let core = TestCore {
            root,
            port,
            token,
            child,
        };
        core.wait_until_healthy();
        core
    }

    fn wait_until_healthy(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(reply) = self.request("GET", "/health", None, false) {
                if reply.status == 200 {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("the core never answered /health on port {}", self.port);
    }

    pub fn get(&self, path: &str) -> Reply {
        self.request("GET", path, None, true).expect("request")
    }

    pub fn get_without_token(&self, path: &str) -> Reply {
        self.request("GET", path, None, false).expect("request")
    }

    pub fn post(&self, path: &str, body: serde_json::Value) -> Reply {
        self.request("POST", path, Some(&body.to_string()), true)
            .expect("request")
    }

    pub fn post_without_token(&self, path: &str, body: serde_json::Value) -> Reply {
        self.request("POST", path, Some(&body.to_string()), false)
            .expect("request")
    }

    /// What the Studio plugin sends. The tests play the plugin.
    pub fn push(&self, event_type: &str, data: serde_json::Value) -> Reply {
        self.post(
            "/sync/push",
            serde_json::json!({ "version": "v1", "event_type": event_type, "data": data }),
        )
    }

    /// One message from the outbox, or None when nothing arrives in time.
    pub fn poll(&self, timeout: Duration) -> Option<serde_json::Value> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let reply = self.get("/sync/poll?batch=16");
            let body = reply.json();
            if !body.is_null() {
                return Some(body);
            }
        }
        None
    }

    /// Runs the CLI against this project, as a user would.
    pub fn cli(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_syncix-core"))
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("the CLI should run");
        let mut text = String::from_utf8_lossy(&out.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.code().unwrap_or(-1), strip_colour(&text))
    }

    pub fn sync_dir(&self) -> PathBuf {
        self.root.join("src")
    }

    /// Waits for a file to appear and returns its content.
    pub fn wait_for_file(&self, relative: &str) -> Option<String> {
        let path = self.sync_dir().join(relative);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(&path) {
                return Some(text);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    /// Waits until `check` is happy with /health, and returns that reply.
    pub fn wait_for_health(
        &self,
        check: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last = serde_json::Value::Null;
        while Instant::now() < deadline {
            last = self.get("/health").json();
            if check(&last) {
                return last;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        last
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        with_token: bool,
    ) -> std::io::Result<Reply> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let token_header = if with_token {
            format!("X-Syncix-Token: {}\r\n", self.token)
        } else {
            String::new()
        };
        let payload = body.unwrap_or("");
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {len}\r\n{token}\r\n{payload}",
            method = method,
            path = path,
            len = payload.len(),
            token = token_header,
            payload = payload
        );
        stream.write_all(request.as_bytes())?;
        stream.flush()?;

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        let text = String::from_utf8_lossy(&raw).to_string();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        let body = match text.find("\r\n\r\n") {
            Some(at) => text[at + 4..].to_string(),
            None => String::new(),
        };
        Ok(Reply { status, body })
    }
}

impl Drop for TestCore {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn wait_for_token(root: &Path) -> Option<String> {
    let path = root.join(".syncix").join("token");
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let token = text.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// The CLI writes colour for a terminal; in a test it is noise.
pub fn strip_colour(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        while let Some(next) = chars.next() {
            if next == 'm' {
                break;
            }
        }
    }
    out
}

/// A FULL_SYNC as the plugin sends one: a service with one script inside it.
pub fn full_sync_payload(service_id: &str, script_id: &str, source: &str) -> serde_json::Value {
    serde_json::json!({
        "place_key": "test-place",
        "place_name": "Test Place",
        "place_id": "0",
        "instances": [
            {
                "syncix_id": service_id,
                "class_name": "ServerScriptService",
                "name": "ServerScriptService",
                "parent": serde_json::Value::Null,
            },
            {
                "syncix_id": script_id,
                "class_name": "Script",
                "name": "Main",
                "parent": service_id,
                "source": source,
            }
        ]
    })
}
