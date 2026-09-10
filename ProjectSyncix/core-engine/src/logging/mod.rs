use std::fs;
use std::panic;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Enterprise logging initialiser
/// Writes logs both to the terminal and, non-blocking, to a file in JSON format.
pub fn init_enterprise_logging(log_dir: &str) -> Result<WorkerGuard, String> {
    // Create the log directory
    fs::create_dir_all(log_dir).map_err(|e| e.to_string())?;

    // Appender writing to a file (rotated daily)
    let file_appender = tracing_appender::rolling::daily(log_dir, "syncix.log");

    // Non-blocking writer (keeps disk IO from blocking the event bus)
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Terminal output (human-readable)
    let stdout_log = fmt::layer().pretty().with_target(false);

    // File output (JSON, for log analysis)
    let file_log = fmt::layer().json().with_writer(non_blocking);

    // Environment variable filter (RUST_LOG=info)
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Build the subscriber and install it
    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_log)
        .with(file_log)
        .init();

    setup_panic_hook(log_dir);

    info!("Logging system initialised.");

    // The guard must be returned so flushing is not cancelled before the main thread ends
    Ok(guard)
}

/// Crash report generator
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
            report.push_str("Error message: unknown\n");
        }

        if let Some(location) = panic_info.location() {
            report.push_str(&format!("Location: {}:{}\n", location.file(), location.line()));
        }

        // TODO: Backtrace can be added (RUST_BACKTRACE=1)

        let crash_file = log_dir.join("crash_report.txt");
        if let Err(e) = fs::write(&crash_file, report) {
            eprintln!("Could not write the crash report to disk: {}", e);
        } else {
            eprintln!("Crash report created: {:?}", crash_file);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_init_does_not_panic() {
        // Try starting with a temporary directory
        let temp_dir = std::env::temp_dir().join("syncix_test_logs");
        let _guard = init_enterprise_logging(temp_dir.to_str().unwrap());
        // Init can only run once; if no error comes back it succeeded.
    }
}
