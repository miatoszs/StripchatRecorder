//! Streamer Status Monitor
//!
//! ，：
//! -  `status-update`
//! - /（ auto_record ）
//!
//! Periodically polls the live status of all tracked streamers and on status changes:
//! - Emits `status-update` events to the frontend
//! - Automatically starts/stops recordings (based on auto_record settings)

use crate::core::emitter::{Emitter, EmitterExt};
use crate::recording::recorder::RecorderManager;
use crate::config::settings::{AppState, StreamerData};
use crate::streaming::stripchat::StripchatApi;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// （ `status-update` ）。
/// Streamer real-time status (serialized and sent to the frontend via `status-update` events).
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamerStatus {
    pub username: String,
    pub is_online: bool,
    pub is_recording: bool,
    pub is_recordable: bool,
    pub viewers: i64,
    /// （）/ Stream status text (Chinese)
    pub status: String,
    pub thumbnail_url: Option<String>,
    /// HLS  URL（，）/ HLS playlist URL (not serialized, internal use only)
    #[serde(skip)]
    pub playlist_url: Option<String>,
}

/// ，。
/// Streamer status monitor managing the polling loop and auto-recording logic.
pub struct StatusMonitor {
    /// Application state
    state: Arc<AppState>,
    /// Recorder manager
    recorder: Arc<RecorderManager>,
    /// Latest status cache per streamer
    statuses: RwLock<HashMap<String, StreamerStatus>>,
    /// （ sleep，）
    /// Sender to notify the polling loop to restart (interrupts current sleep, restarts with new interval)
    pub restart_tx: RwLock<Option<mpsc::Sender<()>>>,
}

impl StatusMonitor {
    /// 。
    /// Create a new status monitor instance.
    pub fn new(state: Arc<AppState>, recorder: Arc<RecorderManager>) -> Arc<Self> {
        Arc::new(Self {
            state,
            recorder,
            statuses: RwLock::new(HashMap::new()),
            restart_tx: RwLock::new(None),
        })
    }

    /// （ `None`）。
    /// Get the cached status for a specific streamer (returns `None` if not cached).
    pub fn get_status(&self, username: &str) -> Option<StreamerStatus> {
        self.statuses.read().get(username).cloned()
    }

    /// HLS  URL（， API ）。
    /// Get the cached HLS playlist URL for a streamer (for fast recording start, avoiding repeated API requests).
    pub fn get_cached_playlist_url(&self, username: &str) -> Option<String> {
        self.statuses
            .read()
            .get(username)
            .and_then(|s| s.playlist_url.clone())
    }

    /// （， emitter）。
    /// Start the monitoring loop (generic version, accepts any emitter).
    #[allow(dead_code)]
    pub async fn start_with_emitter(self: Arc<Self>, emitter: Arc<dyn Emitter>) {
        let (restart_tx, restart_rx) = mpsc::channel(1);
        *self.restart_tx.write() = Some(restart_tx);
        self.monitor_loop(emitter, restart_rx).await;
    }

    /// ： restart_rx（ server ）。
    /// Internal version: accepts a pre-created restart_rx (used by server mode).
    pub async fn start_with_emitter_inner(self: Arc<Self>, emitter: Arc<dyn Emitter>, restart_rx: mpsc::Receiver<()>) {
        self.monitor_loop(emitter, restart_rx).await;
    }

    /// ， poll_interval_secs 。
    /// Notify the monitor loop to interrupt the current sleep and restart with the latest poll_interval_secs.
    #[allow(dead_code)]
    pub fn notify_interval_changed(&self) {
        if let Some(tx) = self.restart_tx.read().as_ref() {
            let _ = tx.try_send(());
        }
    }

    /// ：，。
    /// Monitor main loop: poll once immediately, then poll periodically at the configured interval.
    async fn monitor_loop(
        self: Arc<Self>,
        emitter: Arc<dyn Emitter>,
        mut restart_rx: mpsc::Receiver<()>,
    ) {
        self.poll_all_with_emitter(&emitter).await;

        loop {
            let poll_interval =
                tokio::time::Duration::from_secs(self.state.get_settings().poll_interval_secs);

            tokio::select! {
                _ = restart_rx.recv() => {
                    // poll_interval_secs ，（）
                    // poll_interval_secs changed; restart timer with new interval (no immediate poll)
                    tracing::info!("Monitor: poll interval changed, restarting timer");
                    continue;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    self.poll_all_with_emitter(&emitter).await;
                }
            }
        }
    }

    /// （）。
    /// Try to start recordings for all eligible streamers (generic version).
    pub async fn try_start_pending_with_emitter(self: &Arc<Self>, emitter: &Arc<dyn Emitter>) {
        let settings = self.state.get_settings();
        if !settings.auto_record {
            return;
        }
        let streamers = self.state.get_streamers();

        let candidates: Vec<(String, String)> = {
            let statuses = self.statuses.read();
            streamers
                .iter()
                .filter(|s| s.auto_record && !self.recorder.is_recording(&s.username))
                .filter_map(|s| {
                    statuses.get(&s.username).and_then(|cached| {
                        if cached.is_online {
                            cached
                                .playlist_url
                                .as_ref()
                                .map(|url| (s.username.clone(), url.clone()))
                        } else {
                            None
                        }
                    })
                })
                .collect()
        };

        for (username, playlist_url) in candidates {
            if self.recorder.is_recording(&username) {
                continue;
            }
            tracing::info!("try_start_pending: auto-starting recording → {}", username);
            let _ = self
                .recorder
                .start_recording_with_emitter(&username, &playlist_url, Arc::clone(emitter))
                .await;
        }
    }

    /// （）。
    /// Perform a single status poll for one streamer (generic version).
    pub async fn poll_one_with_emitter(
        self: &Arc<Self>,
        username: &str,
        emitter: &Arc<dyn Emitter>,
    ) {
        let settings = self.state.get_settings();
        let streamers = self.state.get_streamers();

        let streamer = match streamers.into_iter().find(|s| s.username == username) {
            Some(s) => s,
            None => return,
        };

        let api = match StripchatApi::new(
            settings.api_proxy_url.as_deref(),
            settings.cdn_proxy_url.as_deref(),
            settings.sc_mirror_url.as_deref(),
            self.recorder.cdn_tld_cache(),
        ) {
            Ok(a) => a.with_mouflon_keys(self.state.get_mouflon_keys()),
            Err(e) => {
                tracing::error!("Failed to create API client: {}", e);
                emitter.emit(
                    "api-error",
                    &serde_json::json!({ "message": e.to_string() }),
                );
                return;
            }
        };

        self.poll_streamer(&api, streamer, emitter, settings.auto_record)
            .await;
    }

    /// （）。
    /// Concurrently poll the status of all tracked streamers (generic version).
    pub async fn poll_all_with_emitter(self: &Arc<Self>, emitter: &Arc<dyn Emitter>) {
        let settings = self.state.get_settings();
        let streamers = self.state.get_streamers();

        if streamers.is_empty() {
            return;
        }

        let api = match StripchatApi::new(
            settings.api_proxy_url.as_deref(),
            settings.cdn_proxy_url.as_deref(),
            settings.sc_mirror_url.as_deref(),
            self.recorder.cdn_tld_cache(),
        ) {
            Ok(a) => Arc::new(a.with_mouflon_keys(self.state.get_mouflon_keys())),
            Err(e) => {
                tracing::error!("Failed to create API client: {}", e);
                emitter.emit(
                    "api-error",
                    &serde_json::json!({ "message": e.to_string() }),
                );
                return;
            }
        };

        let tasks: Vec<_> = streamers
            .into_iter()
            .map(|streamer| {
                let api = Arc::clone(&api);
                let monitor = Arc::clone(self);
                let emitter = Arc::clone(emitter);
                let auto_record_global = settings.auto_record;

                tokio::spawn(async move {
                    monitor
                        .poll_streamer(&api, streamer, &emitter, auto_record_global)
                        .await;
                })
            })
            .collect();

        for t in tasks {
            let _ = t.await;
        }
    }

    /// ，，。
    /// Poll a single streamer's status, update the cache, and trigger auto-recording logic based on status changes.
    async fn poll_streamer(
        self: &Arc<Self>,
        api: &StripchatApi,
        streamer: StreamerData,
        emitter: &Arc<dyn Emitter>,
        auto_record_global: bool,
    ) {
        let username = streamer.username.clone();

        let is_recording = self.recorder.is_recording(&username);
        let (was_online, was_recording) = self
            .statuses
            .read()
            .get(&username)
            .map(|s| (s.is_online, s.is_recording))
            .unwrap_or((false, false));

        if !self.statuses.read().contains_key(&username) {
            self.statuses
                .write()
                .entry(username.clone())
                .or_insert_with(|| StreamerStatus {
                    username: username.clone(),
                    is_online: false,
                    is_recording,
                    is_recordable: false,
                    viewers: 0,
                    status: String::new(),
                    thumbnail_url: None,
                    playlist_url: None,
                });
        }

        let info = match api.get_stream_info(&username, !is_recording).await {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("Poll failed → {}: {}", username, e);
                return;
            }
        };

        let status = StreamerStatus {
            username: username.clone(),
            is_online: info.is_online,
            is_recording,
            // playlist_url， is_recordable ，
            // When recording, playlist_url is not fetched; preserve the last cached is_recordable
            // to avoid incorrectly disabling buttons
            is_recordable: if is_recording {
                self.statuses
                    .read()
                    .get(&username)
                    .map(|s| s.is_recordable)
                    .unwrap_or(info.playlist_url.is_some())
            } else {
                info.playlist_url.is_some()
            },
            viewers: info.viewers,
            status: info.status.clone(),
            thumbnail_url: info.thumbnail_url.clone(),
            playlist_url: info.playlist_url.clone(),
        };

        emitter.emit("status-update", &status);

        self.statuses.write().insert(username.clone(), status);

        let stream_no_longer_recordable = is_recording && !info.is_recordable;
        if stream_no_longer_recordable {
            tracing::info!(
                "Stream no longer recordable → {} (is_online={}, is_recordable={}, status={}), stopping recording",
                username, info.is_online, info.is_recordable, info.status
            );
            let _ = self.recorder.stop_recording_auto(&username).await;
        }

        let recording_dropped = was_recording && !is_recording && info.is_online;
        let just_came_online = info.is_online && !was_online;
        let naturally_stopped = self.recorder.naturally_stopped.write().remove(&username);
        let should_be_recording =
            info.is_recordable && !is_recording && streamer.auto_record && auto_record_global;
        if (just_came_online || recording_dropped || naturally_stopped || should_be_recording)
            && streamer.auto_record
            && auto_record_global
            && !is_recording
            && let Some(ref playlist_url) = info.playlist_url
        {
            tracing::info!("Auto-starting recording → {} (just_online={}, dropped={}, natural_stop={}, should_be={})", username, just_came_online, recording_dropped, naturally_stopped, should_be_recording);
            let _ = self
                .recorder
                .start_recording_with_emitter(&username, playlist_url, Arc::clone(emitter))
                .await;
        }
    }
}
