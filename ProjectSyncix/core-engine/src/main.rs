#[allow(dead_code)]
mod assets;
#[allow(dead_code)]
mod auth;
#[allow(dead_code)]
mod bus;
mod cli;
#[allow(dead_code)]
mod command;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod contract;
mod file_sync;
mod health;
mod layout;
mod localization;
#[allow(dead_code)]
mod logging;
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod pipeline;
mod project;
mod rbxmx;
mod rbxmx_import;
mod sourcemap;
mod upload;
#[allow(dead_code)]
mod scheduler;
#[allow(dead_code)]
mod schema;
#[allow(dead_code)]
mod serializers;
mod server;
#[allow(dead_code)]
mod snapshot;
#[allow(dead_code)]
mod transport;
#[allow(dead_code)]
mod tree_builder;
#[allow(dead_code)]
mod vfs;

use std::fs;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::bus::EventBus;
use crate::model::InstanceNode;
use crate::server::AppState;
use crate::transport::{EventType, Payload, StudioOutbox};

/// Resolves a command target: full UUID, short UUID prefix or name.
/// If a name matches more than one object, None is returned because it is ambiguous.
fn resolve_id(dm: &model::DataModel, target: &str) -> Option<uuid::Uuid> {
    match dm.resolve_target(target) {
        model::ResolveResult::One(u) => Some(u),
        model::ResolveResult::NotFound => None,
        model::ResolveResult::Ambiguous(candidates) => {
            tracing::warn!(
                "{} instances match the name '{}'; the request was ambiguous so nothing was done. Use the short UUID.",
                target,
                candidates.len()
            );
            None
        }
    }
}

/// Turns a string value from the CLI into the matching PropertyValue.
/// "true"/"false" -> Boolean, "x,y,z" -> Vector3, number -> Number, otherwise -> String.
fn parse_property_value(s: &str) -> model::PropertyValue {
    use model::PropertyValue;
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        return PropertyValue::Boolean(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return PropertyValue::Boolean(false);
    }

    // Hex colour: "#ff8800" or "ff8800" -> Color3
    // (It used to reach Studio as plain text and be rejected.)
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) && t.starts_with('#') {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return PropertyValue::Color3 {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
            };
        }
    }
    let parts: Vec<&str> = t.split(',').collect();
    if parts.len() == 3 {
        if let (Ok(x), Ok(y), Ok(z)) = (
            parts[0].trim().parse::<f32>(),
            parts[1].trim().parse::<f32>(),
            parts[2].trim().parse::<f32>(),
        ) {
            return PropertyValue::Vector3 { x, y, z };
        }
    }
    if let Ok(n) = t.parse::<f64>() {
        return PropertyValue::Number(n);
    }
    PropertyValue::String(t.to_string())
}

/// Fits text typed in the terminal to the type of the property's CURRENT value in the
/// model. If it cannot, returns None and the caller falls back to the general parser.
///
/// Why it exists: `syncix set <part> BrickColor "Really red"` produced plain text and
/// the assignment silently failed in Studio. The same problem existed for CFrame, UDim2,
/// NumberRange and similar types — they could not be set from the terminal at all.
fn coerce_to_existing_type(current_value: &model::PropertyValue, text_value: &str) -> Option<model::PropertyValue> {
    use model::PropertyValue as P;
    let t = text_value.trim();

    /// "1, 2, 3" -> [1.0, 2.0, 3.0]; None if anything is not a number.
    fn numbers(t: &str) -> Option<Vec<f32>> {
        t.split(',')
            .map(|p| p.trim().parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()
    }

    match current_value {
        P::BrickColor(_) => Some(P::BrickColor(t.to_string())),
        // A Ref target may be a UUID or a short handle; resolution happens on the Studio side.
        P::Ref(_) => Some(P::Ref(t.to_string())),
        P::String(_) => Some(P::String(t.to_string())),
        // Asset ids are written as text: "rbxassetid://123".
        P::Content(_) => Some(P::Content(t.to_string())),
        P::Vector2 { .. } => match numbers(t)?[..] {
            [x, y] => Some(P::Vector2 { x, y }),
            _ => None,
        },
        P::UDim { .. } => match numbers(t)?[..] {
            [scale, offset] => Some(P::UDim { scale, offset }),
            _ => None,
        },
        P::NumberRange { .. } => match numbers(t)?[..] {
            [min, max] => Some(P::NumberRange { min, max }),
            // A single number pins the range to that point.
            [single] => Some(P::NumberRange { min: single, max: single }),
            _ => None,
        },
        P::UDim2 { .. } => match numbers(t)?[..] {
            [xs, xo, ys, yo] => Some(P::UDim2 { xs, xo, ys, yo }),
            _ => None,
        },
        P::CFrame { rot, .. } => {
            let s = numbers(t)?;
            match s.len() {
                // Only a position was given: the current rotation is kept. This is the most common request.
                3 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: *rot,
                }),
                12 => Some(P::CFrame {
                    pos: [s[0], s[1], s[2]],
                    rot: [s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11]],
                }),
                _ => None,
            }
        }
        // For Color3 both "#ff8800" and "1,0.5,0" are valid.
        P::Color3 { .. } => {
            if let P::Color3 { r, g, b } = parse_property_value(t) {
                return Some(P::Color3 { r, g, b });
            }
            match numbers(t)?[..] {
                [r, g, b] => Some(P::Color3 { r, g, b }),
                _ => None,
            }
        }
        P::Rect { .. } => match numbers(t)?[..] {
            [x0, y0, x1, y1] => Some(P::Rect {
                min: [x0, y0],
                max: [x1, y1],
            }),
            _ => None,
        },
        P::PhysicalProperties { .. } => match numbers(t)?[..] {
            [d, f, e, fw, ew] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: fw,
                elasticity_weight: ew,
            }),
            // Three values is the most common form; the weights stay at Roblox's defaults.
            [d, f, e] => Some(P::PhysicalProperties {
                density: d,
                friction: f,
                elasticity: e,
                friction_weight: 1.0,
                elasticity_weight: 1.0,
            }),
            _ => None,
        },
        // Curve types cannot be typed point by point in the terminal; the given values
        // are spread at EQUAL intervals along the time axis. "1,0" = fade out from start to end.
        P::NumberSequence(_) => {
            let v = numbers(t)?;
            if v.is_empty() {
                return None;
            }
            let last_item = (v.len() - 1).max(1) as f32;
            Some(P::NumberSequence(
                v.iter()
                    .enumerate()
                    .map(|(i, raw_value)| model::NumberKeypoint {
                        t: i as f32 / last_item,
                        v: *raw_value,
                        envelope: 0.0,
                    })
                    .collect(),
            ))
        }
        // Like "#ff0000,#0000ff": colours are spread at equal intervals.
        P::ColorSequence(_) => {
            let mut points = Vec::new();
            let pieces: Vec<&str> = t.split(',').map(|x| x.trim()).collect();
            let last_item = (pieces.len().saturating_sub(1)).max(1) as f32;
            for (i, piece) in pieces.iter().enumerate() {
                match parse_property_value(piece) {
                    P::Color3 { r, g, b } => points.push(model::ColorKeypoint {
                        t: i as f32 / last_item,
                        r,
                        g,
                        b,
                    }),
                    // If even one is not a colour the whole value is rejected: a curve applied
                    // halfway is more confusing than one not applied at all.
                    _ => return None,
                }
            }
            (!points.is_empty()).then_some(P::ColorSequence(points))
        }
        // Only the family changes; weight and style keep their current values.
        P::Font { weight, style, .. } => Some(P::Font {
            family: t.to_string(),
            weight: weight.clone(),
            style: style.clone(),
        }),
        // The rest (Vector3, Number, Boolean) already come out right in the general parser.
        _ => None,
    }
}

/// Turns a JSON value in wire format into a PropertyValue.
/// Accepts raw scalars (5, "hi", true), {Vector3:{..}}/{Color3:{..}} tables
/// and the serde enum form ({"Number":5}).
fn parse_wire_value(v: &serde_json::Value) -> Option<model::PropertyValue> {
    use model::PropertyValue;
    match v {
        serde_json::Value::String(s) => Some(PropertyValue::String(s.clone())),
        serde_json::Value::Bool(b) => Some(PropertyValue::Boolean(*b)),
        serde_json::Value::Number(n) => n.as_f64().map(PropertyValue::Number),
        serde_json::Value::Object(o) => {
            if let Some(v3) = o.get("Vector3") {
                Some(PropertyValue::Vector3 {
                    x: v3.get("x")?.as_f64()? as f32,
                    y: v3.get("y")?.as_f64()? as f32,
                    z: v3.get("z")?.as_f64()? as f32,
                })
            } else if let Some(c3) = o.get("Color3") {
                Some(PropertyValue::Color3 {
                    r: c3.get("r")?.as_f64()? as f32,
                    g: c3.get("g")?.as_f64()? as f32,
                    b: c3.get("b")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim2") {
                Some(PropertyValue::UDim2 {
                    xs: u.get("xs")?.as_f64()? as f32,
                    xo: u.get("xo")?.as_f64()? as f32,
                    ys: u.get("ys")?.as_f64()? as f32,
                    yo: u.get("yo")?.as_f64()? as f32,
                })
            } else if let Some(v) = o.get("Vector2") {
                Some(PropertyValue::Vector2 {
                    x: v.get("x")?.as_f64()? as f32,
                    y: v.get("y")?.as_f64()? as f32,
                })
            } else if let Some(u) = o.get("UDim") {
                Some(PropertyValue::UDim {
                    scale: u.get("scale")?.as_f64()? as f32,
                    offset: u.get("offset")?.as_f64()? as f32,
                })
            } else if let Some(c) = o.get("CFrame") {
                let pos = c.get("pos")?.as_array()?;
                let rot = c.get("rot")?.as_array()?;
                if pos.len() != 3 || rot.len() != 9 {
                    return None;
                }
                let mut p = [0f32; 3];
                let mut r = [0f32; 9];
                for (i, v) in pos.iter().enumerate() {
                    p[i] = v.as_f64()? as f32;
                }
                for (i, v) in rot.iter().enumerate() {
                    r[i] = v.as_f64()? as f32;
                }
                Some(PropertyValue::CFrame { pos: p, rot: r })
            } else if let Some(n) = o.get("NumberRange") {
                Some(PropertyValue::NumberRange {
                    min: n.get("min")?.as_f64()? as f32,
                    max: n.get("max")?.as_f64()? as f32,
                })
            } else if let Some(r) = o.get("Ref") {
                Some(PropertyValue::Ref(r.as_str()?.to_string()))
            } else if let Some(b) = o.get("BrickColor") {
                Some(PropertyValue::BrickColor(b.as_str()?.to_string()))
            } else if let Some(c) = o.get("Content") {
                Some(PropertyValue::Content(c.as_str()?.to_string()))
            } else if let Some(a) = o.get("ColorSequence") {
                let mut points = Vec::new();
                for k in a.as_array()? {
                    points.push(model::ColorKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        r: k.get("r")?.as_f64()? as f32,
                        g: k.get("g")?.as_f64()? as f32,
                        b: k.get("b")?.as_f64()? as f32,
                    });
                }
                Some(PropertyValue::ColorSequence(points))
            } else if let Some(a) = o.get("NumberSequence") {
                let mut points = Vec::new();
                for k in a.as_array()? {
                    points.push(model::NumberKeypoint {
                        t: k.get("t")?.as_f64()? as f32,
                        v: k.get("v")?.as_f64()? as f32,
                        envelope: k.get("envelope").and_then(|e| e.as_f64()).unwrap_or(0.0) as f32,
                    });
                }
                Some(PropertyValue::NumberSequence(points))
            } else if let Some(r) = o.get("Rect") {
                let mn = r.get("min")?.as_array()?;
                let mx = r.get("max")?.as_array()?;
                if mn.len() != 2 || mx.len() != 2 {
                    return None;
                }
                Some(PropertyValue::Rect {
                    min: [mn[0].as_f64()? as f32, mn[1].as_f64()? as f32],
                    max: [mx[0].as_f64()? as f32, mx[1].as_f64()? as f32],
                })
            } else if let Some(f) = o.get("Font") {
                Some(PropertyValue::Font {
                    family: f.get("family")?.as_str()?.to_string(),
                    weight: f.get("weight")?.as_str()?.to_string(),
                    style: f.get("style")?.as_str()?.to_string(),
                })
            } else if let Some(pp) = o.get("PhysicalProperties") {
                Some(PropertyValue::PhysicalProperties {
                    density: pp.get("density")?.as_f64()? as f32,
                    friction: pp.get("friction")?.as_f64()? as f32,
                    elasticity: pp.get("elasticity")?.as_f64()? as f32,
                    friction_weight: pp.get("frictionWeight")?.as_f64()? as f32,
                    elasticity_weight: pp.get("elasticityWeight")?.as_f64()? as f32,
                })
            } else {
                serde_json::from_value(v.clone()).ok()
            }
        }
        _ => None,
    }
}

/// Converts a PropertyValue into the wire format the Studio plugin (PatchExecutor) expects.
/// Scalars are sent raw; Vector3/Color3 are wrapped in tables.
pub fn pv_to_wire(pv: &model::PropertyValue) -> serde_json::Value {
    use model::PropertyValue;
    match pv {
        PropertyValue::String(s) => serde_json::json!(s),
        PropertyValue::Number(n) => serde_json::json!(n),
        PropertyValue::Boolean(b) => serde_json::json!(b),
        PropertyValue::Vector3 { x, y, z } => serde_json::json!({ "Vector3": { "x": x, "y": y, "z": z } }),
        PropertyValue::Color3 { r, g, b } => serde_json::json!({ "Color3": { "r": r, "g": g, "b": b } }),
        PropertyValue::UDim2 { xs, xo, ys, yo } => {
            serde_json::json!({ "UDim2": { "xs": xs, "xo": xo, "ys": ys, "yo": yo } })
        }
        PropertyValue::Vector2 { x, y } => serde_json::json!({ "Vector2": { "x": x, "y": y } }),
        PropertyValue::UDim { scale, offset } => {
            serde_json::json!({ "UDim": { "scale": scale, "offset": offset } })
        }
        PropertyValue::CFrame { pos, rot } => {
            serde_json::json!({ "CFrame": { "pos": pos, "rot": rot } })
        }
        PropertyValue::NumberRange { min, max } => {
            serde_json::json!({ "NumberRange": { "min": min, "max": max } })
        }
        PropertyValue::Ref(id) => serde_json::json!({ "Ref": id }),
        PropertyValue::BrickColor(item_name) => serde_json::json!({ "BrickColor": item_name }),
        PropertyValue::Content(u) => serde_json::json!({ "Content": u }),
        PropertyValue::ColorSequence(k) => serde_json::json!({ "ColorSequence": k }),
        PropertyValue::NumberSequence(k) => serde_json::json!({ "NumberSequence": k }),
        PropertyValue::Rect { min, max } => {
            serde_json::json!({ "Rect": { "min": min, "max": max } })
        }
        PropertyValue::Font {
            family,
            weight,
            style,
        } => serde_json::json!({
            "Font": { "family": family, "weight": weight, "style": style }
        }),
        PropertyValue::PhysicalProperties {
            density,
            friction,
            elasticity,
            friction_weight,
            elasticity_weight,
        } => serde_json::json!({
            "PhysicalProperties": {
                "density": density,
                "friction": friction,
                "elasticity": elasticity,
                "frictionWeight": friction_weight,
                "elasticityWeight": elasticity_weight
            }
        }),
    }
}

#[tokio::main]
async fn main() {
    // CLI mode: with arguments, act as a client and do not start a server.
    // The same binary is both server and CLI, so there is no PowerShell/Node dependency.
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(script_code) = cli::execute_run(&cli_args) {
        std::process::exit(script_code);
    }

    // Log filter: silence the DEBUG noise of dependencies like hyper/tower,
    // while Syncix's own events show down to DEBUG level.
    // The RUST_LOG environment variable can override it.
    // Logs go to both the console and a file (../syncix-core.log), because the core
    // can be started in the background by VS Code, where its console is not visible.
    use tracing_subscriber::prelude::*;
    let file_appender = tracing_appender::rolling::never("../", "syncix-core.log");
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
    let _event_bus = Arc::new(EventBus::new());
    let data_model = model::create_shared_model();

    // Transport (HTTP) channels
    // Messages to Studio are queued (Outbox) for lossless delivery.
    let studio_outbox = Arc::new(StudioOutbox::new());
    let (tx_to_core, mut rx_from_studio) = mpsc::channel::<Payload>(100);

    // VS Code RPC channel
    let (tx_to_vscode, _) = broadcast::channel::<String>(100);

    let health_monitor = Arc::new(crate::health::HealthMonitor::new());

    let app_state = Arc::new(AppState {
        studio_outbox: studio_outbox.clone(),
        tx_to_core,
        tx_to_vscode,
        health_monitor: health_monitor.clone(),
        data_model: data_model.clone(),
        chaos_mode_enabled: false, // should normally come from the config
        project: cfg.clone(),
        actual_port,
        place_clash_state: Arc::new(std::sync::Mutex::new(None)),
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
                let dm = data_model_for_writer.read().await;
                let allow_removal =
                    model_authoritative_writer.load(std::sync::atomic::Ordering::SeqCst);
                layout::write_full_tree(&dm, sync_dir, &cfg_for_writer.ignore, allow_removal);

                // sourcemap.json: tells luau-lsp which file maps to which place in the DataModel,
                // so it can offer autocompletion.
                // Refreshed whenever the tree changes; no separate watcher process is needed.
                if cfg_for_writer.sourcemap {
                    let file_content = sourcemap::json(&dm, sync_dir, &cfg_for_writer.root);
                    let dest = cfg_for_writer.sourcemap_file();
                    let is_same = std::fs::read_to_string(&dest)
                        .map(|m| m == file_content)
                        .unwrap_or(false);
                    if !is_same {
                        if let Err(e) = std::fs::write(&dest, file_content) {
                            tracing::warn!("Could not write sourcemap.json: {}", e);
                        }
                    }
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

    let cfg_for_loop = cfg.clone();
    // Show the settings actually applied at startup; the cheapest way to settle
    // "I set it but it did nothing".
    tracing::info!(
        "Sync mode: {} | play: {} | debounce: {} ms | trash: {} (keep {}) | undo: {}",
        cfg.mode_value.name_of(),
        cfg.play.name_of(),
        cfg.debounce_ms,
        cfg.safety_settings.trash_enabled,
        cfg.safety_settings.trash_keep_runs,
        cfg.restore_cmd
    );
    layout::configure_trash(cfg.safety_settings.trash_enabled, cfg.safety_settings.trash_keep_runs);
    layout::configure_meta(cfg.meta_files);

    // 4. Message dispatcher (listens to the event bus and routes)
    // The core does not know the transport; it only reads Payloads from the channel.
    while let Some(payload) = rx_from_studio.recv().await {
        // With the Studio -> disk direction off, no change coming from Studio
        // is applied to the model. The one exception is FULL_SYNC: even in disk_to_studio mode
        // the core has to know Studio's UUIDs, otherwise it cannot find which object
        // to write to.
        if !cfg_for_loop.mode_value.accepts_from_studio() && payload.event_type != EventType::FullSync {
            continue;
        }

        if payload.event_type == EventType::FullSync {
            // PLACE IDENTITY GATE
            //
            // A sync folder belongs to ONE place. When another place connected to the same
            // folder, the two trees used to merge silently:
            // service UUIDs are the same in every place, so both
            // landed on the same skeleton and SINGLETON objects such as StarterPlayerScripts
            // were duplicated. On top of that, old files on disk were taken for "new objects"
            // and created inside the new place.
            //
            // We no longer merge: if a different place arrives we stop and
            // leave the decision to the user (syncix bind).
            let incoming_place = payload
                .data
                .get("place_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !incoming_place.is_empty() {
                match cfg_for_loop.linked_place() {
                    // The folder is empty or being bound for the first time: claim it.
                    None => {
                        cfg_for_loop.bind_place(&incoming_place);
                        tracing::info!("This folder is now bound to the connected place.");
                    }
                    Some(current_value) if current_value == incoming_place => {
                        // Same place, no problem.
                        if let Ok(mut c) = app_state.place_clash_state.lock() {
                            *c = None;
                        }
                    }
                    Some(current_value) => {
                        let item_name = payload
                            .data
                            .get("place_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let pid = payload
                            .data
                            .get("place_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .to_string();

                        tracing::error!(
                            "This folder belongs to a different place. Sync is on hold so nothing gets mixed.
                               folder is bound to : {}
                               place connecting   : {} (\"{}\", id {})
                               Decide with: syncix bind --studio  (write this place into the folder)
                               or:          syncix bind --disk    (load the folder into this place)",
                            current_value, incoming_place, item_name, pid
                        );

                        if let Ok(mut c) = app_state.place_clash_state.lock() {
                            *c = Some(crate::server::PlaceConflict {
                                folder_place: current_value,
                                incoming_place,
                                incoming_name: item_name,
                                incoming_place_id: pid,
                            });
                        }
                        // Do NOT touch the model and stop every direction: until a decision is made
                        // both sides must stay as they are.
                        crate::project::set_sync_suspended(true);
                        continue;
                    }
                }
            }

            tracing::info!("Received FULL_SYNC (bootstrap) from Studio. Synchronising state...");
            if let Some(instances) = payload.data.get("instances").and_then(|p| p.as_array()) {
                let mut added_count = 0;
                let mut ws_nodes = Vec::new();
                
                {
                    let mut dm = data_model.write().await;
                    // Recovery: when a FULL_SYNC arrives the old state is discarded completely.
                    // That way objects deleted on the Studio side do not linger in memory.
                    *dm = crate::model::DataModel::new();
                    for node_data in instances {
                        if let (Some(class_name), Some(name), Some(syncix_id)) = (
                            node_data.get("class_name").and_then(|v| v.as_str()),
                            node_data.get("name").and_then(|v| v.as_str()),
                            node_data.get("syncix_id").and_then(|v| v.as_str()),
                        ) {
                            // Classes the user excluded never enter the model.
                            //
                            // The plugin applies the same filter; this second gate is there so the
                            // setting still applies when an older plugin connects. If either
                            // side failed to apply the setting, you would get
                            // "I excluded it but it still comes".
                            if !cfg_for_loop.class_allowed(class_name) {
                                continue;
                            }
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let mut instance = InstanceNode::new(class_name, name);
                                instance.syncix_id = uuid;
                                
                                // Parent ID logic
                                if let Some(parent_str) = node_data.get("parent").and_then(|v| v.as_str()) {
                                    if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                        instance.parent = Some(parent_uuid);
                                    }
                                }

                                // Script source code
                                if let Some(src) = node_data.get("source").and_then(|v| v.as_str()) {
                                    instance.source = Some(src.to_string());
                                }

                                // Attributes
                                if let Some(attrs) = node_data.get("attributes").and_then(|v| v.as_object()) {
                                    for (k, val) in attrs {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.attributes.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // CollectionService tags
                                if let Some(t) = node_data.get("tags").and_then(|v| v.as_array()) {
                                    instance.tags = t
                                        .iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect();
                                }

                                // Properties (wide scope — generic properties object)
                                if let Some(props) = node_data.get("properties").and_then(|v| v.as_object()) {
                                    for (k, val) in props {
                                        if !cfg_for_loop.property_allowed(k) {
                                            continue;
                                        }
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.properties.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                if let Err(e) = dm.upsert_instance(instance.clone()) {
                                    tracing::warn!("FULL_SYNC upsert failed ({}): {}", instance.name, e);
                                    continue;
                                }
                                added_count += 1;
                                // Disk writing is done by the central debounced writer (layout).

                                ws_nodes.push(serde_json::json!({
                                    "id": instance.syncix_id,
                                    "name": instance.name,
                                    "className": instance.class_name,
                                    "parentId": instance.parent.map(|u| u.to_string()),
                                    "childrenIds": [],
                                    "isExpanded": false
                                }));
                            }
                        }
                    }
                }
                
                tracing::info!("FULL_SYNC complete. {} instances added or updated.", added_count);
                // The model is now Studio's tree: the reconciler may delete extras.
                model_authoritative.store(true, std::sync::atomic::Ordering::SeqCst);
                
                // Notify VS Code with FULL_SYNC
                let ws_msg = serde_json::json!({
                    "event_type": "FULL_SYNC",
                    "data": {
                        "nodes": ws_nodes
                    }
                });
                let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
            }
        } else if payload.event_type == EventType::GetTree {
            tracing::info!("GET_TREE requested. Sending state...");
            let mut ws_nodes = Vec::new();
            {
                let dm = data_model.read().await;
                for instance in dm.get_all_instances().values() {
                    // Do not send the internal root "Game" (DataModel) node out
                    if instance.class_name == "DataModel" {
                        continue;
                    }
                    ws_nodes.push(serde_json::json!({
                        "id": instance.syncix_id,
                        "name": instance.name,
                        "className": instance.class_name,
                        "parentId": instance.parent.map(|u| u.to_string()),
                        "childrenIds": [],
                        "isExpanded": false
                    }));
                }
            }
            
            let ws_msg = serde_json::json!({
                "event_type": "FULL_SYNC",
                "data": {
                    "nodes": ws_nodes
                }
            });
            let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
        } else if payload.event_type == EventType::RenameInstance {
            // VS Code -> Studio: rename
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = payload.data.get("newName").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };
            if new_name.is_empty() {
                tracing::warn!("RENAME_INSTANCE: newName was empty, ignored.");
            } else if let Some(uuid) = resolved {
                let mut updated_instance = None;
                let mut old_name = None;
                {
                    let mut dm = data_model.write().await;
                    if let Some(instance) = dm.get_mut_instance(&uuid) {
                        if instance.parent.is_none() {
                            tracing::warn!("RENAME_INSTANCE: services cannot be renamed ({}).", instance.name);
                            continue;
                        }
                        old_name = Some(instance.name.clone());
                        instance.name = new_name.to_string();
                        instance.last_updated = chrono::Utc::now().timestamp_millis();
                        updated_instance = Some(instance.clone());
                    }
                }
                if let Some(instance) = updated_instance {
                    let _ = &old_name;
                    // Disk writing is done by the central debounced writer (layout).
                    // Apply to Studio
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "PROPERTY_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "property": "Name",
                                    "value": instance.name
                                }
                            }]
                        }),
                    });
                    // Reflect in the VS Code Explorer
                    let ws_msg = serde_json::json!({
                        "event_type": "INSTANCE_UPDATED",
                        "data": {
                            "id": instance.syncix_id,
                            "name": instance.name,
                            "className": instance.class_name,
                            "parentId": instance.parent.map(|u| u.to_string()),
                            "childrenIds": instance.children,
                            "isExpanded": false
                        }
                    });
                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    tracing::info!(
                        "VS Code RENAME handled: {} -> {}",
                        old_name.unwrap_or_default(),
                        instance.name
                    );
                } else {
                    tracing::warn!("RENAME_INSTANCE: unknown UUID: {}", id);
                }
            } else {
                tracing::warn!("RENAME_INSTANCE target not found: {}", id);
            }
        } else if payload.event_type == EventType::CreateInstance {
            // VS Code -> Studio: create a new instance.
            // The UUID is generated only here, at CREATE time (data integrity rule).
            let class_name = payload.data.get("className").and_then(|v| v.as_str()).unwrap_or("");
            let parent_id = payload.data.get("parentId").and_then(|v| v.as_str()).unwrap_or("");
            // Without a given name the class name is used (the old behaviour is kept).
            let node_name = payload
                .data
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(class_name);
            if class_name.is_empty() {
                tracing::warn!("CREATE_INSTANCE: className was empty, ignored.");
            } else if rbxmx_import::is_singleton(class_name) {
                // Studio cannot create a service or a singleton container; a model entry
                // for one would be a phantom that also makes the real one's name ambiguous.
                tracing::warn!("CREATE_INSTANCE: {} exists once per place and cannot be created, ignored.", class_name);
            } else {
                let mut instance = InstanceNode::new(class_name, node_name);
                // If the client supplied a UUID, use it; that lets an import target the object
                // it created by identity rather than by name.
                if let Some(given) = payload
                    .data
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                {
                    instance.syncix_id = given;
                }
                {
                    // Parent resolution: UUID or name (e.g. "Workspace").
                    // When empty it attaches to the Workspace service by default.
                    let dm = data_model.read().await;
                    let effective_parent = if parent_id.is_empty() { "Workspace" } else { parent_id };
                    instance.parent = resolve_id(&dm, effective_parent);
                }
                let uuid = instance.syncix_id;
                let insert_result = {
                    let mut dm = data_model.write().await;
                    dm.upsert_instance(instance.clone())
                };
                match insert_result {
                    Ok(()) => {
                        // Disk writing is done by the central debounced writer (layout).
                        // Send CREATE to Studio
                        studio_outbox.push(Payload {
                            version: "v1".to_string(),
                            event_type: EventType::CompositeUpdate,
                            data: serde_json::json!({
                                "patches": [{
                                    "event_type": "CREATE",
                                    "data": {
                                        "syncix_id": uuid,
                                        "class_name": instance.class_name,
                                        "name": instance.name,
                                        "parent": instance.parent.map(|u| u.to_string())
                                    }
                                }]
                            }),
                        });
                        // Reflect in the VS Code Explorer
                        let ws_msg = serde_json::json!({
                            "event_type": "INSTANCE_CREATED",
                            "data": {
                                "id": uuid,
                                "name": instance.name,
                                "className": instance.class_name,
                                "parentId": instance.parent.map(|u| u.to_string()),
                                "childrenIds": [],
                                "isExpanded": false
                            }
                        });
                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                        tracing::info!("CREATE handled: {} ({})", instance.name, instance.class_name);
                    }
                    Err(e) => tracing::warn!("CREATE_INSTANCE upsert failed: {}", e),
                }
            }
        } else if payload.event_type == EventType::DeleteInstance {
            // VS Code -> Studio: delete
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };
            if let Some(uuid) = resolved {
                // Service nodes cannot be deleted
                let is_service = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
                };
                if is_service {
                    tracing::warn!("DELETE_INSTANCE: services cannot be deleted ({}).", id);
                    continue;
                }
                let removed_instance = {
                    let mut dm = data_model.write().await;
                    dm.remove_instance(&uuid)
                };
                if let Some(instance) = removed_instance {
                    // Disk writing is done by the central debounced writer (layout).
                    // Send DESTROY to Studio
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "DESTROY",
                                "data": { "syncix_id": uuid }
                            }]
                        }),
                    });
                    // Reflect in the VS Code Explorer
                    let ws_msg = serde_json::json!({
                        "event_type": "INSTANCE_REMOVED",
                        "data": {
                            "id": uuid,
                            "parentId": instance.parent.map(|u| u.to_string())
                        }
                    });
                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    tracing::info!("DELETE handled: {}", instance.name);
                } else {
                    tracing::warn!("DELETE_INSTANCE: unknown UUID: {}", id);
                }
            } else {
                tracing::warn!("DELETE_INSTANCE target not found: {}", id);
            }
        } else if payload.event_type == EventType::ReparentInstance {
            // VS Code/CLI -> Studio: move
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let new_parent_field = payload
                .data
                .get("newParentId")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let (resolved_id, resolved_parent) = {
                let dm = data_model.read().await;
                (resolve_id(&dm, id), resolve_id(&dm, new_parent_field))
            };

            if let (Some(uuid), Some(new_parent)) = (resolved_id, resolved_parent) {
                if uuid == new_parent {
                    tracing::warn!("REPARENT_INSTANCE: an instance cannot be moved into itself.");
                    continue;
                }
                // Service nodes cannot be moved
                let is_service = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid).map(|i| i.parent.is_none()).unwrap_or(false)
                };
                if is_service {
                    tracing::warn!("REPARENT_INSTANCE: services cannot be moved ({}).", id);
                    continue;
                }

                let result = {
                    let mut dm = data_model.write().await;
                    dm.reparent(&uuid, Some(new_parent))
                };

                match result {
                    Ok((old_parent, _)) => {
                        let instance = {
                            let dm = data_model.read().await;
                            dm.get_instance(&uuid).cloned()
                        };
                        if let Some(instance) = instance {
                            // Disk writing is done by the central debounced writer (layout).
                            // Send REPARENT to Studio
                            studio_outbox.push(Payload {
                                version: "v1".to_string(),
                                event_type: EventType::CompositeUpdate,
                                data: serde_json::json!({
                                    "patches": [{
                                        "event_type": "REPARENT",
                                        "data": {
                                            "syncix_id": uuid,
                                            "parent": new_parent.to_string()
                                        }
                                    }]
                                }),
                            });
                            // Reflect in the VS Code Explorer
                            let ws_msg = serde_json::json!({
                                "event_type": "INSTANCE_MOVED",
                                "data": {
                                    "id": uuid,
                                    "oldParentId": old_parent.map(|u| u.to_string()),
                                    "newParentId": new_parent.to_string()
                                }
                            });
                            let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                            tracing::info!("REPARENT handled: {} -> {}", instance.name, new_parent);
                        }
                    }
                    Err(e) => tracing::warn!("REPARENT_INSTANCE failed: {}", e),
                }
            } else {
                tracing::warn!("REPARENT_INSTANCE target or parent not found (id={}, parent={})", id, new_parent_field);
            }
        } else if payload.event_type == EventType::SetProperty {
            // VS Code/CLI -> Studio: set a property
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let property = payload
                .data
                .get("property")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // The value may come as a string from the CLI ("0.5", "true", "1,2,3") or typed
            // from the Inspector ({"Color3":{...}}, number, bool).
            let value_json = payload
                .data
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let value_str = value_json
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| value_json.to_string());

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if property.is_empty() {
                tracing::warn!("SET_PROPERTY: the property name was empty.");
            } else if let Some(uuid) = resolved {
                // A value from the terminal is plain text; it carries no type information.
                // So it is first fitted to the type of the property's CURRENT
                // value: `set X BrickColor "Really red"` becomes a BrickColor, not text,
                // `set X CFrame 0,10,0` becomes a CFrame, not text.
                // The type decision is still made from a value — the value compared
                // is simply the one already in the model.
                let current_value = {
                    let dm = data_model.read().await;
                    dm.get_instance(&uuid)
                        .and_then(|i| i.properties.get(&property).cloned())
                };
                let mut pv = match &value_json {
                    serde_json::Value::String(s) => current_value
                        .as_ref()
                        .and_then(|m| coerce_to_existing_type(m, s))
                        .unwrap_or_else(|| parse_property_value(s)),
                    other => parse_wire_value(other)
                        .unwrap_or_else(|| model::PropertyValue::String(value_str.clone())),
                };
                // Without a current value there is no type to fit to, and "x,y,z" parses as a
                // Vector3, which Studio refuses for a CFrame. A CFrame it is, unrotated.
                if property == "CFrame" {
                    if let model::PropertyValue::Vector3 { x, y, z } = pv {
                        pv = model::PropertyValue::CFrame {
                            pos: [x, y, z],
                            rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                        };
                    }
                }

                // The reference target is resolved to a full UUID here. The user may type a short
                // handle or a name ("syncix set door Part0 hinge"),
                // but the cache in Studio is looked up by full UUID only;
                // sent unresolved, the reference would silently stay nil.
                if let model::PropertyValue::Ref(dest) = &pv {
                    if !dest.is_empty() {
                        let resolved_n = {
                            let dm = data_model.read().await;
                            resolve_id(&dm, dest)
                        };
                        match resolved_n {
                            Some(u) => pv = model::PropertyValue::Ref(u.to_string()),
                            None => {
                                tracing::warn!(
                                    "SET_PROPERTY: reference target '{}' was not found; the property was left unchanged.",
                                    dest
                                );
                                return;
                            }
                        }
                    }
                }
                let mut ok = false;
                {
                    let mut dm = data_model.write().await;
                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                        if property == "Name" {
                            inst.name = value_str.clone();
                        } else {
                            inst.properties.insert(property.clone(), pv.clone());
                        }
                        inst.last_updated = chrono::Utc::now().timestamp_millis();
                        ok = true;
                    }
                }

                if ok {
                    // Apply in Studio
                    let wire_value = if property == "Name" {
                        serde_json::json!(value_str)
                    } else {
                        pv_to_wire(&pv)
                    };
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "PROPERTY_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "property": property,
                                    "value": wire_value
                                }
                            }]
                        }),
                    });
                    // Notify the VS Code Explorer (the name may have changed)
                    let instance = {
                        let dm = data_model.read().await;
                        dm.get_instance(&uuid).cloned()
                    };
                    if let Some(instance) = instance {
                        let ws_msg = serde_json::json!({
                            "event_type": "INSTANCE_UPDATED",
                            "data": {
                                "id": instance.syncix_id,
                                "name": instance.name,
                                "className": instance.class_name,
                                "parentId": instance.parent.map(|u| u.to_string()),
                                "childrenIds": instance.children,
                                "isExpanded": false
                            }
                        });
                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                    }
                    tracing::info!("SET_PROPERTY handled: {} .{} = {}", id, property, value_str);
                } else {
                    tracing::warn!("SET_PROPERTY: unknown UUID: {}", id);
                }
            } else {
                tracing::warn!("SET_PROPERTY target not found: {}", id);
            }
        } else if payload.event_type == EventType::Selection {
            // Selection is a two-way MIRROR: an object clicked in Studio is selected in the editor,
            // an object clicked in the editor is selected in Studio.
            //
            // It is not written to the model because selection is not project content but momentary
            // state. Written to disk, every click would be a file change and
            // version control would drown in noise.
            let identities: Vec<String> = payload
                .data
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // The source travels with the message to stop it bouncing back: sending a selection
            // that came from Studio back to Studio would be an endless ping-pong.
            let origin = payload
                .data
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("studio");

            if origin == "studio" {
                let ws = serde_json::json!({
                    "event_type": "SELECTION",
                    "data": { "ids": identities }
                });
                let _ = app_state.tx_to_vscode.send(ws.to_string());
            } else {
                // Came from the editor: apply in Studio.
                let already_resolved: Vec<String> = {
                    let dm = data_model.read().await;
                    identities
                        .iter()
                        .filter_map(|h| resolve_id(&dm, h).map(|u| u.to_string()))
                        .collect()
                };
                studio_outbox.push(Payload {
                    version: "v1".to_string(),
                    event_type: EventType::CompositeUpdate,
                    data: serde_json::json!({
                        "patches": [{
                            "event_type": "SELECTION_UPDATE",
                            "data": { "ids": already_resolved }
                        }]
                    }),
                });
            }
        } else if payload.event_type == EventType::SetTags {
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let mut tag_list: Vec<String> = payload
                .data
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            // Sorted and deduplicated: both sides see the same list in the same order,
            // otherwise every comparison says "changed" and produces needless patches.
            tag_list.sort();
            tag_list.dedup();

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if let Some(uuid) = resolved {
                let was_applied = {
                    let mut dm = data_model.write().await;
                    match dm.get_mut_instance(&uuid) {
                        Some(inst) => {
                            inst.tags = tag_list.clone();
                            inst.last_updated = chrono::Utc::now().timestamp_millis();
                            true
                        }
                        None => false,
                    }
                };
                if was_applied {
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "TAGS_UPDATE",
                                "data": { "syncix_id": uuid, "tags": tag_list }
                            }]
                        }),
                    });
                    tracing::info!("SET_TAGS handled: {} -> {:?}", id, tag_list);
                }
            } else {
                tracing::warn!("SET_TAGS target not found: {}", id);
            }
        } else if payload.event_type == EventType::SetAttribute {
            // VS Code/CLI -> Studio: set an attribute
            let id = payload.data.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = payload
                .data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // A JSON null value field means this is a DELETE request.
            // On the Studio side SetAttribute(name, nil) removes the attribute; the same meaning
            // travels as null on the wire, so no separate event type is needed.
            let deletion = payload
                .data
                .get("value")
                .map(|v| v.is_null())
                .unwrap_or(true);
            let value_str = payload
                .data
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let resolved = {
                let dm = data_model.read().await;
                resolve_id(&dm, id)
            };

            if name.is_empty() {
                tracing::warn!("SET_ATTRIBUTE: the attribute name was empty.");
            } else if let Some(uuid) = resolved {
                let pv = parse_property_value(&value_str);
                let mut ok = false;
                {
                    let mut dm = data_model.write().await;
                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                        if deletion {
                            inst.attributes.remove(&name);
                        } else {
                            inst.attributes.insert(name.clone(), pv.clone());
                        }
                        inst.last_updated = chrono::Utc::now().timestamp_millis();
                        ok = true;
                    }
                }
                if ok {
                    studio_outbox.push(Payload {
                        version: "v1".to_string(),
                        event_type: EventType::CompositeUpdate,
                        data: serde_json::json!({
                            "patches": [{
                                "event_type": "ATTRIBUTE_UPDATE",
                                "data": {
                                    "syncix_id": uuid,
                                    "name": name,
                                    "value": if deletion { serde_json::Value::Null } else { pv_to_wire(&pv) }
                                }
                            }]
                        }),
                    });
                    tracing::info!("SET_ATTRIBUTE handled: {} @{} = {}", name, id, value_str);
                } else {
                    tracing::warn!("SET_ATTRIBUTE for an unknown UUID: {}", id);
                }
            } else {
                tracing::warn!("SET_ATTRIBUTE target not found: {}", id);
            }
        } else if payload.event_type == EventType::CompositeUpdate {
            if let Some(patches) = payload.data.get("patches").and_then(|p| p.as_array()) {
                for patch in patches {
                    let p_type = patch.get("event_type").and_then(|e| e.as_str()).unwrap_or("");
                    if p_type == "CREATE" {
                        // Extract basic info
                        if let (Some(_data), Some(class_name), Some(name), Some(syncix_id)) = (
                            patch.get("data"),
                            patch.get("data").and_then(|d| d.get("class_name")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("name")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                        ) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let mut instance = InstanceNode::new(class_name, name);
                                instance.syncix_id = uuid;

                                // Parent UUID (for the hierarchy)
                                if let Some(parent_str) = patch
                                    .get("data")
                                    .and_then(|d| d.get("parent"))
                                    .and_then(|v| v.as_str())
                                {
                                    if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_str) {
                                        instance.parent = Some(parent_uuid);
                                    }
                                }

                                // Script source code
                                if let Some(src) = patch
                                    .get("data")
                                    .and_then(|d| d.get("source"))
                                    .and_then(|v| v.as_str())
                                {
                                    instance.source = Some(src.to_string());
                                }

                                // Attributes
                                if let Some(attrs) = patch
                                    .get("data")
                                    .and_then(|d| d.get("attributes"))
                                    .and_then(|v| v.as_object())
                                {
                                    for (k, val) in attrs {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.attributes.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // Properties (wide scope)
                                if let Some(props) = patch
                                    .get("data")
                                    .and_then(|d| d.get("properties"))
                                    .and_then(|v| v.as_object())
                                {
                                    for (k, val) in props {
                                        if let Some(pv) = parse_wire_value(val) {
                                            instance.properties.insert(k.clone(), pv);
                                        }
                                    }
                                }

                                // Save to DataModel
                                {
                                    let mut dm = data_model.write().await;
                                    if let Err(e) = dm.upsert_instance(instance.clone()) {
                                        tracing::warn!("CREATE upsert failed ({}): {}", instance.name, e);
                                    }
                                }
                                
                                // Disk writing is done by the central debounced writer (layout).

                                // Notify VS Code (for every class, with or without a serializer)
                                let ws_msg = serde_json::json!({
                                    "event_type": "INSTANCE_CREATED",
                                    "data": {
                                        "id": instance.syncix_id,
                                        "name": instance.name,
                                        "className": instance.class_name,
                                        "parentId": instance.parent.map(|u| u.to_string()),
                                        "childrenIds": instance.children,
                                        "isExpanded": false
                                    }
                                });
                                let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                tracing::info!(
                                    "Studio CREATE handled: {} ({})",
                                    instance.name,
                                    instance.class_name
                                );
                            }
                        }
                    } else if p_type == "PROPERTY_UPDATE" {
                        if let (Some(_data), Some(syncix_id), Some(property), Some(value)) = (
                            patch.get("data"),
                            patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("property")).and_then(|v| v.as_str()),
                            patch.get("data").and_then(|d| d.get("value")),
                        ) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                // Echo loop detection: if the same property bounces back and forth many times in a short
                                // time, echo suppression missed it. This used to be
                                // completely silent; now the property involved is logged.
                                if health_monitor
                                    .loop_detector
                                    .persist(syncix_id, property)
                                {
                                    tracing::warn!(
                                        "Suspected echo loop: {} .{} was updated many times in a short window. Studio and the core may be writing the same value back and forth.",
                                        syncix_id,
                                        property
                                    );
                                }

                                let mut updated_instance = None;
                                let mut old_name = None;

                                {
                                    let mut dm = data_model.write().await;
                                    if let Some(instance) = dm.get_mut_instance(&uuid) {
                                        old_name = Some(instance.name.clone());
                                        
                                        if property == "Name" {
                                            if let Some(new_name) = value.as_str() {
                                                instance.name = new_name.to_string();
                                            }
                                        } else if property == "Source" {
                                            if let Some(src) = value.as_str() {
                                                instance.source = Some(src.to_string());
                                            }
                                        } else if let Some(pv) = parse_wire_value(value) {
                                            instance.properties.insert(property.to_string(), pv);
                                        }
                                        updated_instance = Some(instance.clone());
                                    }
                                }
                                
                                if let Some(instance) = updated_instance {
                                    let _ = &old_name;
                                    // Disk writing is done by the central debounced writer (layout).

                                    // Notify VS Code (for every class)
                                    let ws_msg = serde_json::json!({
                                        "event_type": "INSTANCE_UPDATED",
                                        "data": {
                                            "id": instance.syncix_id,
                                            "name": instance.name,
                                            "className": instance.class_name,
                                            "parentId": instance.parent.map(|u| u.to_string()),
                                            "childrenIds": instance.children,
                                            "isExpanded": false
                                        }
                                    });
                                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                    tracing::info!(
                                        "Studio PROPERTY_UPDATE handled: {} -> {}",
                                        instance.name,
                                        property
                                    );
                                } else {
                                    tracing::warn!(
                                        "PROPERTY_UPDATE ignored for an unknown UUID: {} (property: {}). FULL_SYNC may not have arrived yet.",
                                        uuid,
                                        property
                                    );
                                }
                            }
                        }
                    } else if p_type == "DESTROY" {
                        if let Some(syncix_id) = patch.get("data").and_then(|d| d.get("syncix_id")).and_then(|v| v.as_str()) {
                            if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                                let removed_instance = {
                                    let mut dm = data_model.write().await;
                                    dm.remove_instance(&uuid)
                                };
                                
                                if let Some(instance) = removed_instance {
                                    // Disk writing is done by the central debounced writer (layout).
                                    let ws_msg = serde_json::json!({
                                        "event_type": "INSTANCE_REMOVED",
                                        "data": {
                                            "id": uuid,
                                            "parentId": instance.parent.map(|u| u.to_string())
                                        }
                                    });
                                    let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                    tracing::info!("Studio DESTROY handled: {}", instance.name);
                                } else {
                                    tracing::warn!(
                                        "DESTROY ignored for an unknown UUID: {}",
                                        uuid
                                    );
                                }
                            }
                        }
                    } else if p_type == "REPARENT" {
                        // The object was moved to another parent in Studio.
                        let syncix_id = patch
                            .get("data")
                            .and_then(|d| d.get("syncix_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let new_parent_str = patch
                            .get("data")
                            .and_then(|d| d.get("parent"))
                            .and_then(|v| v.as_str());

                        if let Ok(uuid) = uuid::Uuid::parse_str(syncix_id) {
                            let new_parent = new_parent_str
                                .and_then(|s| uuid::Uuid::parse_str(s).ok());

                            let result = {
                                let mut dm = data_model.write().await;
                                dm.reparent(&uuid, new_parent)
                            };

                            match result {
                                Ok((old_parent, _)) => {
                                    let instance = {
                                        let dm = data_model.read().await;
                                        dm.get_instance(&uuid).cloned()
                                    };
                                    if let Some(instance) = instance {
                                        // Disk writing is done by the central debounced writer (layout).
                                        // Report the move to the VS Code Explorer
                                        let ws_msg = serde_json::json!({
                                            "event_type": "INSTANCE_MOVED",
                                            "data": {
                                                "id": uuid,
                                                "oldParentId": old_parent.map(|u| u.to_string()),
                                                "newParentId": new_parent.map(|u| u.to_string())
                                            }
                                        });
                                        let _ = app_state.tx_to_vscode.send(ws_msg.to_string());
                                        tracing::info!(
                                            "Studio REPARENT handled: {} -> parent {}",
                                            instance.name,
                                            new_parent.map(|u| u.to_string()).unwrap_or_else(|| "none".to_string())
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("REPARENT failed ({}): {}", uuid, e);
                                }
                            }
                        }
                    } else if p_type == "ATTRIBUTE_UPDATE" {
                        // An attribute changed in Studio.
                        let syncix_id = patch
                            .get("data")
                            .and_then(|d| d.get("syncix_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = patch
                            .get("data")
                            .and_then(|d| d.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let value = patch.get("data").and_then(|d| d.get("value"));

                        if let (Ok(uuid), false, Some(val)) =
                            (uuid::Uuid::parse_str(syncix_id), name.is_empty(), value)
                        {
                            if let Some(pv) = parse_wire_value(val) {
                                let mut applied = false;
                                {
                                    let mut dm = data_model.write().await;
                                    if let Some(inst) = dm.get_mut_instance(&uuid) {
                                        inst.attributes.insert(name.to_string(), pv);
                                        applied = true;
                                    }
                                }
                                if applied {
                                    tracing::info!("Studio ATTRIBUTE_UPDATE handled: {} @{}", name, syncix_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // The model may have changed after any event; notify the debounced writer so the
        // disk (the mirror of Studio's Explorer) is updated.
        disk_notify.notify_one();
    }
}

// ===========================================================================
// PROPERTY TYPE CONVERSION TESTS
//
// Why here: both real bugs in this project went through exactly this layer.
//   1. The Position bug — Vector3 values reached Studio but the wire format
//      was misread and silently dropped; every object stayed at 0,0,0.
//   2. The colour bug — "#5aa832" went as a plain String and Studio rejected it.
// Neither could have been caught by model tests; the tree logic was flawless.
// These tests LOCK the wire format: if the format changes, a test breaks.
// ===========================================================================
#[cfg(test)]
mod property_tests {
    use super::*;
    use model::PropertyValue;

    /// Wire-format round trip: pv -> wire -> pv must give the same value.
    /// If a type is lost in this loop, sync silently loses data.
    #[test]
    fn wire_format_round_trip_all_types() {
        let samples = vec![
            PropertyValue::String("Hello".into()),
            PropertyValue::Number(42.5),
            PropertyValue::Boolean(true),
            PropertyValue::Boolean(false),
            PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 },
            PropertyValue::Color3 { r: 0.35, g: 0.66, b: 0.2 },
            PropertyValue::UDim2 { xs: 0.5, xo: 10.0, ys: 1.0, yo: -4.0 },
        ];

        for sample in samples {
            let wire = pv_to_wire(&sample);
            let restored_count = parse_wire_value(&wire)
                .unwrap_or_else(|| panic!("could not parse wire value: {:?} -> {}", sample, wire));
            assert_eq!(restored_count, sample, "round-trip broken: {}", wire);
        }
    }

    /// Vector3 must NEVER be plain text on the wire.
    /// Sent as plain text, Studio rejects it with "Vector3 expected" and
    /// the object stays at 0,0,0 — exactly what the Position bug did.
    #[test]
    fn vector3_goes_as_table_on_wire() {
        let wire = pv_to_wire(&PropertyValue::Vector3 { x: 1.0, y: 2.0, z: 3.0 });
        assert!(wire.is_object(), "Vector3 must be a table, not a plain value: {}", wire);
        assert!(wire.get("Vector3").is_some(), "Vector3 anahtari bulunmali: {}", wire);
        assert_eq!(wire["Vector3"]["y"].as_f64().unwrap(), 2.0);
    }

    #[test]
    fn color3_goes_as_table_on_wire() {
        let wire = pv_to_wire(&PropertyValue::Color3 { r: 1.0, g: 0.0, b: 0.5 });
        assert!(wire.get("Color3").is_some(), "Color3 anahtari bulunmali: {}", wire);
    }

    /// Text from the CLI and HTTP must be converted to the right type.
    /// If "0,0.5,-60" stays a String the position is not applied; this is the input side of the bug.
    #[test]
    fn text_parses_as_vector3() {
        assert_eq!(
            parse_property_value("0,0.5,-60"),
            PropertyValue::Vector3 { x: 0.0, y: 0.5, z: -60.0 }
        );
        // Negative values actually used in the map
        assert_eq!(
            parse_property_value("-20, 0.5, -6"),
            PropertyValue::Vector3 { x: -20.0, y: 0.5, z: -6.0 }
        );
        // The zero vector must not be read as a Number
        assert_eq!(
            parse_property_value("0,0,0"),
            PropertyValue::Vector3 { x: 0.0, y: 0.0, z: 0.0 }
        );
    }

    /// The hex colour bug: if "#5aa832" stays a String, Studio rejects it.
    #[test]
    fn hex_color_parses_as_color3() {
        match parse_property_value("#5aa832") {
            PropertyValue::Color3 { r, g, b } => {
                assert_eq!((r * 255.0).round() as u8, 0x5a);
                assert_eq!((g * 255.0).round() as u8, 0xa8);
                assert_eq!((b * 255.0).round() as u8, 0x32);
            }
            other => panic!("a hex colour must become Color3, got: {:?}", other),
        }
    }

    /// Without a hash it is not hex: "abcdef" could be a name, "123456" is a number.
    /// Without this distinction, name fields would accidentally turn into colours.
    #[test]
    fn text_without_hash_is_not_a_color() {
        assert_eq!(parse_property_value("abcdef"), PropertyValue::String("abcdef".into()));
        assert_eq!(parse_property_value("123456"), PropertyValue::Number(123456.0));
    }

    #[test]
    fn scalar_types_parse_correctly() {
        assert_eq!(parse_property_value("true"), PropertyValue::Boolean(true));
        assert_eq!(parse_property_value("False"), PropertyValue::Boolean(false));
        assert_eq!(parse_property_value("5"), PropertyValue::Number(5.0));
        assert_eq!(parse_property_value("-3.5"), PropertyValue::Number(-3.5));
        assert_eq!(parse_property_value("Box"), PropertyValue::String("Box".into()));
    }

    /// Enum values must pass as text; the Studio side does the resolving.
    /// If they turned into a number or another type here, properties such as Material/Font
    /// would silently not apply.
    #[test]
    fn enum_text_passes_through() {
        assert_eq!(
            parse_property_value("Enum.Material.Neon"),
            PropertyValue::String("Enum.Material.Neon".into())
        );
        let wire = pv_to_wire(&PropertyValue::String("Enum.Material.Neon".into()));
        assert_eq!(wire.as_str(), Some("Enum.Material.Neon"));
    }

    /// Studio may send a raw scalar or the serde enum form.
    /// If either were not accepted, updates coming from Studio would be dropped.
    #[test]
    fn raw_and_serde_forms_are_accepted() {
        assert_eq!(
            parse_wire_value(&serde_json::json!(7)),
            Some(PropertyValue::Number(7.0))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!("text")),
            Some(PropertyValue::String("text".into()))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!(true)),
            Some(PropertyValue::Boolean(true))
        );
        assert_eq!(
            parse_wire_value(&serde_json::json!({"Number": 3.0})),
            Some(PropertyValue::Number(3.0))
        );
    }
}

#[cfg(test)]
mod type_coercion_tests {
    use super::*;
    use model::PropertyValue as P;

    /// This was the real bug: `set <part> BrickColor "Really red"` produced a plain String,
    /// and the assignment silently failed in Studio.
    #[test]
    fn brickcolor_text_stays_brickcolor() {
        let current_value = P::BrickColor("Medium stone grey".into());
        assert_eq!(
            coerce_to_existing_type(&current_value, "Really red"),
            Some(P::BrickColor("Really red".into()))
        );
    }

    /// With three numbers the rotation must be kept: the user only wants to move it.
    #[test]
    fn cframe_three_numbers_keep_rotation() {
        let rot = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let current_value = P::CFrame {
            pos: [1.0, 2.0, 3.0],
            rot,
        };
        assert_eq!(
            coerce_to_existing_type(&current_value, "10, 0, -5"),
            Some(P::CFrame {
                pos: [10.0, 0.0, -5.0],
                rot
            })
        );
    }

    #[test]
    fn udim2_four_numbers() {
        let current_value = P::UDim2 {
            xs: 0.0,
            xo: 0.0,
            ys: 0.0,
            yo: 0.0,
        };
        assert_eq!(
            coerce_to_existing_type(&current_value, "0.5,10,0.25,-4"),
            Some(P::UDim2 {
                xs: 0.5,
                xo: 10.0,
                ys: 0.25,
                yo: -4.0
            })
        );
    }

    /// A number-like value must be writable to a text property:
    /// the general parser would turn "5" into a Number and Studio would reject the text.
    #[test]
    fn string_property_does_not_become_number() {
        let current_value = P::String("hello".into());
        assert_eq!(
            coerce_to_existing_type(&current_value, "5"),
            Some(P::String("5".into()))
        );
    }

    /// Input that cannot be fitted must return None so the caller falls back to the general parser.
    #[test]
    fn bad_input_returns_none() {
        let current_value = P::Vector2 { x: 0.0, y: 0.0 };
        assert_eq!(coerce_to_existing_type(&current_value, "abc"), None);
        assert_eq!(coerce_to_existing_type(&current_value, "1,2,3"), None);
    }

    #[test]
    fn color3_accepts_hex_and_triplet() {
        let current_value = P::Color3 {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
        assert_eq!(
            coerce_to_existing_type(&current_value, "#ff0000"),
            Some(P::Color3 {
                r: 1.0,
                g: 0.0,
                b: 0.0
            })
        );
        assert_eq!(
            coerce_to_existing_type(&current_value, "0,0.5,1"),
            Some(P::Color3 {
                r: 0.0,
                g: 0.5,
                b: 1.0
            })
        );
    }
}

#[cfg(test)]
mod wire_format_tests {
    use super::*;
    use model::{ColorKeypoint, NumberKeypoint, PropertyValue as P};

    /// A type has only really been carried once it can be both written and read.
    /// Types added in one direction only were the source of
    /// "sent, but lost on the other side".
    fn round_trip(pv: P) {
        let on_wire = pv_to_wire(&pv);
        let restored_count = parse_wire_value(&on_wire);
        assert_eq!(restored_count, Some(pv.clone()), "wire format: {:?}", on_wire);
    }

    #[test]
    fn asset_id_round_trip() {
        round_trip(P::Content("rbxassetid://123456".into()));
    }

    #[test]
    fn color_sequence_round_trip() {
        round_trip(P::ColorSequence(vec![
            ColorKeypoint { t: 0.0, r: 1.0, g: 0.0, b: 0.0 },
            ColorKeypoint { t: 1.0, r: 0.0, g: 0.0, b: 1.0 },
        ]));
    }

    /// The envelope is Roblox's randomness margin; if it drops, particle effects flatten.
    #[test]
    fn number_sequence_keeps_envelope() {
        let pv = P::NumberSequence(vec![
            NumberKeypoint { t: 0.0, v: 1.0, envelope: 0.25 },
            NumberKeypoint { t: 1.0, v: 0.0, envelope: 0.0 },
        ]);
        let restored_count = parse_wire_value(&pv_to_wire(&pv));
        match restored_count {
            Some(P::NumberSequence(k)) => assert_eq!(k[0].envelope, 0.25),
            other => panic!("beklenmeyen: {:?}", other),
        }
    }

    #[test]
    fn rect_and_font_round_trip() {
        round_trip(P::Rect { min: [4.0, 4.0], max: [12.0, 12.0] });
        round_trip(P::Font {
            family: "rbxasset://fonts/families/SourceSansPro.json".into(),
            weight: "Enum.FontWeight.Bold".into(),
            style: "Enum.FontStyle.Normal".into(),
        });
    }

    #[test]
    fn physical_properties_round_trip() {
        round_trip(P::PhysicalProperties {
            density: 0.7,
            friction: 0.3,
            elasticity: 0.5,
            friction_weight: 1.0,
            elasticity_weight: 2.0,
        });
    }

    /// A value sent to Studio must NOT be in serde's tagged form.
    ///
    /// This is a regression test: for a while the disk side put `PropertyValue` straight
    /// into JSON, and serde wrote it as {"Number":0.5}. The plugin expects the plain
    /// form, so it rejected it with "unsupported table value for property: Transparency"
    /// — caught in live use.
    #[test]
    fn wire_format_has_no_serde_tag() {
        for pv in [
            P::Number(0.5),
            P::String("hello".into()),
            P::Boolean(true),
        ] {
            let on_wire = pv_to_wire(&pv);
            assert!(
                !on_wire.is_object(),
                "a primitive value must go plain, not as a table: {:?} -> {}",
                pv,
                on_wire
            );
            // Serde's form really must be different; the test verifies its own assumption.
            let serde_form = serde_json::to_value(&pv).unwrap();
            assert!(serde_form.is_object(), "serde must write the tagged form: {}", serde_form);
            assert_ne!(on_wire, serde_form);
        }
    }

    /// Previously added types must not break either: new branches join an ordered if/else
    /// chain, and a branch added in the wrong place can shadow an earlier one.
    #[test]
    fn legacy_types_still_work() {
        round_trip(P::BrickColor("Really red".into()));
        round_trip(P::Ref("11111111-2222-4333-8444-555555555555".into()));
        round_trip(P::CFrame {
            pos: [1.0, 2.0, 3.0],
            rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        });
        round_trip(P::NumberRange { min: 1.0, max: 5.0 });
        round_trip(P::UDim { scale: 0.5, offset: 10.0 });
        round_trip(P::Vector2 { x: 1.0, y: 2.0 });
    }
}
