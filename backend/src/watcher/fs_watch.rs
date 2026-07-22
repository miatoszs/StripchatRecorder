//! File System Watchers
//!
//! ：
//! 1. ： `recordings-dir-changed` （ 400ms）
//! 2. ： `modules-changed` （ 500ms）
//!
//! Provides two file system watchers:
//! 1. Recording output directory watcher: detects file changes and emits `recordings-dir-changed` events (400ms debounce)
//! 2. Modules directory watcher: detects module executable additions/removals and emits `modules-changed` events (500ms debounce)

use crate::core::emitter::{Emitter, EmitterExt};
use crate::config::settings::AppState;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// ""（.ts/.tmp/.part/.partial ），，。
/// Check if a path is a "noisy" path (.ts/.tmp/.part/.partial files) that change frequently
/// and should not trigger a refresh.
fn is_noisy_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase()),
        Some(ext) if ext == "ts" || ext == "tmp" || ext == "part" || ext == "partial"
    )
}

/// 。
/// 。
///
/// Determine if a file system event should trigger a frontend refresh.
/// Filters out pure access events and changes to noisy paths.
fn should_emit(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|p| !is_noisy_path(p))
}

/// （）。
/// 。
///
/// Start the recording output directory watcher (runs in a dedicated thread).
/// Automatically switches the watch target when the output directory setting changes.
pub fn start_recordings_dir_watcher(state: Arc<AppState>, emitter: Arc<dyn Emitter>) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watched_dir = PathBuf::new();
        let mut last_emit = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let mut _watcher: Option<RecommendedWatcher> = None;

        loop {
            let current_dir = PathBuf::from(state.get_settings().output_dir);
            // If output dir changed, recreate the watcher
            if current_dir != watched_dir {
                if let Err(e) = std::fs::create_dir_all(&current_dir) {
                    tracing::error!("Failed to create watch dir {:?}: {}", current_dir, e);
                }

                match RecommendedWatcher::new(tx.clone(), Config::default()) {
                    Ok(mut w) => match w.watch(&current_dir, RecursiveMode::Recursive) {
                        Ok(()) => {
                            tracing::info!("Watching recordings dir: {:?}", current_dir);
                            watched_dir = current_dir;
                            _watcher = Some(w);
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to watch recordings dir {:?}: {}",
                                current_dir,
                                e
                            );
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to create watcher: {}", e);
                    }
                }
            }

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(event)) => {
                    if watched_dir.as_os_str().is_empty() || !should_emit(&event) {
                        continue;
                    }
                    // Debounce: only emit once within 400ms
                    if last_emit.elapsed() < Duration::from_millis(400) {
                        continue;
                    }
                    last_emit = Instant::now();
                    emitter.emit(
                        "recordings-dir-changed",
                        &serde_json::json!({
                            "outputDir": watched_dir,
                            "kind": format!("{:?}", event.kind),
                            "paths": event.paths,
                        }),
                    );
                }
                Ok(Err(e)) => tracing::error!("recordings watcher event error: {}", e),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::error!("recordings watcher channel disconnected");
                    break;
                }
            }
        }
    });
}

/// locale （）。
/// `locale/app/`  JSON ， `locale-files-changed` 。
/// 800ms，。
///
/// Start the locale directory watcher (runs in a dedicated thread).
/// Watches for JSON file additions/removals in `locale/app/` and emits `locale-files-changed` events.
/// Debounced at 800ms to avoid multiple triggers when several files are written at once.
pub fn start_locale_dir_watcher(emitter: Arc<dyn Emitter>) {
    std::thread::spawn(move || {
        let locale_dir = crate::locale::manager::app_locale_dir();
        let _ = std::fs::create_dir_all(&locale_dir);

        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut last_emit = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(mut w) => {
                // locale/app/ ，（ .json ）
                // Watch only the locale/app/ dir itself, non-recursive (only top-level .json changes matter)
                if let Err(e) = w.watch(&locale_dir, RecursiveMode::NonRecursive) {
                    tracing::error!("Failed to watch locale dir {:?}: {}", locale_dir, e);
                } else {
                    tracing::info!("Watching locale dir: {:?}", locale_dir);
                }
                w
            }
            Err(e) => {
                tracing::error!("Failed to create locale dir watcher: {}", e);
                return;
            }
        };
        let _watcher = &mut watcher;

        loop {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(event)) => {
                    // .json
                    // Only care about Create/Remove events for .json files
                    if matches!(event.kind, EventKind::Access(_)) {
                        continue;
                    }
                    let has_json = event.paths.iter().any(|p| {
                        p.extension().and_then(|e| e.to_str()) == Some("json")
                    });
                    if !has_json {
                        continue;
                    }
                    // Debounce: only emit once within 800ms
                    if last_emit.elapsed() < Duration::from_millis(800) {
                        continue;
                    }
                    last_emit = Instant::now();
                    emitter.emit("locale-files-changed", &serde_json::json!({}));
                }
                Ok(Err(e)) => tracing::error!("locale watcher error: {}", e),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// （）。
/// modules/ ， `modules-changed` 。
///
/// Start the modules directory watcher (runs in a dedicated thread).
/// Detects additions/removals of executables in the modules/ directory and emits `modules-changed` events.
pub fn start_modules_dir_watcher(emitter: Arc<dyn Emitter>) {
    std::thread::spawn(move || {
        let modules_dir = crate::postprocess::pipeline::modules_dir();
        let _ = std::fs::create_dir_all(&modules_dir);

        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut last_emit = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(mut w) => {
                // Watch only the modules dir itself, non-recursive
                if let Err(e) = w.watch(&modules_dir, RecursiveMode::NonRecursive) {
                    tracing::error!("Failed to watch modules dir: {}", e);
                } else {
                    tracing::info!("Watching modules dir: {:?}", modules_dir);
                }
                w
            }
            Err(e) => {
                tracing::error!("Failed to create modules watcher: {}", e);
                return;
            }
        };
        let _watcher = &mut watcher;

        loop {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(event)) => {
                    if matches!(event.kind, EventKind::Access(_)) {
                        continue;
                    }
                    // Debounce: only emit once within 500ms
                    if last_emit.elapsed() < Duration::from_millis(500) {
                        continue;
                    }
                    last_emit = Instant::now();
                    emitter.emit("modules-changed", &serde_json::json!({}));
                }
                Ok(Err(e)) => tracing::error!("modules watcher error: {}", e),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}
