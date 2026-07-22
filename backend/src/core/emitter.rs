//! Event Emitter Abstraction Layer
//!
//! `Emitter` trait，、。
//! Defines a unified `Emitter` trait so that core modules (recorder, monitor, etc.)
//! can emit events without depending on a specific runtime.

use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

/// （ + JSON ）。
/// Raw event data (event name + JSON payload string).
#[derive(Debug, Clone)]
pub struct Event {
    /// Event name
    pub name: String,
    /// JSON-serialized payload string
    pub payload: String,
}

/// trait：。
/// Event emitter trait: broadcasts events to all subscribers.
pub trait Emitter: Send + Sync + 'static {
    /// JSON 。
    /// Emit an event with a raw JSON string payload.
    fn emit_raw(&self, event: &str, payload: &str);
}

/// JSON  `emit_raw`。
/// Serializes a serializable payload to JSON and calls `emit_raw`.
#[allow(dead_code)]
pub fn emit<T: Serialize>(emitter: &dyn Emitter, event: &str, payload: &T) {
    match serde_json::to_string(payload) {
        Ok(s) => emitter.emit_raw(event, &s),
        Err(e) => tracing::error!("emit serialize error: {}", e),
    }
}

/// `Emitter`  `emit`  trait。
/// Extension trait providing a generic `emit` method for all `Emitter` implementors.
pub trait EmitterExt {
    /// JSON 。
    /// Serialize the payload to JSON and emit the event.
    fn emit<T: Serialize>(&self, event: &str, payload: &T);
}

impl<E: Emitter + ?Sized> EmitterExt for E {
    fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(s) => self.emit_raw(event, &s),
            Err(e) => tracing::error!("emit serialize error: {}", e),
        }
    }
}

/// HTTP ， `broadcast::Sender`  SSE 。
/// HTTP server mode emitter; pushes events to the SSE stream via `broadcast::Sender`.
#[derive(Clone)]
pub struct BroadcastEmitter(pub broadcast::Sender<Event>);

impl Emitter for BroadcastEmitter {
    fn emit_raw(&self, event: &str, payload: &str) {
        let _ = self.0.send(Event {
            name: event.to_string(),
            payload: payload.to_string(),
        });
    }
}

/// ，。
/// No-op emitter for testing or scenarios where event notifications are not needed.
#[allow(dead_code)]
pub struct NoopEmitter;

impl Emitter for NoopEmitter {
    fn emit_raw(&self, _event: &str, _payload: &str) {}
}

/// `Arc<dyn Emitter>`  `Emitter` 。
/// Allows `Arc<dyn Emitter>` to be used directly as an `Emitter`.
impl Emitter for Arc<dyn Emitter> {
    fn emit_raw(&self, event: &str, payload: &str) {
        (**self).emit_raw(event, payload)
    }
}
