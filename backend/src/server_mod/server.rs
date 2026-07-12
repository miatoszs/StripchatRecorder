//! HTTP 服务器模式 / HTTP Server Mode
//!
//! 基于 Axum 构建的 HTTP API 服务器，提供与 Tauri 命令等价的 REST 接口和 SSE 实时事件流。
//! 同时内嵌前端静态资源（通过 rust-embed 编译进二进制）。
//!
//! Axum-based HTTP API server providing REST endpoints equivalent to Tauri commands,
//! plus an SSE real-time event stream.
//! Also embeds frontend static assets (compiled into the binary via rust-embed).

use crate::config::settings::AppState;
use crate::core::emitter::{BroadcastEmitter, Event};
use crate::recording::recorder::RecorderManager;
use crate::relay::handler::{RelayState, relay_sessions, stop_relay_handler, stream_handler};
use crate::relay::state::RelayManager;
use crate::server_mod::routes::{
    locale::{get_locale_handler, list_locales_handler},
    postprocess::{
        cancel_postprocess, get_module_outputs, get_pipeline, get_postprocess_tasks, list_modules,
        run_postprocess, run_postprocess_batch, save_pipeline,
    },
    recording::{
        delete_recording, get_merging_dirs_handler, list_recordings, open_output_dir,
        open_recording, serve_output_file,
    },
    settings::{
        add_mouflon_key, get_disk_space_handler, get_settings, get_startup_warnings_handler,
        list_mouflon_keys, remove_missing_pp_results_handler, remove_mouflon_key, save_settings,
        sync_mouflon_keys,
    },
    streamer::{
        add_streamer, list_streamers, remove_streamer, set_auto_record, start_recording,
        stop_recording, verify_streamer,
    },
};
use crate::server_mod::sse::sse_handler;
use crate::server_mod::static_files::static_handler;
use crate::streaming::monitor::StatusMonitor;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

/// Axum 路由共享状态 / Axum router shared state
#[derive(Clone)]
pub struct ServerState {
    /// 应用全局状态 / Application global state
    pub app_state: Arc<AppState>,
    /// 录制管理器 / Recorder manager
    pub recorder: Arc<RecorderManager>,
    /// 状态监控器 / Status monitor
    pub monitor: Arc<StatusMonitor>,
    /// 事件发射器 / Event emitter
    pub emitter: Arc<dyn crate::core::emitter::Emitter>,
    /// SSE 广播发送端 / SSE broadcast sender
    pub broadcast_tx: broadcast::Sender<Event>,
    /// 转发管理器 / Relay manager
    pub relay_manager: Arc<RelayManager>,
}

/// 构建 Axum 路由器，注册所有 API 路由和静态资源回退处理器。
/// Build the Axum router, registering all API routes and the static asset fallback handler.
pub fn build_router(state: ServerState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let relay_state = RelayState {
        app_state: Arc::clone(&state.app_state),
        relay_manager: Arc::clone(&state.relay_manager),
    };
    // /stream/{modelname} 路由（独立 state）/ /stream/{modelname} route (independent state)
    let stream_router: Router<()> = Router::new()
        .route("/{modelname}", get(stream_handler))
        .with_state(relay_state.clone());
    // /api/relay/sessions 路由 / /api/relay/sessions route
    let relay_api_router: Router<()> = Router::new()
        .route("/sessions", get(relay_sessions))
        .route("/{modelname}/stop", post(stop_relay_handler))
        .with_state(relay_state);

    // 主路由器先固化 state，再合并转发路由
    // Finalize main router state first, then merge relay router
    let main_router: Router<()> = Router::new()
        .route("/api/streamers", get(list_streamers).post(add_streamer))
        .route("/api/streamers/{name}", delete(remove_streamer))
        .route("/api/streamers/{name}/auto-record", post(set_auto_record))
        .route("/api/streamers/{name}/start", post(start_recording))
        .route("/api/streamers/{name}/stop", post(stop_recording))
        .route("/api/streamers/{name}/verify", get(verify_streamer))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route(
            "/api/mouflon-keys",
            get(list_mouflon_keys).post(add_mouflon_key),
        )
        .route("/api/mouflon-keys/{pkey}", delete(remove_mouflon_key))
        .route("/api/mouflon-keys/sync", post(sync_mouflon_keys))
        .route("/api/startup-warnings", get(get_startup_warnings_handler))
        .route(
            "/api/startup-warnings/pp-results",
            post(remove_missing_pp_results_handler),
        )
        .route("/api/disk-space", get(get_disk_space_handler))
        .route("/api/recordings", get(list_recordings))
        .route("/api/recordings/merging", get(get_merging_dirs_handler))
        .route("/api/recordings/delete", post(delete_recording))
        .route("/api/recordings/open", post(open_recording))
        .route("/api/recordings/open-dir", post(open_output_dir))
        .route("/api/recordings/postprocess", post(run_postprocess))
        .route(
            "/api/recordings/postprocess-batch",
            post(run_postprocess_batch),
        )
        .route(
            "/api/recordings/postprocess-cancel",
            post(cancel_postprocess),
        )
        .route("/api/pipeline", get(get_pipeline).post(save_pipeline))
        .route("/api/modules", get(list_modules))
        .route("/api/postprocess-tasks", get(get_postprocess_tasks))
        .route("/api/recordings/module-outputs", post(get_module_outputs))
        .route("/api/locale/{locale_code}", get(get_locale_handler))
        .route("/api/locales", get(list_locales_handler))
        .route("/api/files", get(serve_output_file))
        .route("/api/events", get(sse_handler))
        .with_state(state)
        .fallback(static_handler);

    // 合并转发路由（两者都是 Router<()>，可以直接 merge）
    // Merge relay routes (both are Router<()>, can merge directly)
    main_router
        .nest("/stream", stream_router)
        .nest("/api/relay", relay_api_router)
        .layer(cors)
}

/// 扫描用户自定义语言文件，将校验失败的文件通过 SSE 推送给前端。
/// 在服务器启动、emitter 就绪后调用一次。
///
/// Scan all user-defined locale files and push validation failures to the frontend via SSE.
/// Called once after server startup when the emitter is ready.
pub fn emit_locale_warnings(emitter: &Arc<dyn crate::core::emitter::Emitter>) {
    use crate::core::emitter::EmitterExt;
    let warnings = crate::locale::manager::check_custom_locale_files();
    if warnings.is_empty() {
        return;
    }
    let payload: Vec<serde_json::Value> = warnings
        .into_iter()
        .map(|(path, reason)| serde_json::json!({ "path": path, "reason": reason }))
        .collect();
    tracing::warn!("Custom locale file validation warnings: {:?}", payload);
    emitter.emit("locale-warnings", &payload);
}

/// 初始化并启动 HTTP 服务器模式。
/// Initialize and start the HTTP server mode.
pub async fn run_server(port: u16) {
    let log_dir = AppState::log_dir();
    if let Err(e) = crate::core::logging::init_logging(&log_dir) {
        eprintln!("Failed to initialize logging: {}", e);
    }

    let app_state = AppState::new().expect("Failed to initialize app state");

    // 初始化 locale 目录（首次运行时创建默认语言 JSON 文件）
    // Initialize locale directories (create default locale JSON files on first run)
    crate::locale::manager::init_locale_dirs();

    let recorder = RecorderManager::new(Arc::clone(&app_state));
    let (tx, _) = broadcast::channel::<Event>(4096);
    let emitter: Arc<dyn crate::core::emitter::Emitter> = Arc::new(BroadcastEmitter(tx.clone()));
    let monitor = StatusMonitor::new(Arc::clone(&app_state), Arc::clone(&recorder));
    crate::watcher::fs_watch::start_recordings_dir_watcher(
        Arc::clone(&app_state),
        Arc::clone(&emitter),
    );
    crate::watcher::fs_watch::start_modules_dir_watcher(Arc::clone(&emitter));
    crate::watcher::fs_watch::start_locale_dir_watcher(Arc::clone(&emitter));

    // 扫描用户自定义语言文件，将校验警告推送给前端
    // Scan user-defined locale files and push validation warnings to the frontend
    {
        let emitter_clone = Arc::clone(&emitter);
        tokio::task::spawn_blocking(move || {
            emit_locale_warnings(&emitter_clone);
        });
    }

    if !crate::recording::recorder::ffmpeg_available() {
        tracing::warn!("ffmpeg not found on PATH");
    }
    {
        let settings = app_state.get_settings();
        let output_dir = std::path::PathBuf::from(&settings.output_dir);
        let tmp_dir = settings
            .tmp_dir
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from);
        let merge_format = settings.merge_format.clone();
        let emitter_clone = Arc::clone(&emitter);
        let recorder_clone = Arc::clone(&recorder);
        tokio::task::spawn_blocking(move || {
            crate::recording::recorder::startup_merge_leftover_segments(
                &output_dir,
                tmp_dir.as_deref(),
                &merge_format,
                &emitter_clone,
                &recorder_clone,
            );
            crate::recording::recorder::startup_remove_empty_dirs(&output_dir);
            // 扫描并补写缺失的 meta 文件
            // Scan and write missing meta files
            crate::recording::meta::startup_ensure_meta_files(&output_dir, &merge_format);
        });
    }

    // 提前创建 restart channel，确保 poll_interval_notify_tx 在 spawn 前就已注入
    // Pre-create restart channel so poll_interval_notify_tx is available before spawning
    {
        let (restart_tx, restart_rx) = tokio::sync::mpsc::channel::<()>(1);
        *app_state.poll_interval_notify_tx.write() = Some(restart_tx.clone());
        *monitor.restart_tx.write() = Some(restart_tx);
        let monitor_clone = Arc::clone(&monitor);
        let emitter_clone = Arc::clone(&emitter);
        tokio::spawn(async move {
            monitor_clone
                .start_with_emitter_inner(emitter_clone, restart_rx)
                .await;
        });
    }

    let app_state_clone = Arc::clone(&app_state);
    let emitter_clone2 = Arc::clone(&emitter);
    tokio::spawn(async move {
        crate::config::settings::schedule_config_checks(app_state_clone, emitter_clone2).await;
    });

    // 启动 Mouflon Keys 自动同步调度器（启动时立即同步一次，之后每小时一次）
    // Start Mouflon Keys auto-sync scheduler (once on startup, then every hour)
    {
        let app_state_clone = Arc::clone(&app_state);
        let emitter_clone = Arc::clone(&emitter);
        let (mouflon_notify_tx, mouflon_notify_rx) = tokio::sync::mpsc::channel::<()>(1);
        *app_state_clone.mouflon_sync_notify_tx.write() = Some(mouflon_notify_tx);
        tokio::spawn(async move {
            crate::config::settings::schedule_mouflon_sync(
                app_state_clone,
                emitter_clone,
                mouflon_notify_rx,
            )
            .await;
        });
    }

    // 启动孤立 meta 文件清理调度器（启动时立即执行一次，之后每小时一次）
    // Start orphaned meta cleanup scheduler (once on startup, then every hour)
    {
        let output_dir = std::path::PathBuf::from(&app_state.get_settings().output_dir);
        tokio::spawn(async move {
            crate::recording::meta::schedule_meta_cleanup(output_dir).await;
        });
    }

    // 启动 meta 版本检查轮询调度器（启动时立即执行一次，之后每 5 分钟一次）
    // Start meta version-check polling scheduler (once on startup, then every 5 minutes)
    {
        let settings = app_state.get_settings();
        let output_dir = std::path::PathBuf::from(&settings.output_dir);
        let merge_format = settings.merge_format.clone();
        tokio::spawn(async move {
            crate::recording::meta::schedule_meta_version_check(output_dir, merge_format, 300)
                .await;
        });
    }

    let server_state = ServerState {
        app_state,
        recorder,
        monitor,
        emitter,
        broadcast_tx: tx,
        relay_manager: RelayManager::new(),
    };

    let app = build_router(server_state);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {} — {}", addr, e));

    println!("Server mode: listening on http://{}", addr);
    println!("API docs: GET /api/events → SSE stream");
    axum::serve(listener, app).await.expect("server error");
}
