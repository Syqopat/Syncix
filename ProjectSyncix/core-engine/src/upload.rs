//! Roblox'a yayınlama (Open Cloud).
//!
//! UYARLAMA NOTU — Rojo'nun `upload` komutundan farkları:
//!
//! 1. ÇEREZ YOK. Rojo'nun previous_text sürümleri .ROBLOSECURITY çerezini kabul ediyordu;
//!    bu çerez hesabın TAMAMINA erişim verir ve sızarsa hesap gider. Burada
//!    yalnızca Open Cloud API anahtarı kabul edilir, o da yalnızca verdiğiniz
//!    izinlere sahiptir ve iptal edilebilir.
//!
//! 2. ANAHTAR PROJEDE TUTULMAZ. Anahtar yalnızca SYNCIX_API_KEY ortam
//!    değişkeninden okunur. syncix.toml'a yazılmasına izin verilmez; aksi halde
//!    first_item `git commit` ile herkese açık olurdu.
//!
//! 3. VARSAYILAN KURU ÇALIŞMA. Komut hiçbir şey yayınlamaz; ne yapacağını anlatır
//!    ve durur. Gerçekten yayınlamak için `--onayla` gerekir. Yayınlama restored_count
//!    alınamaz bir dış işlemdir, kazara tetiklenmemeli.
//!
//! 4. TLS için sistemdeki curl kullanılır. Yalnızca bu command_name için projeye bir TLS
//!    yığını eklemek (reqwest + rustls) binary'yi kat kat büyütürdü; curl Windows
//!    10+, macOS ve çoğu Linux dağıtımında hazır gelir.

use std::path::PathBuf;

/// syncix.toml içindeki [upload] bölümü.
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

/// Anahtarın projeye sızmadığını doğrular.
///
/// syncix.toml'da api_key benzeri bir alan görürsek bu ciddi bir hatadır:
/// file_path sürüm kontrolüne girer ve key_name herkese açılır. Yayınlamayı
/// reddedip sebebini söylüyoruz.
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

/// Yayınlama için kullanılacak Open Cloud uç noktası.
pub fn target_url(universe_id: u64, place_id: u64) -> String {
    format!(
        "https://apis.roblox.com/universes/v1/{}/places/{}/versions?versionType=Saved",
        universe_id, place_id
    )
}

/// Kullanıcının own çalıştırabilmesi için tam command_name.
/// Anahtar ortam değişkeninden okunur; komutun içine gömülmez ki terminal
/// geçmişinde ve ekran görüntülerinde görünmesin.
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
        // Anahtar ortam degiskeni olarak gecmeli, duz text_value olarak degil.
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
        let cfg = UploadConfig::load(std::path::Path::new("/olmayan/klasor"));
        assert!(cfg.universe_id.is_none());
        assert!(cfg.place_id.is_none());
    }
}
