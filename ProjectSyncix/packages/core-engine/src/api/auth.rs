use std::io::Write;
use std::sync::Arc;

use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::api::errors::ApiError;
use crate::server::AppState;

pub const HEADER: &str = "x-syncix-token";
const FILE_NAME: &str = "token";

/// Path of the token file inside a project.
pub fn token_path(root: &str) -> std::path::PathBuf {
    std::path::Path::new(root).join(".syncix").join(FILE_NAME)
}

/// Reads the project's token, writing a fresh one when there is none.
///
/// The server is bound to 127.0.0.1, which stops nothing: any web page the user has
/// open could POST to the port and, before this, stop the core or change the tree.
/// A page cannot read this file and cannot read the core's replies either (no CORS
/// header is ever sent), so requiring the token closes that door while leaving every
/// local client -- CLI, editor extension, Studio plugin -- able to get it.
pub fn ensure_token(root: &str) -> String {
    let path = token_path(root);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 32 {
            return existing;
        }
    }

    let token = new_token();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::File::create(&path) {
        Ok(mut file) => {
            let _ = file.write_all(token.as_bytes());
            restrict(&path);
        }
        Err(e) => tracing::warn!("Could not write the token file ({}): {}", path.display(), e),
    }
    token
}

fn new_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..48)
        .map(|_| {
            let n: u8 = rng.gen_range(0..16);
            std::char::from_digit(n as u32, 16).unwrap_or('0')
        })
        .collect()
}

#[cfg(unix)]
fn restrict(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &std::path::Path) {}

fn presented<B>(request: &Request<B>) -> Option<String> {
    if let Some(value) = request.headers().get(HEADER) {
        return value.to_str().ok().map(|s| s.trim().to_string());
    }
    // A WebSocket client cannot always set headers, so /rpc may carry ?token=.
    request.uri().query().and_then(|q| {
        q.split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == "token")
            .map(|(_, value)| value.to_string())
    })
}

/// Rejects a request that does not carry the project's token.
pub async fn require_token<B>(
    State(state): State<Arc<AppState>>,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, ApiError> {
    match presented(&request) {
        Some(token) if token == state.access_token => Ok(next.run(request).await),
        Some(_) => Err(ApiError::Unauthorized(
            "The token is wrong. Read it from .syncix/token or from /health.".to_string(),
        )),
        None => Err(ApiError::Unauthorized(format!(
            "This call needs the {} header. Read the token from .syncix/token or from /health.",
            HEADER
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_long_and_hexadecimal() {
        let token = new_token();
        assert_eq!(token.len(), 48);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_same_project_keeps_its_token() {
        let dir = std::env::temp_dir().join(format!("syncix-token-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = dir.to_string_lossy().to_string();
        let first = ensure_token(&root);
        let second = ensure_token(&root);
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
