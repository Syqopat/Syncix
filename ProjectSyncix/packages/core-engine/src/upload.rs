//! Publishing to Roblox (Open Cloud).
//!
//! ADAPTATION NOTE — how this differs from Rojo's `upload` command:
//!
//! 1. NO COOKIES. Older Rojo versions accepted the .ROBLOSECURITY cookie;
//!    that cookie grants access to the WHOLE account, and if it leaks the account is gone.
//!    Here only an Open Cloud API key is accepted, which has only the permissions
//!    you give it and can be revoked.
//!
//! 2. THE KEY NEVER LIVES IN THE PROJECT. It is read only from the SYNCIX_API_KEY
//!    environment variable. Writing it to syncix.toml is not allowed; otherwise
//!    the first `git commit` would make it public.
//!
//! 3. DRY RUN BY DEFAULT. The command publishes nothing; it explains what it would do
//!    and stops. Actually publishing requires an explicit confirmation flag. Publishing is
//!    an external action that cannot be undone and must not be triggered by accident.
//!
//! 4. The system's curl is used for TLS. Adding a TLS stack (reqwest + rustls) to
//!    the project for this one command would multiply the binary's size; curl ships
//!    with Windows 10+, macOS and most Linux distributions.

use std::path::PathBuf;

/// The [upload] section of syncix.toml.
#[derive(Debug, Clone, Default)]
pub struct UploadConfig {
    pub universe_id: Option<u64>,
    pub place_id: Option<u64>,
}

impl UploadConfig {
    pub fn load(root: &std::path::Path) -> Self {
        let Ok(text_value) = std::fs::read_to_string(root.join("syncix.toml")) else {
            return Self::default();
        };
        let Ok(v) = text_value.parse::<toml::Value>() else {
            return Self::default();
        };
        let Some(u) = v.get("upload") else {
            return Self::default();
        };
        Self {
            universe_id: u.get("universe_id").and_then(|x| x.as_integer()).map(|x| x as u64),
            place_id: u.get("place_id").and_then(|x| x.as_integer()).map(|x| x as u64),
        }
    }
}

/// Checks that the key has not leaked into the project.
///
/// An api_key-like field in syncix.toml is a serious mistake: the file goes into
/// version control and the key becomes public. Publishing is refused and the reason
/// is given.
pub fn has_key_leak(root: &std::path::Path) -> bool {
    let Ok(text_value) = std::fs::read_to_string(root.join("syncix.toml")) else {
        return false;
    };
    let sub = text_value.to_lowercase();
    sub.contains("api_key") || sub.contains("apikey") || sub.contains("roblosecurity")
}

pub struct UploadPlan {
    pub universe_id: u64,
    pub place_id: u64,
    pub file_path: PathBuf,
    pub byte_count: usize,
    pub object_total: usize,
    pub skipped_enums: usize,
}

/// Open Cloud endpoint used for publishing.
pub fn target_url(universe_id: u64, place_id: u64) -> String {
    format!(
        "https://apis.roblox.com/universes/v1/{}/places/{}/versions?versionType=Saved",
        universe_id, place_id
    )
}

/// The full command, so users can run it themselves.
/// The key is read from the environment variable, not embedded in the command, so it
/// does not show up in terminal history or screenshots.
pub fn curl_command(plan: &UploadPlan) -> String {
    format!(
        "curl -X POST \"{}\" \\\n  -H \"x-api-key: $SYNCIX_API_KEY\" \\\n  -H \"Content-Type: application/xml\" \\\n  --data-binary \"@{}\"",
        target_url(plan.universe_id, plan.place_id),
        plan.file_path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_url_uses_open_cloud_format() {
        let u = target_url(123, 456);
        assert!(u.starts_with("https://apis.roblox.com/universes/v1/123/places/456/versions"));
        assert!(u.contains("versionType=Saved"));
    }

    #[test]
    fn curl_command_does_not_embed_key() {
        let plan = UploadPlan {
            universe_id: 1,
            place_id: 2,
            file_path: PathBuf::from("x.rbxlx"),
            byte_count: 10,
            object_total: 3,
            skipped_enums: 0,
        };
        let k = curl_command(&plan);
        // The key must be passed as an environment variable, not as plain text.
        assert!(k.contains("$SYNCIX_API_KEY"));
        assert!(!k.to_lowercase().contains("roblosecurity"));
    }

    #[test]
    fn key_in_syncix_toml_is_caught() {
        let scratch_dir = std::env::temp_dir().join("syncix_upload_testi");
        let _ = std::fs::create_dir_all(&scratch_dir);

        std::fs::write(scratch_dir.join("syncix.toml"), "sync_dir = \"src\"\n").unwrap();
        assert!(!has_key_leak(&scratch_dir));

        std::fs::write(
            scratch_dir.join("syncix.toml"),
            "sync_dir = \"src\"\n[upload]\napi_key = \"gizli\"\n",
        )
        .unwrap();
        assert!(has_key_leak(&scratch_dir));

        let _ = std::fs::remove_dir_all(&scratch_dir);
    }

    #[test]
    fn upload_settings_are_read() {
        let scratch_dir = std::env::temp_dir().join("syncix_upload_ayar");
        let _ = std::fs::create_dir_all(&scratch_dir);
        std::fs::write(
            scratch_dir.join("syncix.toml"),
            "sync_dir = \"src\"\n[upload]\nuniverse_id = 10603832052\nplace_id = 122029079780221\n",
        )
        .unwrap();

        let cfg = UploadConfig::load(&scratch_dir);
        assert_eq!(cfg.universe_id, Some(10603832052));
        assert_eq!(cfg.place_id, Some(122029079780221));

        let _ = std::fs::remove_dir_all(&scratch_dir);
    }

    #[test]
    fn missing_setting_returns_empty() {
        let cfg = UploadConfig::load(std::path::Path::new("/missing/folder"));
        assert!(cfg.universe_id.is_none());
        assert!(cfg.place_id.is_none());
    }
}
