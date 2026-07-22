//! Relay Session State

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc};

/// Current state of a relay stream
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayStreamState {
    /// Connecting to upstream
    Connecting,
    /// Relaying live stream
    Live,
    /// Upstream offline, outputting status frame
    Offline { status: String },
    /// Error occurred
    Error { message: String },
}

/// Relay session
pub struct RelaySession {
    /// URL（）/ Upstream playlist URL (if obtained)
    pub playlist_url: Option<String>,
    /// Current stream state
    pub stream_state: RelayStreamState,
    /// （ worker ）/ Streamer real online status (updated by worker in real time)
    pub streamer_is_online: bool,
    /// （ worker ）/ Streamer real status text (updated by worker in real time)
    pub streamer_status: String,
    /// Number of active connections
    pub active_connections: u32,
    /// （）/ Session creation time (for uptime calculation)
    pub created_at: Instant,
    /// Unix （，）/ Session creation Unix timestamp in ms (for client-side timer)
    pub created_at_ms: u64,
    /// Last active time
    pub last_active: Instant,
    /// Signal to stop worker
    pub stop_tx: mpsc::Sender<()>,
    /// TS data broadcast sender
    pub ts_tx: broadcast::Sender<Arc<Vec<u8>>>,
}

/// Global relay session manager
pub struct RelayManager {
    pub sessions: RwLock<HashMap<String, RelaySession>>,
}

impl RelayManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: RwLock::new(HashMap::new()),
        })
    }

    /// 。
    pub fn create_session(
        &self,
        username: &str,
        stop_tx: mpsc::Sender<()>,
        ts_tx: broadcast::Sender<Arc<Vec<u8>>>,
    ) {
        let now_instant = Instant::now();
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.sessions.write().insert(
            username.to_string(),
            RelaySession {
                playlist_url: None,
                stream_state: RelayStreamState::Connecting,
                streamer_is_online: false,
                streamer_status: String::new(),
                active_connections: 0,
                created_at: now_instant,
                created_at_ms,
                last_active: now_instant,
                stop_tx,
                ts_tx,
            },
        );
    }

    /// TS ，。
    pub fn subscribe(&self, username: &str) -> Option<broadcast::Receiver<Arc<Vec<u8>>>> {
        let mut sessions = self.sessions.write();
        if let Some(s) = sessions.get_mut(username) {
            s.active_connections += 1;
            s.last_active = Instant::now();
            return Some(s.ts_tx.subscribe());
        }
        None
    }

    /// ，。
    /// Decrement connection count and update last_active when it reaches zero.
    pub fn unsubscribe(&self, username: &str) {
        let mut sessions = self.sessions.write();
        if let Some(s) = sessions.get_mut(username) {
            s.active_connections = s.active_connections.saturating_sub(1);
            if s.active_connections == 0 {
                s.last_active = Instant::now();
            }
        }
    }

    /// （）。
    /// Check if a session is idle (no connections and inactive for more than the given seconds).
    pub fn is_idle(&self, username: &str, idle_secs: u64) -> bool {
        let sessions = self.sessions.read();
        if let Some(s) = sessions.get(username) {
            s.active_connections == 0 && s.last_active.elapsed().as_secs() >= idle_secs
        } else {
            false
        }
    }

    /// 。
    pub fn set_state(&self, username: &str, state: RelayStreamState) {
        let mut sessions = self.sessions.write();
        if let Some(s) = sessions.get_mut(username) {
            s.stream_state = state;
            s.last_active = Instant::now();
        }
    }

    /// （ worker  API ）。
    /// Update the streamer's real status (called by worker after each API query).
    pub fn set_streamer_status(&self, username: &str, is_online: bool, status: String) {
        let mut sessions = self.sessions.write();
        if let Some(s) = sessions.get_mut(username) {
            s.streamer_is_online = is_online;
            s.streamer_status = status;
        }
    }

    /// URL。
    pub fn set_playlist_url(&self, username: &str, url: Option<String>) {
        let mut sessions = self.sessions.write();
        if let Some(s) = sessions.get_mut(username) {
            s.playlist_url = url;
        }
    }

    /// URL。
    #[allow(dead_code)]
    pub fn get_playlist_url(&self, username: &str) -> Option<String> {
        self.sessions.read().get(username).and_then(|s| s.playlist_url.clone())
    }

    /// 。
    pub fn remove(&self, username: &str) {
        if let Some(session) = self.sessions.write().remove(username) {
            let _ = session.stop_tx.try_send(());
        }
    }

    /// 。
    pub fn has_session(&self, username: &str) -> bool {
        self.sessions.read().contains_key(username)
    }

    /// （）。
    pub fn get_all_status(&self) -> Vec<RelaySessionStatus> {
        self.sessions
            .read()
            .iter()
            .map(|(username, s)| RelaySessionStatus {
                username: username.clone(),
                stream_state: s.stream_state.clone(),
                streamer_is_online: s.streamer_is_online,
                streamer_status: s.streamer_status.clone(),
                active_connections: s.active_connections,
                uptime_secs: s.created_at.elapsed().as_secs(),
                created_at_ms: s.created_at_ms,
                stream_url: format!("/stream/{}", username),
            })
            .collect()
    }
}

/// （）/ Session status snapshot (serialized for frontend)
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelaySessionStatus {
    pub username: String,
    pub stream_state: RelayStreamState,
    /// Streamer real online status
    pub streamer_is_online: bool,
    /// Streamer real status text
    pub streamer_status: String,
    pub active_connections: u32,
    /// （，）/ Uptime in seconds (server-computed, used as initial value)
    pub uptime_secs: u64,
    /// Session creation Unix timestamp (ms) for client-side timer
    pub created_at_ms: u64,
    pub stream_url: String,
}
