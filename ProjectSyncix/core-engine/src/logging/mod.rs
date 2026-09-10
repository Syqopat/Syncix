use std::fs;
use std::panic;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Kurumsal Loglama Başlatıcı (Enterprise Logging)
/// Logları hem terminale hem de JSON formatında non-blocking olarak dosyaya yazar.
pub fn init_enterprise_logging(log_dir: &str) -> Result<WorkerGuard, String> {
    // Log dizinini oluştur
    fs::create_dir_all(log_dir).map_err(|e| e.to_string())?;

    // Dosyaya yazan appender (Günlük olarak döner)
    let file_appender = tracing_appender::rolling::daily(log_dir, "syncix.log");

    // Non-blocking writer (Disk IO'nun Event Bus'ı tıkamasını önler)
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Terminal Çıktısı (Okunabilir)
    let stdout_log = fmt::layer().pretty().with_target(false);

    // Dosya Çıktısı (JSON formatında - Log analizi için)
    let file_log = fmt::layer().json().with_writer(non_blocking);

    // Çevresel değişken filtresi (RUST_LOG=info)
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Subscriber oluştur ve persist
    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_log)
        .with(file_log)
        .init();

    setup_panic_hook(log_dir);

    info!("Logging system initialised.");

    // Guard döndürülmeli ki main thread bitene kadar flush işlemi iptal olmasın
    Ok(guard)
}

/// Çökme Raporlayıcı (Crash Report Generator)
fn setup_panic_hook(log_dir: &str) {
    let log_dir = PathBuf::from(log_dir);

    panic::set_hook(Box::new(move |panic_info| {
        error!("PANIC detected: {:?}", panic_info);

        let mut report = String::new();
        report.push_str("=== SYNCIX CRASH REPORT ===\n");

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            report.push_str(&format!("Error message: {}\n", s));
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            report.push_str(&format!("Error message: {}\n", s));
        } else {
            report.push_str("Hata Mesajı: Bilinmiyor\n");
        }

        if let Some(location) = panic_info.location() {
            report.push_str(&format!("Konum: {}:{}\n", location.file(), location.line()));
        }

        // TODO: Backtrace eklenebilir (RUST_BACKTRACE=1)

        let crash_file = log_dir.join("crash_report.txt");
        if let Err(e) = fs::write(&crash_file, report) {
            eprintln!("Crash raporu diske yazılamadı: {}", e);
        } else {
            eprintln!("Çökme raporu oluşturuldu: {:?}", crash_file);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_init_does_not_panic() {
        // Temp bir directory ile başlatmayı dene
        let temp_dir = std::env::temp_dir().join("syncix_test_logs");
        let _guard = init_enterprise_logging(temp_dir.to_str().unwrap());
        // Init bir kere yapılabilir, eğer report_error almazsak başarılıdır.
    }
}
