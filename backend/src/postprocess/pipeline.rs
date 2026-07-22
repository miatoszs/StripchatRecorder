//! Post-processing Pipeline Engine
//!
//! modules/ 、，。
//! ，， stdout 。
//!
//! Responsible for discovering post-processing modules in the modules/ directory,
//! executing pipeline nodes, and managing module process lifecycles.
//! Each module is a standalone executable that receives parameters via environment variables
//! and reports progress and output paths via stdout.

use crate::config::settings::exe_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// （ `--describe` ）/ Module parameter definition (deserialized from `--describe` output)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    /// Parameter key
    pub key: String,
    /// Parameter display label
    pub label: String,
    /// "select"）/ Parameter type
    pub r#type: String,
    /// Parameter default value
    pub default: serde_json::Value,
    /// Options for select type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// （ `--describe` ）/ Post-processing module info (deserialized from `--describe` output)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
    /// Module unique ID
    pub id: String,
    /// Module display name
    pub name: String,
    /// Module description
    pub description: String,
    /// Module parameter definitions
    pub params: Vec<ParamDef>,
    /// （，key  "en-US"）/ i18n translations (optional, key is locale like "en-US")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i18n: Option<serde_json::Value>,
    /// （，）/ Module executable path (not serialized, filled at runtime)
    #[serde(skip)]
    pub exe_path: PathBuf,
}

/// （）/ Pipeline node (module instance)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineNode {
    /// ID（UUID）/ Node unique ID (UUID)
    pub node_id: String,
    /// Corresponding module ID
    pub module_id: String,
    /// Node parameter values
    pub params: HashMap<String, serde_json::Value>,
    /// Whether this node is enabled
    pub enabled: bool,
}

/// Pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    /// Ordered list of nodes
    pub nodes: Vec<PipelineNode>,
}

/// （ modules/ ）。
/// Returns the modules directory (modules/ folder next to the executable).
pub fn modules_dir() -> PathBuf {
    exe_dir().join("modules")
}

/// modules/ ，。
/// `--describe` 。
///
/// Scan the modules/ directory to discover all available post-processing modules.
/// Calls `--describe` on each executable to get module metadata.
pub fn discover_modules() -> Vec<ModuleInfo> {
    let dir = modules_dir();
    if !dir.exists() {
        return vec![];
    }

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut modules = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Platform-specific executable detection
        #[cfg(target_os = "windows")]
        let is_exec = path.extension().and_then(|e| e.to_str()) == Some("exe");
        #[cfg(not(target_os = "windows"))]
        let is_exec = {
            use std::os::unix::fs::PermissionsExt;
            path.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        };

        if !is_exec {
            continue;
        }

        match describe_module(&path) {
            Ok(mut info) => {
                info.exe_path = path;
                modules.push(info);
            }
            Err(e) => {
                tracing::error!("Failed to describe module {:?}: {}", path, e);
            }
        }
    }

    modules
}

/// `--describe` ，。
/// Call the module executable with `--describe` and parse the returned module metadata.
fn describe_module(exe: &PathBuf) -> crate::core::error::Result<ModuleInfo> {
    let output = std::process::Command::new(exe)
        .arg("--describe")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| crate::core::error::AppError::Other(format!("spawn: {}", e)))?;

    if !output.status.success() {
        return Err(crate::core::error::AppError::Other(format!(
            "exit {}",
            output.status
        )));
    }

    let info: ModuleInfo = serde_json::from_slice(&output.stdout)
        .map_err(|e| crate::core::error::AppError::Other(format!("json: {}", e)))?;

    Ok(info)
}

/// Execution result of a single node
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeResult {
    /// Node ID
    pub node_id: String,
    /// Module ID
    pub module_id: String,
    /// Whether execution succeeded
    pub success: bool,
    /// （ stdout，）/ Result message (last stdout line on success, error message on failure)
    pub message: String,
    /// （ `OUTPUT:` ，）/ Module output file path (parsed from `OUTPUT:` prefix line, not serialized)
    #[serde(skip)]
    pub output: Option<PathBuf>,
    /// （ `DELETE_INPUT` ，）
    /// Whether the module requested the host to delete its input file (parsed from `DELETE_INPUT` protocol line, not serialized)
    #[serde(skip)]
    pub delete_input: bool,
}

/// ，。
/// ；（ filter_short ），。
///
/// Execute the post-processing pipeline, running all enabled nodes in sequence.
/// Each node's output path becomes the next node's input; if a node has no output
/// (e.g., filter_short deletes the file), the pipeline terminates.
///
/// Parameters
/// Initial input video path
/// Pipeline configuration
/// Available module list
/// Optional cancel flag
/// - `max_tmp_dir_gb`: tmp （GB，0 = ）/ Max tmp dir size in GB (0 = unlimited)
/// - `on_progress`: （, , , , , ）/ Progress callback
/// - `on_log`: （ ID, , ）/ Log callback
pub fn run_pipeline(
    video_path: &std::path::Path,
    pipeline: &PipelineConfig,
    modules: &[ModuleInfo],
    cancel: Option<Arc<AtomicBool>>,
    max_tmp_dir_gb: f64,
    on_progress: impl Fn(usize, usize, u32, u32, &str, &str),
    on_log: impl Fn(&str, &str, &str),
) -> Vec<NodeResult> {
    let mut results = Vec::new();
    let mut current_input = video_path.to_path_buf();

    let all_nodes: Vec<&PipelineNode> = pipeline.nodes.iter().collect();
    let total = all_nodes.iter().filter(|n| n.enabled).count();
    let mut done = 0usize;

    for node in all_nodes.iter() {
        if !node.enabled {
            continue;
        }

        // Check cancel flag
        if cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
            break;
        }

        let module = match modules.iter().find(|m| m.id == node.module_id) {
            Some(m) => m,
            None => {
                // Module missing, abort the entire pipeline
                results.push(NodeResult {
                    node_id: node.node_id.clone(),
                    module_id: node.module_id.clone(),
                    success: false,
                    message: format!("模块 '{}' 不存在，请检查 modules/ 目录", node.module_id),
                    output: None,
                    delete_input: false,
                });
                done += 1;
                on_progress(done, total, 0, 0, &node.module_id, "");
                break;
            }
        };

        let cur_done = done;
        let module_name = module.name.clone();
        let module_id_for_log = node.module_id.clone();
        let status_text = std::sync::Mutex::new(String::new());
        let last_mod = std::sync::Mutex::new((0u32, 0u32));
        let result = run_node(
            module,
            node,
            &current_input,
            cancel.clone(),
            max_tmp_dir_gb,
            &|md, mt| {
                *last_mod.lock().unwrap() = (md, mt);
                let st = status_text.lock().unwrap().clone();
                on_progress(cur_done, total, md, mt, &module_name, &st);
            },
            &|stream, line| {
                on_log(&module_id_for_log, stream, line);
            },
            &|st| {
                *status_text.lock().unwrap() = st.to_string();
                let (md, mt) = *last_mod.lock().unwrap();
                on_progress(cur_done, total, md, mt, &module_name, st);
            },
        );

        done += 1;
        on_progress(done, total, 0, 0, &module_name, "");

        // ，（ meta ）
        // If the module requested deletion of its input file, the host performs the deletion
        // (also cleaning up the corresponding meta file)
        if result.delete_input {
            if let Err(e) = std::fs::remove_file(&current_input) {
                tracing::warn!("DELETE_INPUT: failed to remove {:?}: {}", current_input, e);
            } else {
                tracing::info!("DELETE_INPUT: removed {:?}", current_input);
                crate::recording::meta::delete_meta(&current_input);
            }
        }

        match &result.output {
            Some(out) => {
                // Node has output, continue to next node
                current_input = out.clone();
                results.push(result);
            }
            None if result.success => {
                // （），
                // Node succeeded but has no output (module requested input deletion), terminate pipeline
                results.push(result);
                break;
            }
            None => {
                // Node failed, terminate pipeline
                results.push(result);
                break;
            }
        }
    }

    results
}

/// （， stdout/stderr，）。
/// Execute a single pipeline node (spawn subprocess, read stdout/stderr, handle cancellation).
///
/// Parameters
/// - `module`: （）/ Module info (including executable path)
/// - `node`: （）/ Node configuration (including parameters)
/// Input file path
/// Optional cancel flag
/// - `max_tmp_dir_gb`: tmp （GB，0 = ）/ Max tmp dir size in GB (0 = unlimited)
/// Module-level progress callback
/// Log line callback
/// - `on_status`: （ `STATUS:` ）/ Status text callback (from `STATUS:` prefix lines)
#[allow(clippy::too_many_arguments)]
fn run_node(
    module: &ModuleInfo,
    node: &PipelineNode,
    input: &std::path::Path,
    cancel: Option<Arc<AtomicBool>>,
    max_tmp_dir_gb: f64,
    on_module_progress: &dyn Fn(u32, u32),
    on_log: &dyn Fn(&str, &str),
    on_status: &dyn Fn(&str),
) -> NodeResult {
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    /// Subprocess output stream events
    enum StreamEvent {
        StdoutLine(String),
        StderrLine(String),
        StdoutEof,
        StderrEof,
    }

    let mut cmd = std::process::Command::new(&module.exe_path);
    // PP_MAX_TMP_MB  MB （GB * 1024，）
    // Pass PP_MAX_TMP_MB to the module in MB (GB * 1024, truncated)
    let max_tmp_mb = (max_tmp_dir_gb * 1024.0) as u64;
    cmd.env("PP_INPUT", input)
        .env("PP_EXE_DIR", exe_dir())
        .env("PP_MAX_TMP_MB", max_tmp_mb.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Convert node params to PP_PARAM_{KEY} env vars
    for (key, val) in &node.params {
        let env_key = format!("PP_PARAM_{}", key.to_uppercase());
        let env_val = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        cmd.env(env_key, env_val);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return NodeResult {
                node_id: node.node_id.clone(),
                module_id: node.module_id.clone(),
                success: false,
                message: format!("Failed to spawn: {}", e),
                output: None,
                delete_input: false,
            }
        }
    };

    let mut last_message = String::new();
    let mut stderr_msg = String::new();
    let mut panic_msg = String::new();
    let mut output_path: Option<PathBuf> = None;
    let mut delete_input = false;
    let mut cancelled = false;

    let module_id = &node.module_id;

    // channel  stdout/stderr
    // Use a channel to funnel stdout/stderr line events into the main loop
    let (tx, rx) = mpsc::channel::<StreamEvent>();
    let mut stdout_done = true;
    let mut stderr_done = true;

    if let Some(stdout) = child.stdout.take() {
        stdout_done = false;
        let tx_stdout = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx_stdout.send(StreamEvent::StdoutLine(line)).is_err() {
                    return;
                }
            }
            let _ = tx_stdout.send(StreamEvent::StdoutEof);
        });
    }

    if let Some(stderr) = child.stderr.take() {
        stderr_done = false;
        let tx_stderr = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx_stderr.send(StreamEvent::StderrLine(line)).is_err() {
                    return;
                }
            }
            let _ = tx_stderr.send(StreamEvent::StderrEof);
        });
    }

    drop(tx);

    while !(stdout_done && stderr_done) {
        // Check cancel flag every 100ms
        if cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
            // Use taskkill on Windows to force-kill the process tree
            #[cfg(target_os = "windows")]
            {
                let pid = child.id();
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            cancelled = true;
            break;
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(StreamEvent::StdoutLine(line)) => {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("PROGRESS:") {
                    // Parse PROGRESS:{done}/{total} format
                    let mut parts = rest.splitn(2, '/');
                    if let (Some(d), Some(t)) = (parts.next(), parts.next())
                        && let (Ok(done), Ok(total)) =
                            (d.trim().parse::<u32>(), t.trim().parse::<u32>())
                    {
                        on_module_progress(done, total);
                    }
                } else if let Some(status_text) = trimmed.strip_prefix("STATUS:") {
                    // STATUS:{text} （）/ Parse STATUS:{text} format (upload speed, etc.)
                    on_log("status", status_text.trim());
                    on_status(status_text.trim());
                } else if let Some(path) = trimmed.strip_prefix("OUTPUT:") {
                    // Parse OUTPUT:{path} format
                    output_path = Some(PathBuf::from(path.trim()));
                } else if trimmed == "DELETE_INPUT" {
                    // Module requests host to delete its input file
                    delete_input = true;
                } else if !trimmed.is_empty() {
                    tracing::info!("[{}] {}", module_id, trimmed);
                    on_log("stdout", trimmed);
                    last_message = trimmed.to_string();
                }
            }
            Ok(StreamEvent::StderrLine(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Filter Rust panic BACKTRACE hint lines
                if trimmed.starts_with("note: run with `RUST_BACKTRACE") {
                    continue;
                }
                if trimmed.contains("panicked at") && panic_msg.is_empty() {
                    panic_msg = trimmed.to_string();
                }
                tracing::warn!("[{}] stderr: {}", module_id, trimmed);
                on_log("stderr", trimmed);
                stderr_msg = trimmed.to_string();
            }
            Ok(StreamEvent::StdoutEof) => {
                stdout_done = true;
            }
            Ok(StreamEvent::StderrEof) => {
                stderr_done = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                stdout_done = true;
                stderr_done = true;
            }
        }
    }

    if cancelled {
        let _ = child.wait();
        return NodeResult {
            node_id: node.node_id.clone(),
            module_id: node.module_id.clone(),
            success: false,
            message: "cancelled".to_string(),
            output: None,
            delete_input: false,
        };
    }

    // Panic message takes priority over regular stderr message
    if !panic_msg.is_empty() {
        stderr_msg = panic_msg;
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return NodeResult {
                node_id: node.node_id.clone(),
                module_id: node.module_id.clone(),
                success: false,
                message: format!("wait failed: {}", e),
                output: None,
                delete_input: false,
            }
        }
    };

    let success = status.success();
    let message = if success {
        if last_message.is_empty() {
            "OK".to_string()
        } else {
            last_message
        }
    } else if !stderr_msg.is_empty() {
        stderr_msg
    } else if !last_message.is_empty() {
        last_message
    } else {
        format!("exit {}", status)
    };

    NodeResult {
        node_id: node.node_id.clone(),
        module_id: node.module_id.clone(),
        success,
        message,
        output: if success { output_path } else { None },
        delete_input: success && delete_input,
    }
}
