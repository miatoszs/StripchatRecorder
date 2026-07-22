//! Filter Short Videos Post-processing Module
//!
//! ，。
//! dry_run （，）。
//!
//! Checks the duration of the input video and deletes it if below the specified threshold.
//! Supports dry_run mode (preview only, no actual deletion).
//!
//! Protocol
//! Output module metadata as JSON
//! Input video file path via env var
//! Module parameters via env vars
//! Output path when video passes filter
//! -  `DELETE_INPUT`: （）/ Request host to delete input file (when duration is below threshold)
//! Progress reporting

use pp_utils::{param_bool, param_f64, video_duration, PROGRESS_SCALE};
use std::env;
use std::path::PathBuf;

/// JSON， `--describe` 。
/// Module metadata JSON, output via `--describe` argument.
const DESCRIBE: &str = r#"{
    "id": "filter_short",
    "name": "过滤短视频 0.2.0",
    "description": "删除时长低于指定阈值的视频文件",
    "params": [
        {
        "key": "min_duration",
        "label": "最短时长（秒）",
        "type": "number",
        "default": 60
        },
        {
        "key": "dry_run",
        "label": "仅预览，不实际删除",
        "type": "boolean",
        "default": false
        }
    ]
}"#;

/// ：、、。
/// Main module logic: read parameters, check video duration, decide whether to delete.
fn run() -> Result<(), String> {
    // Read input file path from environment variable
    let input_str = env::var("PP_INPUT").map_err(|_| "PP_INPUT not set".to_string())?;
    let input = PathBuf::from(&input_str);

    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()));
    }

    // Read module parameters
    let min_duration = param_f64("min_duration", 60.0).max(0.0);
    let dry_run = param_bool("dry_run", false);

    // Report initial progress
    println!("PROGRESS:0/{}", PROGRESS_SCALE);

    // Get video duration using ffprobe
    let duration = video_duration(&input)
        .ok_or_else(|| "无法获取视频时长，请确认 ffprobe 已安装".to_string())?;

    // Report completion progress
    println!("PROGRESS:{}/{}", PROGRESS_SCALE, PROGRESS_SCALE);

    if duration < min_duration {
        // Video duration below threshold: request host deletion or preview
        if dry_run {
            eprintln!(
                "DRY_RUN: would delete '{}' (duration {:.1}s < {:.1}s)",
                input.display(),
                duration,
                min_duration
            );
        } else {
            // DELETE_INPUT ，
            // Output DELETE_INPUT protocol line; the host is responsible for deleting the file
            println!("DELETE_INPUT");
            eprintln!(
                "Requesting deletion of '{}' (duration {:.1}s < {:.1}s)",
                input.display(),
                duration,
                min_duration
            );
        }
        // OUTPUT，
        // No OUTPUT when video will be deleted; subsequent pipeline modules will be skipped
    } else {
        // Video duration meets requirement, pass to next module
        println!("OUTPUT:{}", input.display());
    }

    Ok(())
}

/// ： `--describe` 。
/// Entry point: handle `--describe` argument or execute main logic.
fn main() {
    let args: Vec<String> = env::args().collect();
    // Output module description JSON and exit
    if args.get(1).map(|s| s.as_str()) == Some("--describe") {
        print!("{}", DESCRIBE);
        return;
    }
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
