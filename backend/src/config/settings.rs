//! Application Configuration and Global State Management
//!
//! `Settings`（）、`AppData`（） `AppState`（）。
//! `AppState`  `parking_lot::RwLock` ，。
//!
//! Defines `Settings` (user configuration), `AppData` (persisted data), and `AppState` (runtime state).
//! `AppState` protects shared data with `parking_lot::RwLock` and provides post-processing task state tracking.

use crate::core::error::{AppError, Result};
use crate::postprocess::pipeline::PipelineConfig;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Mouflon ， mouflon_keys.json。
/// Mouflon key store, persisted to mouflon_keys.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MouflonKeysStore {
    /// pkey -> pdkey key pairs
    #[serde(default)]
    pub keys: HashMap<String, String>,
    /// （Worker）（RFC 3339）， Worker  updated_at ，。
    /// Key update timestamp from the data source (Worker, RFC 3339).
    /// Compared against the Worker's `updated_at`; skip write if equal.
    #[serde(default)]
    pub auto_synced_at: Option<String>,
    /// /（RFC 3339）。
    /// Timestamp of the last manual key add/remove (RFC 3339).
    #[serde(default)]
    pub manual_updated_at: Option<String>,
}

/// （）。
/// Post-processing task status snapshot (serialized and sent to the frontend).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PpTaskStatus {
    /// Video file path
    pub path: String,
    /// （0.0 - 100.0）/ Overall progress percentage (0.0 - 100.0)
    pub pct: f64,
    /// Current module done progress value
    pub mod_done: u32,
    /// Current module total progress value
    pub mod_total: u32,
    /// Current module name
    pub module_name: String,
    /// Number of completed nodes
    pub done: usize,
    /// Total number of nodes
    pub total: usize,
    /// "error"）/ Task status string
    pub status: String,
    /// （true = ，false = ）/ Whether from memory (true = in-progress, false = persisted result)
    pub from_memory: bool,
}

/// User-configurable recorder settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Recording output directory
    pub output_dir: String,
    /// （）/ Streamer status poll interval (seconds)
    pub poll_interval_secs: u64,
    /// Whether auto-record is enabled by default
    pub auto_record: bool,
    /// Stripchat API proxy URL
    pub api_proxy_url: Option<String>,
    /// CDN segment download proxy URL
    pub cdn_proxy_url: Option<String>,
    /// Stripchat mirror site URL
    pub sc_mirror_url: Option<String>,
    /// （0 = ）/ Max concurrent recordings (0 = unlimited)
    pub max_concurrent: usize,
    /// （ "mp4"）/ Recording segment merge format (default "mp4")
    #[serde(default = "default_merge_format")]
    pub merge_format: String,
    /// （GB，0 = ， 50 GB）
    /// Max size of the post-processing tmp directory in GB (0 = unlimited, default 50 GB)
    #[serde(default = "default_max_tmp_dir_gb")]
    pub max_tmp_dir_gb: f64,
    /// （"en-US"  "en-US"）/ UI language ("en-US" or "en-US")
    #[serde(default = "default_language")]
    pub language: String,
    /// Listen port
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    /// Mouflon Keys  Worker URL（）
    /// Mouflon Keys sync Worker URL (empty = auto-sync disabled)
    #[serde(default = "default_mouflon_sync_url")]
    pub mouflon_sync_url: Option<String>,
    /// Mouflon Keys  Worker  Token（ Worker  AUTH_TOKEN ）
    /// Mouflon Keys sync Worker auth token (corresponds to Worker's AUTH_TOKEN env var)
    #[serde(default)]
    pub mouflon_sync_token: Option<String>,
    /// （false =  Setup ）
    /// Whether the first-launch setup wizard has been completed (false = show Setup page)
    #[serde(default)]
    pub setup_done: bool,
}

/// Default value for Mouflon sync URL
fn default_mouflon_sync_url() -> Option<String> {
    Some("https://mouflon.chantrail.com".to_string())
}

/// Default value for merge format
fn default_merge_format() -> String {
    "mp4".to_string()
}

/// tmp （50 GB）/ Default value for max tmp dir size (50 GB)
fn default_max_tmp_dir_gb() -> f64 {
    50.0
}

/// Default value for language
fn default_language() -> String {
    "en-US".to_string()
}

/// Default value for server port
fn default_server_port() -> u16 {
    3030
}

/// ，。
/// Returns the directory containing the executable, used to locate config files and module directories.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

impl Default for Settings {
    fn default() -> Self {
        // recordings
        // Default output directory is the recordings folder next to the executable
        let output_dir = exe_dir().join("recordings").to_string_lossy().to_string();

        Self {
            output_dir,
            poll_interval_secs: 30,
            auto_record: true,
            api_proxy_url: None,
            cdn_proxy_url: None,
            sc_mirror_url: None,
            max_concurrent: 0,
            merge_format: default_merge_format(),
            max_tmp_dir_gb: default_max_tmp_dir_gb(),
            language: default_language(),
            server_port: default_server_port(),
            mouflon_sync_url: default_mouflon_sync_url(),
            mouflon_sync_token: None,
            setup_done: false,
        }
    }
}

/// All application data persisted to JSON files under the config/ directory
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppData {
    /// User settings
    pub settings: Settings,
    /// List of tracked streamers
    pub streamers: Vec<StreamerData>,
    /// Mouflon HLS （）/ Mouflon HLS decryption key store (keys + timestamps)
    #[serde(default)]
    pub mouflon_keys: MouflonKeysStore,
    /// Post-processing pipeline configuration
    #[serde(default)]
    pub pipeline: PipelineConfig,
    /// （，true/false  meta JSON ）
    /// List of video paths that have been post-processed (directory file; success/failure confirmed by reading the corresponding meta JSON)
    #[serde(default)]
    pub pp_results: Vec<String>,
}

/// Persisted data for a single streamer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamerData {
    /// （）/ Streamer username (lowercase)
    pub username: String,
    /// Whether auto-record is enabled
    pub auto_record: bool,
    /// （RFC 3339 ）/ Time added (RFC 3339 format)
    pub added_at: String,
}

/// ， `Arc<AppState>` 。
/// Global application runtime state, shared across modules via `Arc<AppState>`.
pub struct AppState {
    /// （）/ Persisted data (protected by read-write lock)
    pub data: RwLock<AppData>,
    /// （exe_dir/config/）/ Config directory path (exe_dir/config/)
    config_dir: PathBuf,
    /// （ -> ）/ Post-processing task status map (file path -> status)
    pub pp_tasks: RwLock<HashMap<String, PpTaskStatus>>,
    /// （ -> ）/ Post-processing cancel flags (file path -> atomic bool)
    pub pp_cancel_flags: RwLock<HashMap<String, Arc<AtomicBool>>>,
    /// Serial lock ensuring only one post-processing task runs at a time
    pub pp_lock: std::sync::Mutex<()>,
    /// Startup merge lock preventing concurrent startup merge and normal recording
    pub startup_lock: std::sync::Mutex<()>,
    /// poll_interval_secs （，）
    /// Sender to notify the monitor that poll_interval_secs has changed (optional, injected after startup)
    pub poll_interval_notify_tx: RwLock<Option<tokio::sync::mpsc::Sender<()>>>,
    /// Mouflon （，）
    /// Sender to notify the Mouflon sync scheduler to trigger an immediate sync (optional, injected after startup)
    pub mouflon_sync_notify_tx: RwLock<Option<tokio::sync::mpsc::Sender<()>>>,
}

impl AppState {
    /// （exe_dir/config/）。
    /// Returns the config directory path (exe_dir/config/).
    pub fn config_dir() -> PathBuf {
        exe_dir().join("config")
    }

    /// ，。
    /// Load configuration from disk and initialize application state, ensuring the output directory exists.
    pub fn new() -> Result<Arc<Self>> {
        let config_dir = Self::config_dir();
        fs::create_dir_all(&config_dir)?;

        // Load each section from split files
        let load_json = |name: &str| -> Option<String> {
            fs::read_to_string(config_dir.join(name)).ok()
        };

        let settings: Settings = load_json("settings.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let streamers: Vec<StreamerData> = load_json("streamers.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mouflon_keys: MouflonKeysStore = load_json("mouflon_keys.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let pipeline: PipelineConfig = load_json("pipeline.json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // pp_results.json （Vec<String>）
        // （HashMap<String, bool>）： Vec ， keys
        // pp_results.json stores a list of video paths that have been post-processed (Vec<String>)
        // Compatibility with old format (HashMap<String, bool>): if Vec parse fails, try old format and extract keys
        let pp_results: Vec<String> = load_json("pp_results.json")
            .and_then(|s| {
                serde_json::from_str::<Vec<String>>(&s).ok().or_else(|| {
                    serde_json::from_str::<HashMap<String, bool>>(&s)
                        .ok()
                        .map(|m| m.into_keys().collect())
                })
            })
            .unwrap_or_default();

        let data = AppData { settings, streamers, mouflon_keys, pipeline, pp_results };

        fs::create_dir_all(&data.settings.output_dir)?;

        Ok(Arc::new(Self {
            data: RwLock::new(data),
            config_dir,
            pp_tasks: RwLock::new(HashMap::new()),
            pp_cancel_flags: RwLock::new(HashMap::new()),
            pp_lock: std::sync::Mutex::new(()),
            startup_lock: std::sync::Mutex::new(()),
            poll_interval_notify_tx: RwLock::new(None),
            mouflon_sync_notify_tx: RwLock::new(None),
        }))
    }

    /// （ logs ）。
    /// Returns the log directory path (logs folder next to the executable).
    pub fn log_dir() -> PathBuf {
        exe_dir().join("logs")
    }

    /// `AppData` 。
    /// Serialize the current `AppData` into split config files.
    pub fn save(&self) -> Result<()> {
        let data = self.data.read();
        let dir = &self.config_dir;
        fs::write(dir.join("settings.json"), serde_json::to_string_pretty(&data.settings)?)?;
        fs::write(dir.join("streamers.json"), serde_json::to_string_pretty(&data.streamers)?)?;
        fs::write(dir.join("mouflon_keys.json"), serde_json::to_string_pretty(&data.mouflon_keys)?)?;
        fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&data.pipeline)?)?;
        fs::write(dir.join("pp_results.json"), serde_json::to_string_pretty(&data.pp_results)?)?;
        Ok(())
    }

    /// 。
    /// Get a cloned copy of the current settings.
    pub fn get_settings(&self) -> Settings {
        self.data.read().settings.clone()
    }

    /// ，。
    /// poll_interval_secs ，。
    /// mouflon_sync_url  mouflon_sync_token ，。
    ///
    /// Update settings and save to disk, also ensuring the new output directory exists.
    /// If poll_interval_secs changed, notify the monitor to restart its timer with the new interval.
    /// If mouflon_sync_url or mouflon_sync_token changed, notify the sync scheduler to trigger immediately.
    pub fn update_settings(&self, settings: Settings) -> Result<()> {
        fs::create_dir_all(&settings.output_dir)?;
        let old = self.data.read().settings.clone();
        let poll_interval_changed = old.poll_interval_secs != settings.poll_interval_secs;
        let mouflon_sync_changed = old.mouflon_sync_url != settings.mouflon_sync_url
            || old.mouflon_sync_token != settings.mouflon_sync_token;
        self.data.write().settings = settings;
        self.save()?;
        if poll_interval_changed
            && let Some(tx) = self.poll_interval_notify_tx.read().as_ref() {
            let _ = tx.try_send(());
        }
        if mouflon_sync_changed
            && let Some(tx) = self.mouflon_sync_notify_tx.read().as_ref() {
            let _ = tx.try_send(());
        }
        Ok(())
    }

    /// 。
    /// Get a cloned list of all tracked streamers.
    pub fn get_streamers(&self) -> Vec<StreamerData> {
        self.data.read().streamers.clone()
    }

    /// （）。
    /// Add a new streamer to the tracking list (returns error if already exists).
    pub fn add_streamer(&self, username: &str) -> Result<()> {
        let mut data = self.data.write();
        if data.streamers.iter().any(|s| s.username == username) {
            return Err(AppError::Other(format!("模特 {} 已存在", username)));
        }
        let auto_record = data.settings.auto_record;
        data.streamers.push(StreamerData {
            username: username.to_string(),
            auto_record,
            added_at: chrono::Utc::now().to_rfc3339(),
        });
        drop(data);
        self.save()
    }

    /// 。
    /// Remove a streamer from the tracking list and save.
    pub fn remove_streamer(&self, username: &str) -> Result<()> {
        let mut data = self.data.write();
        data.streamers.retain(|s| s.username != username);
        drop(data);
        self.save()
    }

    /// 。
    /// Set the auto-record toggle for a specific streamer and save.
    pub fn set_auto_record(&self, username: &str, enabled: bool) -> Result<()> {
        let mut data = self.data.write();
        if let Some(s) = data.streamers.iter_mut().find(|s| s.username == username) {
            s.auto_record = enabled;
        }
        drop(data);
        self.save()
    }

    /// Mouflon （ keys ，/）。
    /// Get a cloned copy of all Mouflon decryption keys (keys map only, for recording/relay use).
    pub fn get_mouflon_keys(&self) -> HashMap<String, String> {
        self.data.read().mouflon_keys.keys.clone()
    }

    /// Mouflon （），。
    /// Get the full Mouflon key store (including timestamps), for frontend display.
    pub fn get_mouflon_keys_store(&self) -> MouflonKeysStore {
        self.data.read().mouflon_keys.clone()
    }

    /// Mouflon ， manual_updated_at 。
    /// Add or update a Mouflon key pair, update manual_updated_at, and save.
    pub fn add_mouflon_key(&self, pkey: &str, pdkey: &str) -> Result<()> {
        let mut data = self.data.write();
        data.mouflon_keys.keys.insert(pkey.to_string(), pdkey.to_string());
        data.mouflon_keys.manual_updated_at = Some(chrono::Utc::now().to_rfc3339());
        drop(data);
        self.save()
    }

    /// pkey  Mouflon ， manual_updated_at 。
    /// Remove the Mouflon key with the given pkey, update manual_updated_at, and save.
    pub fn remove_mouflon_key(&self, pkey: &str) -> Result<()> {
        let mut data = self.data.write();
        data.mouflon_keys.keys.remove(pkey);
        data.mouflon_keys.manual_updated_at = Some(chrono::Utc::now().to_rfc3339());
        drop(data);
        self.save()
    }

    /// Cloudflare Worker  Mouflon 。
    /// Worker  updated_at  auto_synced_at：
    ///   -  → ， false（）
    ///   -  →  keys、 auto_synced_at， true（）
    ///
    /// Sync Mouflon keys from the Cloudflare Worker.
    /// Compares Worker's `updated_at` against local `auto_synced_at`:
    ///   - Equal   → skip, return false (no update needed)
    ///   - Different → overwrite keys, update auto_synced_at, return true (updated)
    pub async fn sync_mouflon_keys_from_worker(
        &self,
        worker_url: &str,
        auth_token: Option<&str>,
    ) -> Result<bool> {
        #[derive(Deserialize)]
        struct WorkerResponse {
            keys: HashMap<String, String>,
            updated_at: String,
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::Other(e.to_string()))?;

        let mut req = client.get(worker_url);
        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Other(format!("Worker 请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Other(format!(
                "Worker 返回错误状态: {}",
                resp.status()
            )));
        }

        let body: WorkerResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Other(format!("Worker 响应解析失败: {}", e)))?;

        // updated_at：，
        // Compare updated_at by parsing to a time point, avoiding false mismatches due to format differences
        let worker_ts = chrono::DateTime::parse_from_rfc3339(&body.updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok();

        let same_timestamp = {
            let data = self.data.read();
            match (&worker_ts, &data.mouflon_keys.auto_synced_at) {
                (Some(wt), Some(local)) => {
                    chrono::DateTime::parse_from_rfc3339(local)
                        .map(|lt| lt.with_timezone(&chrono::Utc) == *wt)
                        .unwrap_or(false)
                }
                _ => false,
            }
        };

        if same_timestamp {
            // Same timestamp, check for locally missing keys
            let missing: Vec<(String, String)> = {
                let data = self.data.read();
                body.keys
                    .iter()
                    .filter(|(pkey, _)| !data.mouflon_keys.keys.contains_key(pkey.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            };
            if missing.is_empty() {
                return Ok(false);
            }
            // Insert missing keys
            let mut data = self.data.write();
            for (pkey, pdkey) in missing {
                data.mouflon_keys.keys.insert(pkey, pdkey);
            }
            drop(data);
            self.save()?;
            return Ok(true);
        }

        // ，， auto_synced_at
        // Different timestamp: insert missing key pairs, update auto_synced_at
        {
            // Worker  chrono RFC 3339 ， manual_updated_at
            // Normalize Worker timestamp to chrono RFC 3339 format, consistent with manual_updated_at
            let normalized_at = chrono::DateTime::parse_from_rfc3339(&body.updated_at)
                .map(|dt| dt.with_timezone(&chrono::Utc).to_rfc3339())
                .unwrap_or(body.updated_at);
            let mut data = self.data.write();
            for (pkey, pdkey) in body.keys {
                data.mouflon_keys.keys.entry(pkey).or_insert(pdkey);
            }
            data.mouflon_keys.auto_synced_at = Some(normalized_at);
        }
        self.save()?;
        Ok(true)
    }

    /// 。
    /// Get a cloned copy of the current pipeline configuration.
    pub fn get_pipeline(&self) -> crate::postprocess::pipeline::PipelineConfig {
        self.data.read().pipeline.clone()
    }

    /// 。
    /// Update the pipeline configuration and save to disk.
    pub fn update_pipeline(&self, pipeline: crate::postprocess::pipeline::PipelineConfig) -> Result<()> {
        self.data.write().pipeline = pipeline;
        self.save()
    }

    /// 。
    /// Enqueue a post-processing task for the given file path.
    pub fn pp_task_enqueue(&self, path: &str) {
        self.pp_tasks.write().insert(
            path.to_string(),
            PpTaskStatus {
                path: path.to_string(),
                pct: 0.0,
                mod_done: 0,
                mod_total: 0,
                module_name: String::new(),
                done: 0,
                total: 0,
                status: "waiting".to_string(),
                from_memory: true,
            },
        );
        // （）/ Ensure cancel flag exists (don't overwrite if already present)
        self.pp_cancel_flags
            .write()
            .entry(path.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)));
    }

    /// 。
    /// Mark the post-processing task for the given file path as running.
    pub fn pp_task_start(&self, path: &str, total: usize) {
        self.pp_tasks.write().insert(
            path.to_string(),
            PpTaskStatus {
                path: path.to_string(),
                pct: 0.0,
                mod_done: 0,
                mod_total: 0,
                module_name: String::new(),
                done: 0,
                total,
                status: "running".to_string(),
                from_memory: true,
            },
        );
    }

    /// 。
    /// Get or create the cancel flag for the given file path.
    pub fn pp_task_make_cancel_flag(&self, path: &str) -> Arc<AtomicBool> {
        let mut flags = self.pp_cancel_flags.write();
        if let Some(existing) = flags.get(path) {
            return Arc::clone(existing);
        }
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(path.to_string(), Arc::clone(&flag));
        flag
    }

    /// true，。
    /// Set the cancel flag for the given file path to true, requesting post-processing abort.
    pub fn pp_task_cancel(&self, path: &str) {
        if let Some(flag) = self.pp_cancel_flags.read().get(path) {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// （）。
    /// Clear the cancel flag for the given file path (called after task completes).
    pub fn pp_task_clear_cancel_flag(&self, path: &str) {
        self.pp_cancel_flags.write().remove(path);
    }

    /// 。
    /// Update the post-processing progress for the given file path.
    #[allow(clippy::too_many_arguments)]
    pub fn pp_task_progress(
        &self,
        path: &str,
        pct: f64,
        mod_done: u32,
        mod_total: u32,
        module_name: &str,
        done: usize,
        total: usize,
    ) {
        if let Some(t) = self.pp_tasks.write().get_mut(path) {
            t.pct = pct;
            t.mod_done = mod_done;
            t.mod_total = mod_total;
            t.module_name = module_name.to_string();
            t.done = done;
            t.total = total;
        }
    }

    /// 。/ meta JSON  status ，
    /// pp_results （）。
    ///
    /// Mark the post-processing task as done or failed. Success/failure is confirmed by the
    /// corresponding meta JSON's status field; here we only record the path in the pp_results
    /// directory file (for quick lookup of whether post-processing has been run).
    pub fn pp_task_finish(&self, path: &str, success: bool) {
        if let Some(t) = self.pp_tasks.write().get_mut(path) {
            t.status = if success { "done" } else { "error" }.to_string();
            t.pct = if success { 100.0 } else { t.pct };
        }
        // （）/ Add path to directory list (deduplicated)
        {
            let mut data = self.data.write();
            if !data.pp_results.contains(&path.to_string()) {
                data.pp_results.push(path.to_string());
            }
        }
        let _ = self.save();
    }

    /// ，。
    /// meta JSON  status /。
    ///
    /// Get a list of all post-processing task statuses, merging in-memory runtime state with persisted historical results.
    /// Historical results are confirmed by reading the status field from the corresponding meta JSON.
    pub fn get_pp_tasks(&self) -> Vec<PpTaskStatus> {
        let mut tasks: HashMap<String, PpTaskStatus> = self.pp_tasks.read().clone();

        // pp_results ， meta  success/failure
        // Supplement historical tasks from pp_results directory, confirming success/failure via meta
        for path in self.data.read().pp_results.iter() {
            if tasks.contains_key(path) {
                continue;
            }
            let video_path = std::path::Path::new(path);
            let success = crate::recording::meta::read_meta(video_path)
                .map(|m| m.status == "finish")
                .unwrap_or(false);
            tasks.insert(path.clone(), PpTaskStatus {
                path: path.clone(),
                pct: if success { 100.0 } else { 0.0 },
                mod_done: 0,
                mod_total: 0,
                module_name: String::new(),
                done: 0,
                total: 0,
                status: if success { "done" } else { "error" }.to_string(),
                from_memory: false,
            });
        }

        tasks.into_values().collect()
    }
}

/// ：，。
/// ， emitter  `startup-warnings` 。
///
/// Perform a single config check: verify all tracked streamers still exist,
/// and check for orphaned post-processing records.
/// If issues are found, emit a `startup-warnings` event to the frontend via the emitter.
pub async fn run_config_check(state: &Arc<AppState>, emitter: &Arc<dyn crate::core::emitter::Emitter>) {
    use crate::core::emitter::EmitterExt;
    use crate::core::error::AppError;

    let settings = state.get_settings();
    let streamers = state.get_streamers();

    let api = match crate::streaming::stripchat::StripchatApi::new_api_only(
        settings.api_proxy_url.as_deref(),
        settings.cdn_proxy_url.as_deref(),
        settings.sc_mirror_url.as_deref(),
    ) {
        Ok(a) => a,
        Err(_) => return,
    };

    // 3 ， 10 ，
    // Retry up to 3 times per streamer with 10s delay; only add to missing list after confirmed
    const MAX_ATTEMPTS: u32 = 3;
    const RETRY_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(10);

    let mut missing_streamers = Vec::new();
    for s in &streamers {
        let mut confirmed_missing = false;
        for attempt in 1..=MAX_ATTEMPTS {
            match api.get_stream_info(&s.username, false).await {
                Ok(_) => {
                    confirmed_missing = false;
                    break;
                }
                Err(AppError::UserNotFound(_)) => {
                    confirmed_missing = true;
                    break;
                }
                Err(_) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_DELAY).await;
                    } else {
                        confirmed_missing = true;
                    }
                }
            }
        }
        if confirmed_missing {
            missing_streamers.push(s.username.clone());
        }
    }

    // pp_results
    // Find orphaned pp_results entries whose corresponding files no longer exist
    let missing_pp_results: Vec<String> = state
        .data
        .read()
        .pp_results
        .iter()
        .filter(|p| !std::path::Path::new(p.as_str()).exists())
        .cloned()
        .collect();

    if !missing_streamers.is_empty() || !missing_pp_results.is_empty() {
        emitter.emit(
            "startup-warnings",
            &serde_json::json!({
                "missing_streamers": missing_streamers,
                "missing_pp_results": missing_pp_results,
            }),
        );
    }
}

/// ：，。
/// Start the config check scheduler: run once immediately, then once every day at midnight.
pub async fn schedule_config_checks(state: Arc<AppState>, emitter: Arc<dyn crate::core::emitter::Emitter>) {
    run_config_check(&state, &emitter).await;

    loop {
        // Calculate seconds until next midnight
        let now = chrono::Local::now();
        let secs_until = {
            let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
            let midnight = tomorrow.and_hms_opt(0, 0, 0).unwrap();
            let midnight_local = midnight
                .and_local_timezone(chrono::Local)
                .single()
                .unwrap_or_else(|| now + chrono::Duration::hours(24));
            (midnight_local - now).num_seconds().max(0) as u64
        };
        tokio::time::sleep(tokio::time::Duration::from_secs(secs_until)).await;
        run_config_check(&state, &emitter).await;
    }
}

/// Mouflon Keys ：，。
/// Settings  mouflon_sync_url，。
///
/// Start the Mouflon Keys auto-sync scheduler: sync once on startup, then every hour.
/// Silently skips if mouflon_sync_url is not configured in Settings.
pub async fn schedule_mouflon_sync(
    state: Arc<AppState>,
    emitter: Arc<dyn crate::core::emitter::Emitter>,
    mut notify_rx: tokio::sync::mpsc::Receiver<()>,
) {
    use crate::core::emitter::EmitterExt;
    const INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(3600);

    loop {
        let settings = state.get_settings();
        if let Some(url) = settings.mouflon_sync_url.as_deref().filter(|u| !u.is_empty()) {
            let token = settings.mouflon_sync_token.as_deref();
            match state.sync_mouflon_keys_from_worker(url, token).await {
                Ok(true) => {
                    tracing::info!("Mouflon keys synced from {}", url);
                    emitter.emit(
                        "mouflon-keys-updated",
                        &state.get_mouflon_keys_store(),
                    );
                }
                Ok(false) => {
                    tracing::debug!("Mouflon keys up-to-date, skipped");
                }
                Err(e) => {
                    tracing::warn!("Mouflon keys sync failed: {}", e);
                }
            }
        }
        // 1 ，
        // Wait 1 hour, or until an immediate sync notification arrives
        tokio::select! {
            _ = tokio::time::sleep(INTERVAL) => {}
            _ = notify_rx.recv() => {
                tracing::info!("Mouflon sync: settings changed, triggering immediate sync");
            }
        }
    }
}
