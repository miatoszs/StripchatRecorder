//! 主播管理命令 / Streamer Management Commands
//!
//! 提供主播列表查询、添加/移除主播、设置自动录制、手动开始/停止录制等功能。
//! Provides streamer list queries, add/remove streamers, auto-record toggle, and manual recording control.
//! These functions are called directly by the HTTP server handlers in server_mod/server.rs.

use crate::core::error::Result;
use crate::streaming::monitor::StatusMonitor;
use crate::recording::recorder::RecorderManager;
use crate::config::settings::AppState;
use crate::streaming::stripchat::StripchatApi;
use std::sync::Arc;

/// 主播条目（序列化后返回给前端）/ Streamer entry (serialized and returned to the frontend)
#[derive(serde::Serialize)]
pub struct StreamerEntry {
    pub username: String,
    pub auto_record: bool,
    pub added_at: String,
    /// 是否在线 / Whether online
    pub is_online: bool,
    /// 是否正在录制 / Whether currently recording
    pub is_recording: bool,
    /// 是否可录制（直播间公开可访问）/ Whether recordable (stream publicly accessible)
    pub is_recordable: bool,
    /// 直播间状态文字 / Stream status text
    pub status: String,
    pub thumbnail_url: Option<String>,
}

/// 列出所有追踪主播及其当前状态。
/// List all tracked streamers with their current status.
pub async fn list_streamers(
    state: &Arc<AppState>,
    monitor: &Arc<StatusMonitor>,
    recorder: &Arc<RecorderManager>,
) -> Result<Vec<StreamerEntry>> {
    let streamers = state.get_streamers();

    Ok(streamers
        .into_iter()
        .map(|s| {
            let status = monitor.get_status(&s.username);
            StreamerEntry {
                username: s.username.clone(),
                auto_record: s.auto_record,
                added_at: s.added_at,
                is_online: status.as_ref().map(|s| s.is_online).unwrap_or(false),
                is_recording: recorder.is_recording(&s.username),
                is_recordable: status.as_ref().map(|s| s.is_recordable).unwrap_or(false),
                status: status
                    .as_ref()
                    .map(|s| s.status.clone())
                    .unwrap_or_else(|| "未知".to_string()),
                thumbnail_url: status.and_then(|s| s.thumbnail_url),
            }
        })
        .collect())
}

/// 添加新主播到追踪列表，支持批量。
/// 并发验证所有用户名（最多 5 个并发），逐个添加成功的主播，
/// 每处理完一条通过 emitter 推送 `streamer-batch-progress` 进度事件。
///
/// Add streamers to the tracking list, supporting batch input.
/// Verifies all usernames concurrently, adds each valid one,
/// and emits a `streamer-batch-progress` event after each entry completes.
pub async fn add_streamer(
    usernames: Vec<String>,
    state: &Arc<AppState>,
    emitter: &Arc<dyn crate::core::emitter::Emitter>,
    monitor: &Arc<StatusMonitor>,
) -> Result<serde_json::Value> {
    use crate::core::emitter::EmitterExt;

    // 去重、清理、过滤空值 / Deduplicate, trim, filter blanks
    let mut seen = std::collections::HashSet::new();
    let usernames: Vec<String> = usernames
        .into_iter()
        .map(|u| u.trim().to_lowercase())
        .filter(|u| !u.is_empty() && seen.insert(u.clone()))
        .collect();

    if usernames.is_empty() {
        return Err("用户名不能为空".into());
    }

    let total = usernames.len();
    let settings = state.get_settings();
    let api = Arc::new(StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    )?);

    // 全量并发：所有请求同时发出，边完成边处理，最快速度
    // Full concurrency: all requests sent at once, processed as they complete for maximum speed
    let mut tasks = tokio::task::JoinSet::new();
    for username in &usernames {
        let api = Arc::clone(&api);
        let username = username.clone();
        tasks.spawn(async move {
            let result = api.verify_user_exists(&username).await;
            (username, result)
        });
    }

    let mut success = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut done = 0usize;

    // 每完成一个验证立即处理并推送进度事件，无需等待其他请求
    // Process and emit progress for each result as soon as it completes
    while let Some(res) = tasks.join_next().await {
        let Ok((username, verify_result)) = res else { continue };

        let (ok, skipped_entry, error_msg) = match verify_result {
            Ok(()) => match state.add_streamer(&username) {
                Ok(()) => {
                    emitter.emit(
                        "streamer-added",
                        &serde_json::json!({ "username": username }),
                    );
                    success += 1;
                    (true, false, None::<String>)
                }
                Err(e) if e.to_string().contains("已存在") => {
                    // 已存在视为跳过，不计入失败 / Already exists: treat as skipped, not failed
                    skipped += 1;
                    (true, true, None::<String>)
                }
                Err(e) => {
                    failed += 1;
                    (false, false, Some(e.to_string()))
                }
            },
            Err(e) => {
                failed += 1;
                (false, false, Some(e.to_string()))
            }
        };

        done += 1;
        // 推送进度事件 / Emit progress event
        emitter.emit(
            "streamer-batch-progress",
            &serde_json::json!({
                "done": done,
                "total": total,
                "username": username,
                "ok": ok,
                "skipped": skipped_entry,
                "error": error_msg,
            }),
        );
    }

    // 所有验证和添加完成后，统一触发一次全量状态轮询（而非每个主播单独 spawn），
    // 避免与验证请求并发过多导致 API 限流。
    // After all verifications and additions, trigger a single full poll (instead of per-streamer spawns)
    // to avoid concurrent API rate limiting with the verify requests.
    if success > 0 {
        let emitter_clone = Arc::clone(emitter);
        let monitor_clone = Arc::clone(monitor);
        tokio::spawn(async move {
            monitor_clone.poll_all_with_emitter(&emitter_clone).await;
        });
    }

    Ok(serde_json::json!({
        "total": total,
        "success": success,
        "skipped": skipped,
        "failed": failed,
    }))
}

/// 从追踪列表中移除主播，同时停止录制并删除录制文件目录。
/// Remove a streamer from the tracking list, stopping any recording and deleting the recording directory.
pub async fn remove_streamer(
    username: String,
    state: &Arc<AppState>,
    recorder: &Arc<RecorderManager>,
) -> Result<()> {
    if recorder.is_recording(&username) {
        recorder.stop_recording(&username).await?;
    }
    let settings = state.get_settings();
    let streamer_dir = std::path::PathBuf::from(&settings.output_dir).join(&username);
    if streamer_dir.exists() {
        std::fs::remove_dir_all(&streamer_dir)?;
    }
    state.remove_streamer(&username)?;
    Ok(())
}

/// 设置指定主播的自动录制开关。
/// Set the auto-record toggle for a specific streamer.
pub async fn set_auto_record(
    username: String,
    enabled: bool,
    state: &Arc<AppState>,
) -> Result<()> {
    state.set_auto_record(&username, enabled)?;
    Ok(())
}

/// 手动开始录制指定主播。
/// Manually start recording a specific streamer.
pub async fn start_recording(
    username: String,
    state: &Arc<AppState>,
    recorder: &Arc<RecorderManager>,
    monitor: &Arc<StatusMonitor>,
    emitter: &Arc<dyn crate::core::emitter::Emitter>,
) -> Result<String> {
    let playlist_url = if let Some(url) = monitor.get_cached_playlist_url(&username) {
        url
    } else {
        let settings = state.get_settings();
        let api = StripchatApi::new_api_only(
            settings.api_proxy_url.as_deref(),
            settings.cdn_proxy_url.as_deref(),
            settings.sc_mirror_url.as_deref(),
        )?
        .with_mouflon_keys(state.get_mouflon_keys());
        let info = api.get_stream_info(&username, true).await?;
        info.playlist_url
            .ok_or_else(|| crate::core::error::AppError::StreamOffline(username.clone()))?
    };

    recorder
        .start_recording_with_emitter(&username, &playlist_url, Arc::clone(emitter))
        .await
}

/// 手动停止录制指定主播。
/// Manually stop recording a specific streamer.
pub async fn stop_recording(
    username: String,
    state: &Arc<AppState>,
    recorder: &Arc<RecorderManager>,
) -> Result<()> {
    let _ = state.set_auto_record(&username, false);
    recorder.stop_recording(&username).await?;
    Ok(())
}

/// 验证主播用户名是否存在于 Stripchat。
/// Verify whether a streamer username exists on Stripchat.
pub async fn verify_streamer(
    username: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value> {
    let settings = state.get_settings();
    let api = StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    )?;
    match api.verify_user_exists(&username).await {
        Ok(_) => Ok(serde_json::json!({ "exists": true })),
        Err(crate::core::error::AppError::UserNotFound(_)) => {
            Ok(serde_json::json!({ "exists": false }))
        }
        Err(e) => Err(e),
    }
}
