//! Tauri Desktop Application Library Entry
//!
//! （AppState、RecorderManager、StatusMonitor），
//! Tauri commands，（、Mouflon 、）。
//!
//! Initializes all backend components (AppState, RecorderManager, StatusMonitor),
//! registers Tauri commands, and starts background tasks
//! (status monitoring, Mouflon sync, file watching, etc.).

mod commands;
mod emitter;
mod state;

use crate::emitter::TauriEmitter;
use crate::state::DesktopState;
use std::sync::Arc;
use tauri::Manager;
use stripchat_recorder_lib::{
    config::settings::{AppState, schedule_config_checks, schedule_mouflon_sync},
    core::emitter::{EmitterExt, Emitter},
    recording::{
        meta::{schedule_meta_cleanup, schedule_meta_version_check},
        recorder::RecorderManager,
    },
    streaming::monitor::StatusMonitor,
    watcher::fs_watch::{start_modules_dir_watcher, start_recordings_dir_watcher},
};
use tokio::sync::mpsc;

/// Tauri ， `main.rs` 。
/// Tauri application run entry point, called from `main.rs`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tokio runtime， setup 。
    // Tauri  setup()  Tokio ， runtime。
    //
    // Create a dedicated Tokio runtime for setup and all background tasks.
    // Tauri's setup() callback does not run in a Tokio context, so we must
    // establish the runtime here.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // runtime  Arc ， setup closure  builder 。
    // Wrap the runtime in Arc so it can be shared across the setup closure.
    let rt = Arc::new(rt);
    let rt_for_setup = Arc::clone(&rt);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // ，
            // When another instance is launched, bring the existing window to front
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // （），。
            // Fast synchronous initialization (excluding time-consuming leftover segment merging),
            // then show the window immediately.
            rt_for_setup.block_on(async move {
                setup_app(app_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Streamers
            commands::list_streamers,
            commands::add_streamer,
            commands::remove_streamer,
            commands::set_auto_record,
            commands::start_recording,
            commands::stop_recording,
            commands::verify_streamer,
            // Settings
            commands::get_settings,
            commands::save_settings_cmd,
            commands::get_disk_space,
            // Mouflon Keys
            commands::list_mouflon_keys,
            commands::add_mouflon_key,
            commands::remove_mouflon_key,
            commands::sync_mouflon_keys,
            // Recordings
            commands::list_recordings,
            commands::get_merging_dirs,
            commands::delete_recording,
            commands::open_recording,
            commands::open_output_dir,
            commands::read_output_file,
            commands::get_module_outputs,
            // Post-processing
            commands::run_postprocess_cmd,
            commands::run_postprocess_batch,
            commands::cancel_postprocess,
            commands::get_postprocess_tasks,
            // Pipeline
            commands::get_pipeline,
            commands::save_pipeline,
            commands::list_modules,
            // Locale
            commands::get_locale,
            commands::list_locales,
            // Startup warnings
            commands::get_startup_warnings,
            commands::remove_missing_pp_results,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // runtime  drop，。
    // Runtime is dropped here; all background tasks terminate naturally on exit.
    drop(rt);
}

/// Tokio runtime 。
/// async fn，，。
///
/// ：
/// 1. （）：，。
/// 2. （）：，。
///
/// Application initialization logic executed within the Tokio runtime context.
///
/// Initialization is split into two phases:
/// 1. Fast initialization (synchronous, blocking): shows the main window immediately.
/// 2. Background initialization (async, non-blocking): time-consuming leftover segment
///    merging runs in the background after the window is shown.
async fn setup_app(app_handle: tauri::AppHandle) {
    // Initialize logging
    let log_dir = AppState::log_dir();
    if let Err(e) = stripchat_recorder_lib::core::logging::init_logging(&log_dir) {
        eprintln!("Failed to initialize logging: {}", e);
    }

    // Initialize application state
    let app_state = AppState::new().expect("Failed to initialize app state");

    // Initialize locale directories
    stripchat_recorder_lib::locale::manager::init_locale_dirs();

    // Create TauriEmitter
    let emitter: Arc<dyn Emitter> = Arc::new(TauriEmitter::new(app_handle.clone()));

    // Create recorder manager
    let recorder = RecorderManager::new(Arc::clone(&app_state));

    // Create status monitor
    let monitor = StatusMonitor::new(Arc::clone(&app_state), Arc::clone(&recorder));

    // （，）
    // Remove empty directories on startup (sync, fast)
    {
        let settings = app_state.get_settings();
        let output_path_buf = std::path::PathBuf::from(&settings.output_dir);
        let output_ref = output_path_buf.as_path();
        stripchat_recorder_lib::recording::recorder::startup_remove_empty_dirs(output_ref);
        stripchat_recorder_lib::recording::meta::startup_ensure_meta_files(
            output_ref,
            &settings.merge_format,
        );
    }

    // Check if ffmpeg is available
    if !stripchat_recorder_lib::recording::recorder::ffmpeg_available() {
        emitter.emit(
            "ffmpeg-missing",
            &serde_json::json!({
                "message": "ffmpeg 未安装或不在 PATH 中，录制功能将不可用"
            }),
        );
    }

    // Validate and push custom locale warnings
    {
        let warnings = stripchat_recorder_lib::locale::manager::check_custom_locale_files();
        if !warnings.is_empty() {
            let payload: Vec<serde_json::Value> = warnings
                .into_iter()
                .map(|(path, reason)| serde_json::json!({ "path": path, "reason": reason }))
                .collect();
            emitter.emit("locale-warnings", &payload);
        }
    }

    // Inject poll interval change notification sender
    let (poll_tx, poll_rx) = mpsc::channel(1);
    *app_state.poll_interval_notify_tx.write() = Some(poll_tx);

    // Inject Mouflon sync notification sender
    let (mouflon_tx, mouflon_rx) = mpsc::channel(1);
    *app_state.mouflon_sync_notify_tx.write() = Some(mouflon_tx);

    // Register DesktopState as Tauri-managed state
    app_handle.manage(DesktopState {
        app_state: Arc::clone(&app_state),
        recorder: Arc::clone(&recorder),
        monitor: Arc::clone(&monitor),
        emitter: Arc::clone(&emitter),
    });

    // Start background async tasks

    // Status monitor polling
    let monitor_clone = Arc::clone(&monitor);
    let emitter_for_monitor = Arc::clone(&emitter);
    tokio::spawn(async move {
        monitor_clone.start_with_emitter_inner(emitter_for_monitor, poll_rx).await;
    });

    // Mouflon Keys auto-sync
    let app_state_for_mouflon = Arc::clone(&app_state);
    let emitter_for_mouflon = Arc::clone(&emitter);
    tokio::spawn(async move {
        schedule_mouflon_sync(app_state_for_mouflon, emitter_for_mouflon, mouflon_rx).await;
    });

    // Config check scheduler
    let app_state_for_config = Arc::clone(&app_state);
    let emitter_for_config = Arc::clone(&emitter);
    tokio::spawn(async move {
        schedule_config_checks(app_state_for_config, emitter_for_config).await;
    });

    // Meta file cleanup scheduler
    {
        let output_dir = std::path::PathBuf::from(app_state.get_settings().output_dir.clone());
        tokio::spawn(async move { schedule_meta_cleanup(output_dir).await });
    }

    // Meta version check scheduler
    {
        let output_dir = std::path::PathBuf::from(app_state.get_settings().output_dir.clone());
        let merge_format = app_state.get_settings().merge_format.clone();
        tokio::spawn(async move {
            schedule_meta_version_check(output_dir, merge_format, 3600).await;
        });
    }

    // （， Tokio）
    // File system watchers (run in dedicated threads, no Tokio needed)
    start_recordings_dir_watcher(Arc::clone(&app_state), Arc::clone(&emitter));
    start_modules_dir_watcher(Arc::clone(&emitter));
    stripchat_recorder_lib::watcher::fs_watch::start_locale_dir_watcher(Arc::clone(&emitter));

    // ── ：， ─────────────────────────────────
    // ── Phase 1 complete: show the main window so the user can interact immediately ──
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }

    // ── ：（，）─────────────────────────
    // ── Phase 2: merge leftover segments in the background (time-consuming, non-blocking) ──
    //
    // startup_merge_leftover_segments  ffmpeg，。
    // spawn_blocking ，，。
    //
    // startup_merge_leftover_segments internally calls ffmpeg, which may take seconds to minutes.
    // Running it in a background spawn_blocking thread after the window is shown avoids
    // blocking the first interactive frame.
    {
        let settings = app_state.get_settings();
        let output_path_buf = std::path::PathBuf::from(&settings.output_dir);
        let merge_format = settings.merge_format.clone();
        let emitter_blocking = Arc::clone(&emitter);
        let recorder_blocking = Arc::clone(&recorder);
        tokio::task::spawn_blocking(move || {
            stripchat_recorder_lib::recording::recorder::startup_merge_leftover_segments(
                output_path_buf.as_path(),
                &merge_format,
                &emitter_blocking,
                &recorder_blocking,
            );
        });
        // ： .await，
        // Note: no .await here — merging proceeds asynchronously in the background
    }
}
