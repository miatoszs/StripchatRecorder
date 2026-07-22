//! Relay Route Handlers
//!
//! ：
//! - GET  /stream/{modelname}       →  MPEG-TS （ worker）
//! - GET  /api/relay/sessions       →
//! - POST /api/relay/{modelname}/stop →  worker
//!
//! ，：
//! -  worker
//! -
//! - +

use super::state::RelayManager;
use super::streamer::start_streamer;
use crate::config::settings::AppState;
use axum::{
    Json,
    body::Body,
    extract::{Path, State as AxumState},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Axum （）/ Axum shared state for relay routes
#[derive(Clone)]
pub struct RelayState {
    pub app_state: Arc<AppState>,
    pub relay_manager: Arc<RelayManager>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// GET /stream/{modelname}
///
/// worker， MPEG-TS 。
/// URL ，。
///
/// Starts relay worker on demand, continuously outputs MPEG-TS byte stream.
/// Players open this URL directly without any prior configuration.
pub async fn stream_handler(
    AxumState(s): AxumState<RelayState>,
    Path(modelname): Path<String>,
) -> Response {
    // Auto-start worker if no active session
    if !s.relay_manager.has_session(&modelname) {
        let (stop_tx, ts_tx) = start_streamer(
            modelname.clone(),
            Arc::clone(&s.app_state),
            Arc::clone(&s.relay_manager),
        );
        s.relay_manager.create_session(&modelname, stop_tx, ts_tx);
    }

    let rx = match s.relay_manager.subscribe(&modelname) {
        Some(rx) => rx,
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to subscribe to stream")
                .into_response();
        }
    };

    let relay_manager = Arc::clone(&s.relay_manager);
    let modelname_clone = modelname.clone();

    // RAII guard： stream ， unsubscribe 。
    // RAII guard: ensures unsubscribe is called whether the stream ends normally or the client disconnects abruptly.
    struct UnsubscribeGuard {
        relay_manager: Arc<RelayManager>,
        username: String,
    }
    impl Drop for UnsubscribeGuard {
        fn drop(&mut self) {
            self.relay_manager.unsubscribe(&self.username);
        }
    }
    let _guard = UnsubscribeGuard {
        relay_manager: Arc::clone(&relay_manager),
        username: modelname_clone.clone(),
    };

    // Decrement connection count on disconnect
    let stream = async_stream::stream! {
        // guard  stream ， stream  drop  unsubscribe
        // Move guard into stream closure so unsubscribe fires when stream is dropped
        let _guard = _guard;
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(chunk) => {
                    yield Ok::<axum::body::Bytes, std::convert::Infallible>(
                        axum::body::Bytes::from(chunk.as_ref().clone())
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    (
        [
            (header::CONTENT_TYPE, "video/mp2t"),
            (header::CACHE_CONTROL, "no-cache, no-store"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::TRANSFER_ENCODING, "chunked"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// GET /api/relay/sessions
///
/// 。
/// Returns the status list of all active relay sessions.
pub async fn relay_sessions(
    AxumState(s): AxumState<RelayState>,
) -> impl IntoResponse {
    let sessions = s.relay_manager.get_all_status();
    Json(sessions)
}

/// POST /api/relay/{modelname}/stop
///
/// worker，。
/// PotPlayer 、。
///
/// Forcefully stops the relay worker for the given streamer, regardless of active connections.
/// Useful for players like PotPlayer that briefly reconnect on close, preventing idle timeout.
pub async fn stop_relay_handler(
    AxumState(s): AxumState<RelayState>,
    Path(modelname): Path<String>,
) -> impl IntoResponse {
    s.relay_manager.remove(&modelname);
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}
