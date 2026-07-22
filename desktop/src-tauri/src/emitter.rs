//! Tauri Event Emitter
//!
//! backend  `Emitter` trait， Tauri  `AppHandle::emit` 。
//! Implements the backend `Emitter` trait, broadcasting events to frontend windows
//! via Tauri's `AppHandle::emit`.

use stripchat_recorder_lib::core::emitter::Emitter;
use tauri::AppHandle;
use tauri::Emitter as TauriEmitterTrait;

/// Tauri 。
/// Event emitter for Tauri mode.
pub struct TauriEmitter {
    app: AppHandle,
}

impl TauriEmitter {
    /// TauriEmitter 。
    /// Create a new TauriEmitter instance.
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Emitter for TauriEmitter {
    fn emit_raw(&self, event: &str, payload: &str) {
        // Tauri emit ；payload  JSON ，
        // serde_json::Value ，。
        // Tauri emit needs a serializable value; payload is already a JSON string,
        // deserialize to serde_json::Value to avoid double-escaping.
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(payload) {
            let _ = TauriEmitterTrait::emit(&self.app, event, raw);
        } else {
            let _ = TauriEmitterTrait::emit(&self.app, event, payload);
        }
    }
}
