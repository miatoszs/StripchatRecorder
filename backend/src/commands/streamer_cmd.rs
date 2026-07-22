//! Streamer Management Commands
//!
//! 、/、、/。
//! Provides streamer list queries, add/remove streamers, auto-record toggle, and manual recording control.
//! These functions are called directly by the HTTP server handlers in server_mod/server.rs.

use crate::core::error::Result;
use crate::streaming::monitor::StatusMonitor;
use crate::recording::recorder::RecorderManager;
use crate::config::settings::AppState;
use crate::streaming::stripchat::StripchatApi;
use std::sync::Arc;

/// （）/ Streamer entry (serialized and returned to the frontend)
#[derive(serde::Serialize)]
pub struct StreamerEntry {
    pub username: String,
    pub auto_record: bool,
    pub added_at: String,
    /// Whether online
    pub is_online: bool,
    /// Whether currently recording
    pub is_recording: bool,
    /// （）/ Whether recordable (stream publicly accessible)
    pub is_recordable: bool,
    pub viewers: i64,
    /// Stream status text
    pub status: String,
    pub thumbnail_url: Option<String>,
}

/// 。
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
                viewers: status.as_ref().map(|s| s.viewers).unwrap_or(0),
                status: status
                    .as_ref()
                    .map(|s| s.status.clone())
                    .unwrap_or_else(|| "未知".to_string()),
                thumbnail_url: status.and_then(|s| s.thumbnail_url),
            }
        })
        .collect())
}

/// 。
/// Add a new streamer to the tracking list.
pub async fn add_streamer(
    username: String,
    state: &Arc<AppState>,
) -> Result<()> {
    let username = username.trim().to_lowercase();
    if username.is_empty() {
        return Err("用户名不能为空".into());
    }

    let settings = state.get_settings();
    let api = StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    )?;
    api.get_stream_info(&username, false)
        .await
        .map_err(|e| crate::core::error::AppError::Other(format!("{}", e)))?;

    state.add_streamer(&username)?;
    Ok(())
}

/// ，。
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

/// 。
/// Set the auto-record toggle for a specific streamer.
pub async fn set_auto_record(
    username: String,
    enabled: bool,
    state: &Arc<AppState>,
) -> Result<()> {
    state.set_auto_record(&username, enabled)?;
    Ok(())
}

/// 。
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

/// 。
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

/// Stripchat。
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
    match api.get_stream_info(&username, false).await {
        Ok(_) => Ok(serde_json::json!({ "exists": true })),
        Err(crate::core::error::AppError::UserNotFound(_)) => {
            Ok(serde_json::json!({ "exists": false }))
        }
        Err(e) => Err(e),
    }
}
