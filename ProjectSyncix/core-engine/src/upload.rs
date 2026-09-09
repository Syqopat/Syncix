//! Roblox'a yayınlama (Open Cloud).
//!
//! UYARLAMA NOTU — Rojo'nun `upload` komutundan farkları:
//!
//! 1. ÇEREZ YOK. Rojo'nun eski sürümleri .ROBLOSECURITY çerezini kabul ediyordu;
//!    bu çerez hesabın TAMAMINA erişim verir ve sızarsa hesap gider. Burada
//!    yalnızca Open Cloud API anahtarı kabul edilir, o da yalnızca verdiğiniz
//!    izinlere sahiptir ve iptal edilebilir.
//!
//! 2. ANAHTAR PROJEDE TUTULMAZ. Anahtar yalnızca SYNCIX_API_KEY ortam
//!    değişkeninden okunur. syncix.toml'a yazılmasına izin verilmez; aksi halde
//!    ilk `git commit` ile herkese açık olurdu.
//!
//! 3. VARSAYILAN KURU ÇALIŞMA. Komut hiçbir şey yayınlamaz; ne yapacağını anlatır
//!    ve durur. Gerçekten yayınlamak için `--onayla` gerekir. Yayınlama geri
//!    alınamaz bir dış işlemdir, kazara tetiklenmemeli.
//!
//! 4. TLS için sistemdeki curl kullanılır. Yalnızca bu komut için projeye bir TLS
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
        let Ok(metin) = std::fs::read_to_string(root.join("syncix.toml")) else {
            return Self::default();
        };
        let Ok(v) = metin.parse::<toml::Value>() else {
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
/// dosya sürüm kontrolüne girer ve anahtar herkese açılır. Yayınlamayı
/// reddedip sebebini söylüyoruz.
pub fn anahtar_sizintisi_var_mi(root: &std::path::Path) -> bool {
    let Ok(metin) = std::fs::read_to_string(root.join("syncix.toml")) else {
        return false;
    };
    let alt = metin.to_lowercase();
    alt.contains("api_key") || alt.contains("apikey") || alt.contains("roblosecurity")
}

pub struct UploadPlan {
    pub universe_id: u64,
    pub place_id: u64,
    pub dosya: PathBuf,
    pub bayt: usize,
    pub obje_sayisi: usize,
    pub atlanan_enum: usize,
}

/// Yayınlama için kullanılacak Open Cloud uç noktası.
pub fn hedef_url(universe_id: u64, place_id: u64) -> String {
    format!(
        "https://apis.roblox.com/universes/v1/{}/places/{}/versions?versionType=Saved",
        universe_id, place_id
    )
}

/// Kullanıcının kendi çalıştırabilmesi için tam komut.
/// Anahtar ortam değişkeninden okunur; komutun içine gömülmez ki terminal
/// geçmişinde ve ekran görüntülerinde görünmesin.
pub fn curl_komutu(plan: &UploadPlan) -> String {
    format!(
        "curl -X POST \"{}\" \\\n  -H \"x-api-key: $SYNCIX_API_KEY\" \\\n  -H \"Content-Type: application/xml\" \\\n  --data-binary \"@{}\"",
        hedef_url(plan.universe_id, plan.place_id),
        plan.dosya.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hedef_url_open_cloud_bicimi() {
        let u = hedef_url(123, 456);
        assert!(u.starts_with("https://apis.roblox.com/universes/v1/123/places/456/versions"));
        assert!(u.contains("versionType=Saved"));
    }

    #[test]
    fn curl_komutu_anahtari_gommez() {
        let plan = UploadPlan {
            universe_id: 1,
            place_id: 2,
            dosya: PathBuf::from("x.rbxlx"),
            bayt: 10,
            obje_sayisi: 3,
            atlanan_enum: 0,
        };
        let k = curl_komutu(&plan);
        // Anahtar ortam degiskeni olarak gecmeli, duz metin olarak degil.
        assert!(k.contains("$SYNCIX_API_KEY"));
        assert!(!k.to_lowercase().contains("roblosecurity"));
    }

    #[test]
    fn syncix_toml_da_anahtar_varsa_yakalanir() {
        let gecici = std::env::temp_dir().join("syncix_upload_testi");
        let _ = std::fs::create_dir_all(&gecici);

        std::fs::write(gecici.join("syncix.toml"), "sync_dir = \"src\"\n").unwrap();
        assert!(!anahtar_sizintisi_var_mi(&gecici));

        std::fs::write(
            gecici.join("syncix.toml"),
            "sync_dir = \"src\"\n[upload]\napi_key = \"gizli\"\n",
        )
        .unwrap();
        assert!(anahtar_sizintisi_var_mi(&gecici));

        let _ = std::fs::remove_dir_all(&gecici);
    }

    #[test]
    fn upload_ayarlari_okunur() {
        let gecici = std::env::temp_dir().join("syncix_upload_ayar");
        let _ = std::fs::create_dir_all(&gecici);
        std::fs::write(
            gecici.join("syncix.toml"),
            "sync_dir = \"src\"\n[upload]\nuniverse_id = 10603832052\nplace_id = 122029079780221\n",
        )
        .unwrap();

        let cfg = UploadConfig::load(&gecici);
        assert_eq!(cfg.universe_id, Some(10603832052));
        assert_eq!(cfg.place_id, Some(122029079780221));

        let _ = std::fs::remove_dir_all(&gecici);
    }

    #[test]
    fn ayar_yoksa_bos_doner() {
        let cfg = UploadConfig::load(std::path::Path::new("/olmayan/klasor"));
        assert!(cfg.universe_id.is_none());
        assert!(cfg.place_id.is_none());
    }
}
