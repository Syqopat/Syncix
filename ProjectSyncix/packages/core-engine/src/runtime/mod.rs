//! Starting the core and keeping it running.
//!
//! main() only hands over: everything the running core is made of -- the log sink, the
//! project settings, the bound port, the disk writer, the file watcher, the HTTP server
//! and the message loop -- is wired here.

mod ctx;
mod inbox;
mod resend;

pub(crate) use ctx::Ctx;
pub(crate) use resend::resend_payloads;

use std::fs;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::server::AppState;
use crate::transport::{Payload, StudioOutbox};
use crate::scheduler::{Job, JobScheduler};
use crate::{api, layout, model, project, server, sourcemap, file_sync};

/// Runs the core until it is stopped.
pub async fn run(cli_args: Vec<String>) {
    // Log filter: silence the DEBUG noise of dependencies like hyper/tower,
    // while Syncix's own events show down to DEBUG level.
    // The RUST_LOG environment variable can override it.
    // Logs go to both the console and a file, because the core can be started in the
    // background by VS Code, where its console is not visible. The file is the project's
    // own .syncix/syncix-core.log, in the folder holding syncix.toml, found the way
    // ProjectConfig::load finds it (the core runs from the project root, from its .syncix
    // folder or from core-engine/). It used to be ../syncix-core.log: right from .syncix,
    // but a core started in the project root wrote beside the project, and every project
    // in a folder (GAMES/) shared one log.
    use tracing_subscriber::prelude::*;
    let project_dir = if std::path::Path::new("../syncix.toml").exists() { ".." } else { "." };
    let log_dir = std::path::Path::new(project_dir).join(".syncix");
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::never(&log_dir, "syncix-core.log");
    let (file_writer, _log_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,syncix_core=debug")),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer),
        )
        .init();

    info!(
        "Starting Syncix Core Engine... (version {}, protocol {})",
        project::VERSION,
        project::PROTOCOL_VERSION
    );

    // Project configuration: sync folder, port and project identity.
    // An explicit port such as `syncix serve 25565` takes precedence over the setting.
    let mut cfg_raw = project::ProjectConfig::load();
    if let Some(p) = cli_args
        .get(1)
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|_| cli_args.first().map(|s| s.as_str()) == Some("serve"))
    {
        info!("Port requested on the command line: {}", p);
        cfg_raw.wanted_port = p;
        // An explicitly requested port is FIXED: if it is taken, the core does not move to the next one.
        // Otherwise the port typed into the Studio plugin and the core's port would diverge.
        cfg_raw.port_fixed = true;
    }
    let cfg = Arc::new(cfg_raw);
    let sync_dir: &'static str = Box::leak(cfg.sync_dir.clone().into_boxed_str());
    info!("Project: {} ({})", cfg.name, cfg.root.display());
    info!("Sync folder: {}", sync_dir);
    fs::create_dir_all(sync_dir).unwrap();

    // Bind the port NOW: the real port has to go into AppState, because /health
    // reports it and that is how the Studio plugin and the editor find the core.
    let Some((listener, actual_port)) = server::bind_with_fallback(&cfg) else {
        error!("Syncix could not start: no port available.");
        std::process::exit(1);
    };
    cfg.write_port_file(actual_port);

    // 1. Start the central systems
    let data_model = model::create_shared_model();

    // Transport (HTTP) channels
    // Messages to Studio are queued (Outbox) for lossless delivery.
    let studio_outbox = Arc::new(StudioOutbox::new());
    let (tx_to_core, rx_from_studio) = mpsc::channel::<Payload>(100);

    // VS Code RPC channel
    let (tx_to_vscode, _) = broadcast::channel::<String>(100);

    let health_monitor = Arc::new(crate::health::HealthMonitor::new());

    // Heavy blocking work (the full-tree write) runs here instead of on an async worker.
    let jobs = Arc::new(JobScheduler::new(cfg.job_workers));

    let app_state = Arc::new(AppState {
        studio_outbox: studio_outbox.clone(),
        tx_to_core,
        tx_to_vscode,
        health_monitor: health_monitor.clone(),
        data_model: data_model.clone(),
        chaos_mode_enabled: false, // should normally come from the config
        project: cfg.clone(),
        actual_port,
        access_token: api::auth::ensure_token(&cfg.root.to_string_lossy()),
        place_clash_state: Arc::new(std::sync::Mutex::new(None)),
        job_stats: jobs.stats(),
    });

    // 2. Disk writer (debounced): triggered whenever the model changes; after a short quiet
    // period it writes the whole tree to disk as an exact copy of Studio's Explorer.
    // Debouncing prevents a storm of disk writes during fast changes such as dragging.
    // NOTE: WRITING to disk is this task's job alone; file_sync only reads.
    let disk_notify = Arc::new(tokio::sync::Notify::new());
    // Until Studio completes a FULL_SYNC in this session the model is NOT the
    // authority over the disk. Until then the writer may write but may not delete any file;
    // reconciling against an empty model moved the whole sync folder to the trash.
    let model_authoritative = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let model_authoritative_writer = model_authoritative.clone();
        let data_model_for_writer = data_model.clone();
        let notify_for_writer = disk_notify.clone();
        let cfg_for_writer = cfg.clone();
        let jobs_for_writer = jobs.clone();
        tokio::spawn(async move {
            loop {
                notify_for_writer.notified().await;
                // Wait for quiet (merge changes arriving back to back)
                loop {
                    tokio::select! {
                        _ = notify_for_writer.notified() => continue,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(
                            cfg_for_writer.debounce_ms,
                        )) => break,
                    }
                }
                // Do NOT touch the disk while suspended. The reconciler treats the model as the truth;
                // while suspended the model may be incomplete or wrong, so bringing the disk
                // in line with it would mean deleting files.
                if crate::project::is_sync_suspended() {
                    continue;
                }
                // The guard is taken as an owned one so the whole write can move onto the
                // job pool: writing thousands of files used to hold an async worker
                // thread, and polls from Studio waited behind the disk.
                let dm = data_model_for_writer.clone().read_owned().await;
                let allow_removal =
                    model_authoritative_writer.load(std::sync::atomic::Ordering::SeqCst);
                let cfg_for_job = cfg_for_writer.clone();
                let write = Job::new("disk-write", move || {
                    layout::write_full_tree(&dm, sync_dir, &cfg_for_job.ignore, allow_removal);

                    // sourcemap.json: tells luau-lsp which file maps to which place in the
                    // DataModel, so it can offer autocompletion. Refreshed whenever the
                    // tree changes; no separate watcher process is needed.
                    if cfg_for_job.sourcemap {
                        let file_content = sourcemap::json(&dm, sync_dir, &cfg_for_job.root);
                        let dest = cfg_for_job.sourcemap_file();
                        let is_same = std::fs::read_to_string(&dest)
                            .map(|m| m == file_content)
                            .unwrap_or(false);
                        if !is_same {
                            if let Err(e) = std::fs::write(&dest, file_content) {
                                tracing::warn!("Could not write sourcemap.json: {}", e);
                            }
                        }
                    }
                });
                // Waiting keeps the writes in order: the next one cannot start before this
                // tree is on disk.
                if let Err(err) = jobs_for_writer.run(write).await {
                    tracing::warn!("The tree was not written: {}", err);
                }
            }
        });
    }

    // 2b. Start the file watcher (reads changes on disk; NEVER writes to disk).
    // It also receives the channels for editor notifications and disk refreshes.
    let outbox_for_watcher = studio_outbox.clone();
    let data_model_for_watcher = data_model.clone();
    let vscode_for_watcher = app_state.tx_to_vscode.clone();
    let notify_for_watcher = disk_notify.clone();
    let ignore_for_watcher = cfg.ignore.clone();
    let cfg_for_watcher = cfg.clone();
    tokio::spawn(async move {
        file_sync::start_watcher(
            outbox_for_watcher,
            sync_dir,
            data_model_for_watcher,
            vscode_for_watcher,
            notify_for_watcher,
            ignore_for_watcher,
            (*cfg_for_watcher).clone(),
        )
        .await;
    });

    // 3. Start the HTTP transport layer (talks to Studio)
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        server::start_server(state_clone, listener).await;
    });

    // Do not leave a stale port file on exit: otherwise the editor tries to connect to a dead port.
    {
        let cfg_for_exit = cfg.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cfg_for_exit.clear_port_file();
                info!("Syncix Core shut down.");
                std::process::exit(0);
            }
        });
    }

    let _cfg_for_loop = cfg.clone();
    // Show the settings actually applied at startup; the cheapest way to settle
    // "I set it but it did nothing".
    tracing::info!(
        "Sync mode: {} | debounce: {} ms | trash: {} (keep {}) | undo: {}",
        cfg.mode_value.name_of(),
        cfg.debounce_ms,
        cfg.safety_settings.trash_enabled,
        cfg.safety_settings.trash_keep_runs,
        cfg.restore_cmd
    );
    layout::configure_trash(cfg.safety_settings.trash_enabled, cfg.safety_settings.trash_keep_runs);
    layout::configure_meta(cfg.meta_files);

    inbox::serve(Ctx {
        state: app_state.clone(),
        data_model: data_model.clone(),
        studio_outbox: studio_outbox.clone(),
        cfg: cfg.clone(),
        disk_notify: disk_notify.clone(),
        model_authoritative: model_authoritative.clone(),
    }, rx_from_studio)
    .await;
}
