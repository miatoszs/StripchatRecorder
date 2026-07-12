//! 主播相关路由 handler / Streamer-related route handlers

use crate::core::emitter::EmitterExt;
use crate::server_mod::error::{ApiError, ApiResult};
use crate::server_mod::server::ServerState;
use axum::{
    Json,
    extract::{Path, State as AxumState},
};
use serde::Deserialize;
use std::sync::Arc;

pub async fn list_streamers(
    AxumState(s): AxumState<ServerState>,
) -> ApiResult<serde_json::Value> {
    let streamers = s.app_state.get_streamers();

    let has_any_status = streamers
        .iter()
        .any(|st| s.monitor.get_status(&st.username).is_some());
    if !has_any_status && !streamers.is_empty() {
        let monitor = Arc::clone(&s.monitor);
        let emitter = Arc::clone(&s.emitter);
        tokio::spawn(async move {
            monitor.poll_all_with_emitter(&emitter).await;
        });
    }

    let result: Vec<serde_json::Value> = streamers
        .into_iter()
        .map(|st| {
            let status = s.monitor.get_status(&st.username);
            serde_json::json!({
                "username": st.username,
                "auto_record": st.auto_record,
                "added_at": st.added_at,
                "is_online": status.as_ref().map(|s| s.is_online).unwrap_or(false),
                "is_recording": s.recorder.is_recording(&st.username),
                "is_recordable": status.as_ref().map(|s| s.is_recordable).unwrap_or(false),
                "status": status.as_ref().map(|s| s.status.clone()).unwrap_or_default(),
                "thumbnail_url": status.and_then(|s| s.thumbnail_url),
            })
        })
        .collect();
    Ok(Json(serde_json::Value::Array(result)))
}

#[derive(Deserialize)]
pub struct AddStreamerBody {
    pub username: String,
}

pub async fn add_streamer(
    AxumState(s): AxumState<ServerState>,
    Json(body): Json<AddStreamerBody>,
) -> ApiResult<serde_json::Value> {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() {
        return Err(ApiError("用户名不能为空".into()));
    }
    let settings = s.app_state.get_settings();
    let api = crate::streaming::stripchat::StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    )
    .map_err(ApiError::from)?;
    api.get_stream_info(&username, false)
        .await
        .map_err(ApiError::from)?;
    s.app_state
        .add_streamer(&username)
        .map_err(ApiError::from)?;
    s.emitter.emit(
        "streamer-added",
        &serde_json::json!({ "username": username }),
    );
    let emitter = Arc::clone(&s.emitter);
    let monitor = Arc::clone(&s.monitor);
    tokio::spawn(async move {
        monitor.poll_one_with_emitter(&username, &emitter).await;
    });
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_streamer(
    AxumState(s): AxumState<ServerState>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if s.recorder.is_recording(&name) {
        s.recorder
            .stop_recording(&name)
            .await
            .map_err(ApiError::from)?;
    }
    let settings = s.app_state.get_settings();
    let dir = std::path::PathBuf::from(&settings.output_dir).join(&name);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    s.app_state.remove_streamer(&name).map_err(ApiError::from)?;
    s.emitter
        .emit("streamer-removed", &serde_json::json!({ "username": name }));
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AutoRecordBody {
    pub enabled: bool,
}

pub async fn set_auto_record(
    AxumState(s): AxumState<ServerState>,
    Path(name): Path<String>,
    Json(body): Json<AutoRecordBody>,
) -> ApiResult<serde_json::Value> {
    s.app_state
        .set_auto_record(&name, body.enabled)
        .map_err(ApiError::from)?;
    s.emitter.emit(
        "auto-record-changed",
        &serde_json::json!({ "username": name, "enabled": body.enabled }),
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn start_recording(
    AxumState(s): AxumState<ServerState>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let playlist_url = if let Some(url) = s.monitor.get_cached_playlist_url(&name) {
        url
    } else {
        let settings = s.app_state.get_settings();
        let api = crate::streaming::stripchat::StripchatApi::new_api_only(
            settings.api_proxy_url.as_deref(),
            settings.cdn_proxy_url.as_deref(),
            settings.sc_mirror_url.as_deref(),
        )
        .map_err(ApiError::from)?
        .with_mouflon_keys(s.app_state.get_mouflon_keys());
        let info = api
            .get_stream_info(&name, true)
            .await
            .map_err(ApiError::from)?;
        info.playlist_url
            .ok_or_else(|| ApiError(format!("Stream offline: {}", name)))?
    };
    let path = s
        .recorder
        .start_recording_with_emitter(&name, &playlist_url, Arc::clone(&s.emitter))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({ "path": path })))
}

pub async fn stop_recording(
    AxumState(s): AxumState<ServerState>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let _ = s.app_state.set_auto_record(&name, false);
    s.emitter.emit(
        "auto-record-changed",
        &serde_json::json!({ "username": name, "enabled": false }),
    );
    s.recorder
        .stop_recording(&name)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn verify_streamer(
    AxumState(s): AxumState<ServerState>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let settings = s.app_state.get_settings();
    let api = crate::streaming::stripchat::StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    )
    .map_err(ApiError::from)?;
    match api.get_stream_info(&name, false).await {
        Ok(_) => Ok(Json(serde_json::json!({ "exists": true }))),
        Err(crate::core::error::AppError::UserNotFound(_)) => {
            Ok(Json(serde_json::json!({ "exists": false })))
        }
        Err(e) => Err(ApiError(e.to_string())),
    }
}
