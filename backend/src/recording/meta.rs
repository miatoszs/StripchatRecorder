//! Video Metadata File Management
//!
//! JSON ， `.{stem}.json`。
//! ，// `status` 。
//!
//! Each recording has a hidden JSON metadata file named `.{stem}.json`.
//! The file is created when recording starts and its `status` field is updated
//! throughout the recording / merging / post-processing lifecycle.
//!
//! Status lifecycle
//!
//! ```
//! recording → merging_waiting → merging → pp_waiting → pp_running → finish
//!                                       ↘ finish (no pipeline)    ↘ pp_error
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// meta 。
/// `VideoMeta` ，
/// meta 。
///
/// Current meta file format version.
/// Increment this whenever a breaking change is made to `VideoMeta`;
/// the periodic scanner will rebuild any meta file whose version doesn't match.
pub const META_VERSION: u32 = 2;

/// stem（：`{name}_{YYYYMMDD}_{HHmmss}`）。
/// Parse the recording start time from a filename stem (format: `{name}_{YYYYMMDD}_{HHmmss}`).
pub fn parse_timestamp_from_stem(stem: &str) -> Option<String> {
    use chrono::TimeZone;
    let parts: Vec<&str> = stem.rsplitn(3, '_').collect();
    if parts.len() < 2 {
        return None;
    }
    let time_part = parts[0];
    let date_part = parts[1];
    if date_part.len() == 8 && time_part.len() == 6 {
        let combined = format!("{}{}", date_part, time_part);
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&combined, "%Y%m%d%H%M%S") {
            let local = chrono::Local.from_local_datetime(&dt).single()?;
            return Some(local.to_rfc3339());
        }
    }
    None
}

/// Execution result of a post-processing module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpModuleResult {
    /// Module ID
    pub module_id: String,
    /// Whether succeeded
    pub success: bool,
    /// Result message
    pub message: String,
}

/// ， `.{stem}.json` 。
/// Video metadata persisted to `.{stem}.json`.
///
/// `status` ，：
/// - `"recording"`       —
/// - `"merging_waiting"` — （）
/// - `"merging"`         —  TS
/// - `"pp_waiting"`      — （）
/// - `"pp_running"`      —
/// - `"pp_error"`        —
/// - `"finish"`          —
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMeta {
    /// meta ，。
    /// 0，。
    ///
    /// Meta format version, used to detect when a rebuild is needed after structural changes.
    /// Deserializes to 0 when absent, triggering a rebuild.
    #[serde(default)]
    pub meta_version: u32,

    /// Current processing status
    pub status: String,

    /// （RFC 3339 ）/ Recording start time (RFC 3339 format)
    pub started_at: String,

    /// （）/ File size (bytes)
    pub size_bytes: u64,

    /// （， ffprobe ）/ Actual video duration (seconds, filled by ffprobe after merge)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_duration_secs: Option<u64>,

    /// （）/ Per-module post-processing results (filled after completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pp_results: Option<Vec<PpModuleResult>>,

    /// （ ID -> ）/ Module output paths (module ID -> output file path)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_outputs: Option<std::collections::HashMap<String, String>>,
}

/// （`.{stem}.json`）。
/// Compute the metadata file path for a given video file path (`.{stem}.json`).
pub fn meta_path_for(video_path: &Path) -> Option<PathBuf> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?;
    Some(parent.join(format!(".{}.json", stem)))
}

/// ， `None`。
/// Read the metadata for a video file; returns `None` if the file doesn't exist or fails to parse.
pub fn read_meta(video_path: &Path) -> Option<VideoMeta> {
    let meta_path = meta_path_for(video_path)?;
    let content = std::fs::read_to_string(&meta_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// `.{stem}.json` 。
/// `meta_version` 。
///
/// Write metadata to the `.{stem}.json` file for a video file.
/// Automatically sets `meta_version` to the current version constant before writing.
pub fn write_meta(video_path: &Path, meta: &VideoMeta) {
    let Some(meta_path) = meta_path_for(video_path) else {
        return;
    };
    let mut meta = meta.clone();
    meta.meta_version = META_VERSION;
    match serde_json::to_string_pretty(&meta) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&meta_path, json) {
                tracing::warn!("Failed to write meta {:?}: {}", meta_path, e);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to serialize meta for {:?}: {}", video_path, e);
        }
    }
}

/// （）。
/// Delete the metadata file for a video file (if it exists).
pub fn delete_meta(video_path: &Path) {
    if let Some(meta_path) = meta_path_for(video_path)
        && meta_path.exists() {
        let _ = std::fs::remove_file(&meta_path);
    }
}

/// meta  `status` ，。
/// meta 。
///
/// Update only the `status` field of the meta file, leaving other fields unchanged.
/// If the meta file doesn't exist, rebuilds it from video file info before writing.
pub fn set_status(video_path: &Path, status: &str) {
    let mut meta = match read_meta(video_path) {
        Some(m) => m,
        None => {
            let size_bytes = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
            let stem = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let started_at = parse_timestamp_from_stem(stem).unwrap_or_else(|| {
                std::fs::metadata(video_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Local> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default()
            });
            VideoMeta {
                meta_version: META_VERSION,
                status: status.to_string(),
                started_at,
                size_bytes,
                video_duration_secs: None,
                pp_results: None,
                module_outputs: None,
            }
        }
    };
    meta.status = status.to_string();
    write_meta(video_path, &meta);
}

/// meta：、。
/// Update meta when post-processing completes: write final status, module results, and output paths.
pub fn set_pp_done(
    video_path: &Path,
    status: &str,
    results: Vec<PpModuleResult>,
    module_outputs: std::collections::HashMap<String, String>,
) {
    let mut meta = match read_meta(video_path) {
        Some(m) => m,
        None => {
            let size_bytes = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
            let stem = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let started_at = parse_timestamp_from_stem(stem).unwrap_or_else(|| {
                std::fs::metadata(video_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Local> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default()
            });
            VideoMeta {
                meta_version: META_VERSION,
                status: status.to_string(),
                started_at,
                size_bytes,
                video_duration_secs: None,
                pp_results: None,
                module_outputs: None,
            }
        }
    };
    meta.status = status.to_string();
    meta.pp_results = Some(results);
    if !module_outputs.is_empty() {
        meta.module_outputs = Some(module_outputs);
    }
    write_meta(video_path, &meta);
}

/// meta ， meta（）。
/// Does not overwrite if the meta file already exists; otherwise creates an initial meta
/// (used as a safety net for leftover segments on startup).
pub fn ensure_meta(video_path: &Path, started_at: &str) {
    if let Some(meta_path) = meta_path_for(video_path)
        && meta_path.exists() {
        return;
    }
    let size_bytes = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
    let meta = VideoMeta {
        meta_version: META_VERSION,
        status: "merging_waiting".to_string(),
        started_at: started_at.to_string(),
        size_bytes,
        video_duration_secs: None,
        pp_results: None,
        module_outputs: None,
    };
    write_meta(video_path, &meta);
}

/// ， meta ， `video_duration_secs`。
/// On startup, scan the output directory and write missing meta files for all videos,
/// also filling in missing `video_duration_secs` via ffprobe.
pub fn startup_ensure_meta_files(
    output_dir: &Path,
    merge_format: &str,
) {
    ensure_meta_files(output_dir, merge_format);
}

/// ， meta /，
/// `video_duration_secs`。
/// （//） meta，。
///
/// Scan the output directory and create/rebuild meta files that are missing or have an
/// outdated version, also filling in missing `video_duration_secs` via ffprobe.
/// Skips meta files in active states (recording/merging/post-processing) to avoid
/// interfering with ongoing tasks.
pub fn ensure_meta_files(output_dir: &Path, merge_format: &str) {
    if !output_dir.exists() {
        return;
    }
    let mut count_created = 0usize;
    let mut count_updated = 0usize;
    scan_and_ensure_meta(
        output_dir,
        merge_format,
        &mut count_created,
        &mut count_updated,
    );
    if count_created > 0 || count_updated > 0 {
        tracing::info!(
            "Meta scan: created {} new, rebuilt/updated {} existing meta files",
            count_created,
            count_updated
        );
    }
}

fn scan_and_ensure_meta(
    dir: &Path,
    merge_format: &str,
    count_created: &mut usize,
    count_updated: &mut usize,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            scan_and_ensure_meta(&path, merge_format, count_created, count_updated);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || ext != merge_format {
                continue;
            }

            match read_meta(&path) {
                None => {
                    // meta ：
                    // Meta missing: create it
                    let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let started_at = parse_timestamp_from_stem(stem).unwrap_or_else(|| {
                        std::fs::metadata(&path)
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .map(|t| {
                                let dt: chrono::DateTime<chrono::Local> = t.into();
                                dt.to_rfc3339()
                            })
                            .unwrap_or_default()
                    });
                    let video_duration_secs = crate::recording::recorder::get_video_duration(&path);
                    let meta = VideoMeta {
                        meta_version: META_VERSION,
                        status: "finish".to_string(),
                        started_at,
                        size_bytes,
                        video_duration_secs,
                        pp_results: None,
                        module_outputs: None,
                    };
                    write_meta(&path, &meta);
                    *count_created += 1;
                }
                Some(mut meta) => {
                    // meta ，
                    // Skip active meta to avoid interfering with ongoing tasks
                    if matches!(
                        meta.status.as_str(),
                        "recording" | "merging_waiting" | "merging" | "pp_waiting" | "pp_running"
                    ) {
                        continue;
                    }

                    let mut changed = false;

                    // ： meta，
                    // Version mismatch: rebuild meta, preserving reusable fields
                    if meta.meta_version != META_VERSION {
                        tracing::info!(
                            "Meta version mismatch for {:?}: found {}, expected {} — rebuilding",
                            path,
                            meta.meta_version,
                            META_VERSION
                        );
                        // size_bytes  video_duration_secs
                        // Re-read size_bytes and video_duration_secs from disk
                        meta.size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(meta.size_bytes);
                        if meta.video_duration_secs.is_none() {
                            meta.video_duration_secs = crate::recording::recorder::get_video_duration(&path);
                        }
                        // finish（）
                        // Correct transient status to finish (crash remnant during version upgrade)
                        if matches!(
                            meta.status.as_str(),
                            "recording" | "merging_waiting" | "merging" | "pp_waiting" | "pp_running"
                        ) {
                            meta.status = "finish".to_string();
                        }
                        // meta_version  write_meta  META_VERSION
                        // meta_version is set to META_VERSION automatically by write_meta
                        write_meta(&path, &meta);
                        *count_updated += 1;
                        continue;
                    }

                    // ：
                    // Version matches: only fill in missing fields
                    if meta.video_duration_secs.is_none() {
                        meta.video_duration_secs = crate::recording::recorder::get_video_duration(&path);
                        changed = true;
                    }
                    if changed {
                        write_meta(&path, &meta);
                        *count_updated += 1;
                    }
                }
            }
        }
    }
}

/// ， meta （`.{stem}.json`）。
/// Scan the output directory and delete orphaned meta files (`.{stem}.json`) whose
/// corresponding video files no longer exist.
///
/// 。
/// Returns the number of deleted files.
pub fn cleanup_orphaned_meta_files(output_dir: &Path) -> usize {
    if !output_dir.exists() {
        return 0;
    }
    let mut count = 0usize;
    cleanup_orphaned_meta_recursive(output_dir, &mut count);
    if count > 0 {
        tracing::info!("Meta cleanup: deleted {} orphaned meta file(s)", count);
    }
    count
}

fn cleanup_orphaned_meta_recursive(dir: &Path, count: &mut usize) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') {
                cleanup_orphaned_meta_recursive(&path, count);
            }
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // meta ： '.' ， '.json'
        // Meta file format: starts with '.', ends with '.json'
        if !name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }

        // meta  stem： '.'  '.json'
        // Recover video stem from meta filename: strip leading '.' and trailing '.json'
        let stem = &name[1..name.len() - 5]; // ".{stem}.json" → "{stem}"
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };

        // Check whether a video file with the same stem exists (any extension)
        let video_exists = std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let p = e.path();
                if !p.is_file() {
                    return false;
                }
                let vname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if vname.starts_with('.') {
                    return false;
                }
                p.file_stem().and_then(|s| s.to_str()) == Some(stem)
            });

        if !video_exists {
            // meta（//），
            // ，。
            // Skip meta files that are still in an active state (recording / merging / post-processing).
            // The video file doesn't exist yet at these stages, so they are not truly orphaned.
            let is_active = std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<VideoMeta>(&c).ok())
                .map(|m| {
                    matches!(
                        m.status.as_str(),
                        "recording" | "merging_waiting" | "merging" | "pp_waiting" | "pp_running"
                    )
                })
                .unwrap_or(false);

            if is_active {
                tracing::debug!("Meta cleanup: skipping active meta {:?}", path);
                continue;
            }

            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Meta cleanup: failed to delete {:?}: {}", path, e);
            } else {
                tracing::info!("Meta cleanup: deleted orphaned meta {:?}", path);
                *count += 1;
            }
        }
    }
}

/// meta ：，。
/// Start the orphaned meta cleanup scheduler: run once immediately, then once every hour.
pub async fn schedule_meta_cleanup(output_dir: std::path::PathBuf) {
    // Run once immediately
    let dir = output_dir.clone();
    tokio::task::spawn_blocking(move || {
        cleanup_orphaned_meta_files(&dir);
    })
    .await
    .ok();

    // Then run every hour
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        let dir = output_dir.clone();
        tokio::task::spawn_blocking(move || {
            cleanup_orphaned_meta_files(&dir);
        })
        .await
        .ok();
    }
}

/// meta ：，。
/// meta ，。
///
/// Start the meta version-check polling scheduler: run once immediately, then at the
/// specified interval. Rebuilds meta files with missing or mismatched versions,
/// skipping active recordings.
pub async fn schedule_meta_version_check(
    output_dir: std::path::PathBuf,
    merge_format: String,
    interval_secs: u64,
) {
    // Run once immediately
    {
        let dir = output_dir.clone();
        let fmt = merge_format.clone();
        tokio::task::spawn_blocking(move || {
            ensure_meta_files(&dir, &fmt);
        })
        .await
        .ok();
    }

    // Then run at the specified interval
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        let dir = output_dir.clone();
        let fmt = merge_format.clone();
        tokio::task::spawn_blocking(move || {
            ensure_meta_files(&dir, &fmt);
        })
        .await
        .ok();
    }
}
