//! Desktop Application Global State
//!
//! backend  `AppState`、`RecorderManager`、`StatusMonitor`  Tauri ，
//! `AppHandle`  Tauri command 。
//!
//! Wraps the backend's `AppState`, `RecorderManager`, and `StatusMonitor`
//! as Tauri-managed state, along with an `AppHandle` for access in Tauri commands.

use std::sync::Arc;
use stripchat_recorder_lib::{
    config::settings::AppState,
    core::emitter::Emitter,
    recording::recorder::RecorderManager,
    streaming::monitor::StatusMonitor,
};

/// Tauri 。
/// Tauri-managed global application state.
pub struct DesktopState {
    /// Application business state
    pub app_state: Arc<AppState>,
    /// Recorder manager
    pub recorder: Arc<RecorderManager>,
    /// Streamer status monitor
    pub monitor: Arc<StatusMonitor>,
    /// （Arc<dyn Emitter>， TauriEmitter）
    /// Event emitter (Arc<dyn Emitter>, backed by TauriEmitter)
    pub emitter: Arc<dyn Emitter>,
}
