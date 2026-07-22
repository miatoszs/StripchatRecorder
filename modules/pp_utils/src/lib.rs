//! Post-processing Utility Library
//!
//! ，：
//! -
//! -  ffprobe
//! - 、
//! -
//! -
//! -
//!
//! Provides shared utility functions for all post-processing modules, including:
//! - Reading module parameters from environment variables
//! - Getting video duration via ffprobe
//! - Formatting duration, file size, and transfer speed
//! - Parsing streamer name and timestamp from recording filenames
//! - Finding cover images for videos
//! - Emitting progress information to stdout

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// ， `PP_PARAM_{KEY}` 。
/// Read a string module parameter passed via environment variable `PP_PARAM_{KEY}`.
///
/// Parameters
/// - `key`: （）/ Parameter key (case-insensitive)
/// Default value when env var is not set
pub fn param(key: &str, fallback: &str) -> String {
    env::var(format!("PP_PARAM_{}", key.to_uppercase())).unwrap_or_else(|_| fallback.to_string())
}

/// u32 ，。
/// Read a u32 module parameter, returns fallback on parse failure.
pub fn param_u32(key: &str, fallback: u32) -> u32 {
    param(key, &fallback.to_string())
        .parse()
        .unwrap_or(fallback)
}

/// f64 ，。
/// Read an f64 module parameter, returns fallback on parse failure.
pub fn param_f64(key: &str, fallback: f64) -> f64 {
    param(key, &fallback.to_string())
        .parse()
        .unwrap_or(fallback)
}

/// ，"true"/"1"/"yes"（） true。
/// Read a boolean module parameter; "true"/"1"/"yes" (case-insensitive) are treated as true.
pub fn param_bool(key: &str, fallback: bool) -> bool {
    matches!(
        param(key, if fallback { "true" } else { "false" })
            .to_lowercase()
            .as_str(),
        "true" | "1" | "yes"
    )
}

/// ffprobe （）。
/// Get the duration of a video file in seconds using ffprobe.
///
/// Returns
/// （），ffprobe  `None`。
/// Video duration in seconds, or `None` if ffprobe is unavailable or parsing fails.
pub fn video_duration(input: &Path) -> Option<f64> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}

/// `HH:MM:SS` 。
/// Format seconds as a duration string in `HH:MM:SS` format.
pub fn format_duration(secs: f64) -> String {
    let s = secs as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// （ "1.23 GB"）。
/// Format bytes as a human-readable size string (e.g. "1.23 GB").
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut val = bytes as f64;
    let mut i = 0;
    while val >= 1024.0 && i < UNITS.len() - 1 {
        val /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", val, UNITS[i])
}

/// （ "↑ 1.5 MB/s"）。
/// Format bytes per second as an upload speed string (e.g. "↑ 1.5 MB/s").
pub fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1024.0 * 1024.0 {
        format!("↑ {:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 {
        format!("↑ {:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("↑ {:.0} B/s", bytes_per_sec)
    }
}

/// stem 。
/// `{model_name}_{YYYYMMDD}_{HHmmss}`。
///
/// Parse the model name and recording timestamp from a recording filename stem.
/// Filename format: `{model_name}_{YYYYMMDD}_{HHmmss}`
///
/// Returns
/// `(model_name, timestamp_str)` ， timestamp 。
/// Tuple of `(model_name, timestamp_str)`, timestamp is empty string on parse failure.
pub fn parse_stem(stem: &str) -> (String, String) {
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() >= 3 {
        let date = parts[parts.len() - 2];
        let time = parts[parts.len() - 1];
        // （8）（6）
        // Validate date (8 digits) and time (6 digits) format
        if date.len() == 8
            && date.chars().all(|c| c.is_ascii_digit())
            && time.len() == 6
            && time.chars().all(|c| c.is_ascii_digit())
        {
            let model = parts[..parts.len() - 2].join("_");
            let ts = format!(
                "{}-{}-{} {}:{}:{}",
                &date[..4],
                &date[4..6],
                &date[6..8],
                &time[..2],
                &time[2..4],
                &time[4..6]
            );
            return (model, ts);
        }
    }
    (stem.to_string(), String::new())
}

/// （ jpg/jpeg/webp/png）。
/// Find the cover image for a video in the same directory (supports jpg/jpeg/webp/png).
///
/// Returns
/// ， `None`。
/// Cover image path, or `None` if not found.
pub fn find_cover(video: &Path) -> Option<PathBuf> {
    let stem = video.file_stem()?.to_str()?;
    let dir = video.parent()?;
    for ext in &["jpg", "jpeg", "webp", "png"] {
        let p = dir.join(format!("{}.{}", stem, ext));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// （ `PP_EXE_DIR`  `tmp` ）。
/// `PP_MAX_TMP_MB` ，。
///
/// Get the temporary file directory (prefers a `tmp` subdirectory under `PP_EXE_DIR` env var).
/// If `PP_MAX_TMP_MB` is set, automatically prunes old files that exceed the size limit before returning.
pub fn tmp_dir() -> PathBuf {
    let base = env::var("PP_EXE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    let tmp = base.join("tmp");
    std::fs::create_dir_all(&tmp).ok();

    // ，
    // If a max size limit is set, prune old files that exceed it
    if let Ok(max_mb_str) = env::var("PP_MAX_TMP_MB")
        && let Ok(max_mb) = max_mb_str.trim().parse::<u64>()
        && max_mb > 0
    {
        cleanup_tmp_dir(&tmp, max_mb);
    }

    tmp
}

/// tmp ，， `max_mb`。
/// ，（）。
///
/// Prune the tmp directory by deleting files from oldest to newest until the total
/// directory size is below `max_mb`. Only direct child files are deleted; subdirectories
/// are left for modules to manage themselves.
pub fn cleanup_tmp_dir(tmp: &Path, max_mb: u64) {
    let max_bytes = max_mb * 1024 * 1024;

    // Collect all direct child files with metadata
    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = std::fs::read_dir(tmp)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let meta = std::fs::metadata(&path).ok()?;
            let size = meta.len();
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((path, size, modified))
        })
        .collect();

    // Calculate current total size
    let total: u64 = entries.iter().map(|(_, s, _)| s).sum();
    if total <= max_bytes {
        return;
    }

    // （）/ Sort by modification time ascending (oldest first)
    entries.sort_by_key(|(_, _, t)| *t);

    let mut remaining = total;
    for (path, size, _) in &entries {
        if remaining <= max_bytes {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            remaining = remaining.saturating_sub(*size);
        }
    }
}

/// ffprobe 。
/// Get image width and height using ffprobe.
///
/// Returns
/// `(width, height)`， `None`。
/// `(width, height)`, or `None` on failure.
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::process::Command;
    let out = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut parts = s.trim().splitn(2, ',');
    let w: u32 = parts.next()?.trim().parse().ok()?;
    let h: u32 = parts.next()?.trim().parse().ok()?;
    Some((w, h))
}

/// ffprobe 、。
/// Get video duration, width, and height using ffprobe.
///
/// Returns
/// `(duration_secs, width, height)`， `None`。
/// `(duration_secs, width, height)`, or `None` on failure.
pub fn video_meta(input: &Path) -> Option<(f64, i32, i32)> {
    let out = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "format=duration:stream=width,height",
            "-of", "csv=p=0",
        ])
        .arg(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut lines = s.lines().filter(|l| !l.trim().is_empty());
    let dims_line = lines.next()?;
    let dur_line = lines.next()?;
    let mut dims = dims_line.splitn(2, ',');
    let w: i32 = dims.next()?.trim().parse().ok()?;
    let h: i32 = dims.next()?.trim().parse().ok()?;
    let dur: f64 = dur_line.trim().parse().ok()?;
    Some((dur, w, h))
}

/// （10000 = 100.00%）。
/// Progress reporting scale base (10000 = 100.00%).
pub const PROGRESS_SCALE: u32 = 10_000;

/// （：`PROGRESS:{scaled}/{PROGRESS_SCALE}`）。
/// Emit progress to stdout (format: `PROGRESS:{scaled}/{PROGRESS_SCALE}`).
///
/// Parameters
/// Amount of work done
/// Total amount of work
pub fn emit_progress(done: u32, total: u32) {
    let scaled = if total == 0 {
        0
    } else {
        ((done as u64) * (PROGRESS_SCALE as u64) / (total as u64)).min(PROGRESS_SCALE as u64) as u32
    };
    println!("PROGRESS:{}/{}", scaled, PROGRESS_SCALE);
}

/// ，（）。
/// Emit progress by step count, suitable for tasks with a fixed number of steps (rounded to nearest step).
///
/// Parameters
/// - `step`: （0-based）/ Current step index (0-based)
/// Total number of steps
pub fn emit_progress_step(step: u32, total_steps: u32) {
    let scaled = if total_steps == 0 {
        0
    } else {
        (((step as u64) * (PROGRESS_SCALE as u64) + ((total_steps as u64) / 2))
            / (total_steps as u64))
            .min(PROGRESS_SCALE as u64) as u32
    };
    println!("PROGRESS:{}/{}", scaled, PROGRESS_SCALE);
}
