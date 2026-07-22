//! Contact Sheet Post-processing Module
//!
//! ，，
//! （contact sheet）。
//!
//! Extracts frames from the video at specified intervals, overlays timestamp watermarks,
//! then tiles all frames into a grid preview image (contact sheet) saved in the video's directory.
//!
//! Protocol
//! Output module metadata as JSON
//! Input video file path via env var
//! -  `OUTPUT:{path}`: （contact sheet ）/ Output video path
//! -  `SKIP:{reason}`: （contact sheet ）/ Skip reason (contact sheet already exists)

use pp_utils::{emit_progress, param, param_u32, video_duration};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// JSON， `--describe` 。
/// Module metadata JSON, output via `--describe` argument.
const DESCRIBE: &str = r#"{
    "id": "contact_sheet",
    "name": "Contact Sheet 0.2.0",
    "description": "每隔指定秒数截帧，拼合成一张带时间戳的预览图保存到视频同目录",
    "params": [
        {
        "key": "interval",
        "label": "截帧间隔（秒）",
        "type": "number",
        "default": 30
        },
        {
        "key": "thumb_width",
        "label": "单帧宽度（px）",
        "type": "number",
        "default": 320
        },
        {
        "key": "format",
        "label": "图片格式",
        "type": "select",
        "default": "webp",
        "options": ["webp", "jpg", "png"]
        },
        {
        "key": "quality",
        "label": "图片质量（1-100，jpg/webp 有效）",
        "type": "number",
        "default": 100
        },
        {
        "key": "cols",
        "label": "列数（0=自动）",
        "type": "number",
        "default": 0
        },
        {
        "key": "rows",
        "label": "行数（0=自动）",
        "type": "number",
        "default": 0
        },
        {
        "key": "fontfile",
        "label": "字体文件路径（留空自动检测）",
        "type": "string",
        "default": ""
        },
        {
        "key": "fontsize",
        "label": "时间戳字号",
        "type": "number",
        "default": 18
        }
    ]
}"#;

/// 。
///
/// Calculate the optimal number of columns for the grid (to make it roughly square).
/// Uses the user-specified value if provided.
///
/// Parameters
/// Total frame count
/// - `forced_cols`: （0 = ）/ User-specified columns (0 = auto)
fn compute_cols(frame_count: u32, forced_cols: u32) -> u32 {
    if forced_cols > 0 {
        return forced_cols;
    }
    // Use sqrt * 1.33 to make grid slightly wider than tall
    (((frame_count as f64).sqrt() * 1.33).ceil() as u32).max(1)
}

/// 。
/// Windows、macOS  Linux。
///
/// Find an available font file in common system paths.
/// Supports Windows, macOS, and Linux.
///
/// Returns
/// ffmpeg drawtext ， `None`。
/// Font path string usable by ffmpeg drawtext filter, or `None` if not found.
fn find_font() -> Option<String> {
    let candidates: &[&str] = &[
        // Windows fonts
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\consola.ttf",
        // macOS fonts
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial.ttf",
        // Linux fonts
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| ffmpeg_escape_path(p))
}

/// ffmpeg drawtext 。
/// Windows ，。
///
/// Convert a file path to a format accepted by ffmpeg's drawtext filter.
/// On Windows, backslashes are converted to forward slashes and the colon after
/// the drive letter is escaped.
fn ffmpeg_escape_path(path: &str) -> String {
    let fwd = path.replace('\\', "/");
    // Windows （ C:/...） \:
    // Windows drive paths (e.g. C:/...) need the colon escaped as \:
    if fwd.len() >= 2 && fwd.as_bytes()[1] == b':' {
        format!("{}\\:{}", &fwd[..1], &fwd[2..])
    } else {
        fwd
    }
}

/// ： ->  -> 。
/// Main module logic: extract frames -> overlay timestamps -> tile into grid.
fn run() -> Result<(), String> {
    // Read input file path
    let input_str = env::var("PP_INPUT").map_err(|_| "PP_INPUT not set".to_string())?;
    let input = PathBuf::from(&input_str);

    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()));
    }

    // Read module parameters
    let interval = param_u32("interval", 30).max(1);
    let thumb_width = param_u32("thumb_width", 320).max(16);
    let forced_cols = param_u32("cols", 0);
    let forced_rows = param_u32("rows", 0);
    let fontsize = param_u32("fontsize", 18).max(8);
    let tile_pad = 4u32;
    let quality = param_u32("quality", 100).clamp(1, 100);
    let format = param("format", "webp");
    let fontfile_param = param("fontfile", "");

    // （）/ Determine font file path (user-specified takes priority)
    let fontfile = if !fontfile_param.is_empty() {
        Some(ffmpeg_escape_path(&fontfile_param))
    } else {
        find_font()
    };

    if fontfile.is_none() {
        eprintln!("Warning: no font file found, timestamp overlay will be skipped");
    }

    // ：，，
    // Output path: same directory as video, same name, image format extension
    let output_path = input
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.{}",
            input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("contact_sheet"),
            format
        ));

    // Skip if contact sheet already exists
    if output_path.exists() {
        println!(
            "SKIP: contact sheet already exists: {}",
            output_path.display()
        );
        println!("OUTPUT:{}", output_path.display());
        return Ok(());
    }

    // Get video duration to calculate frame count
    let duration = video_duration(&input)
        .ok_or_else(|| "无法获取视频时长，请确认 ffprobe 已安装".to_string())?;

    let frame_count = ((duration / interval as f64).floor() as u32).max(1);
    let cols = compute_cols(frame_count, forced_cols);
    let rows = if forced_rows > 0 {
        forced_rows
    } else {
        // Ceiling division to fit all frames in the grid
        frame_count.div_ceil(cols)
    };

    // Create temp directory for extracted frames
    let tmp_dir = std::env::temp_dir().join(format!(
        "contact_sheet_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
    // ，
    // Define cleanup closure to ensure temp dir is always removed
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    };

    emit_progress(0, frame_count);

    // （）/ Build timestamp watermark filter (if font available)
    let drawtext_filter = if let Some(ref font) = fontfile {
        format!(
            ",drawtext=fontfile='{font}'\
             :text='%{{pts\\:hms}}'\
             :x=w-tw-8:y=h-th-8:fontsize={fs}:fontcolor=white\
             :box=1:boxcolor=black@0.6:boxborderw=3",
            font = font,
            fs = fontsize,
        )
    } else {
        String::new()
    };

    // ffmpeg ： +  +
    // Build ffmpeg video filter: select frames by timestamp + scale + timestamp overlay
    //
    // select='not(mod(t,{interval}))' ，pts ，
    // drawtext  %{pts\:hms} 。
    // fps ， pts 。
    //
    // Use select='not(mod(t,{interval}))' to pick frames by original timestamp,
    // keeping pts intact throughout so drawtext's %{pts\:hms} shows the correct
    // position in the video. Avoids the fps filter which resets pts to 0.
    // select ：， interval
    // isnan(prev_selected_t) ；gte(t-prev_selected_t, interval)
    // pts ，drawtext 。
    //
    // select filter: pick the first frame, then any frame at least `interval` seconds
    // after the previously selected one. pts stays intact for correct drawtext timestamps.
    let vf = format!(
        "select='isnan(prev_selected_t)+gte(t-prev_selected_t\\,{interval})',scale={w}:-1{dt}",
        interval = interval,
        w = thumb_width,
        dt = drawtext_filter
    );
    let frame_pattern = tmp_dir.join("frame_%06d.png");

    // ： ffmpeg
    // Step 1: Extract frames with ffmpeg and report progress in real-time
    let mut child = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(&input)
        .args(["-vf", &vf])
        .args(["-vsync", "vfr"])
        .args(["-frames:v", &frame_count.to_string()])
        .arg(&frame_pattern)
        .args(["-progress", "pipe:1"])
        .args(["-loglevel", "error"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            cleanup();
            format!("Failed to spawn ffmpeg (extract): {}", e)
        })?;

    {
        // out_time_us （select  frame= ，）
        // Use out_time_us to estimate progress (frame= from select filter counts input frames)
        use std::io::{BufRead, BufReader};
        let stdout = child.stdout.take().expect("stdout piped");
        let reader = BufReader::new(stdout);
        let total_us = (duration * 1_000_000.0) as u64;
        let mut last_reported = 0u32;
        for line in reader.lines().map_while(Result::ok) {
            if let Some(val) = line.strip_prefix("out_time_us=")
                && let Ok(us) = val.trim().parse::<u64>()
            {
                let progress = if total_us > 0 {
                    ((us as f64 / total_us as f64) * frame_count as f64) as u32
                } else {
                    0
                };
                let clamped = progress.min(frame_count);
                if clamped != last_reported {
                    emit_progress(clamped, frame_count);
                    last_reported = clamped;
                }
            }
        }
    }

    let status = child.wait().map_err(|e| {
        cleanup();
        format!("ffmpeg extract wait failed: {}", e)
    })?;

    if !status.success() {
        let stderr_msg = child
            .stderr
            .take()
            .and_then(|mut s| {
                use std::io::Read;
                let mut buf = String::new();
                s.read_to_string(&mut buf).ok()?;
                Some(buf)
            })
            .unwrap_or_default();
        cleanup();
        return Err(format!("ffmpeg extract failed:\n{}", stderr_msg.trim()));
    }

    // Verify actual number of extracted frames
    let extracted = (1..=frame_count)
        .filter(|i| tmp_dir.join(format!("frame_{:06}.png", i)).exists())
        .count() as u32;

    if extracted == 0 {
        cleanup();
        return Err(
            "No frames extracted — check the video file and ffmpeg installation".to_string(),
        );
    }

    emit_progress(frame_count, frame_count);

    // Generate ffmpeg concat file list
    let filelist_path = tmp_dir.join("frames.txt");
    let mut list = String::new();
    for i in 1..=frame_count {
        let p = tmp_dir.join(format!("frame_{:06}.png", i));
        if p.exists() {
            list.push_str(&format!(
                "file '{}'\n",
                p.to_string_lossy().replace('\\', "/")
            ));
        }
    }
    std::fs::write(&filelist_path, &list).map_err(|e| {
        cleanup();
        format!("Failed to write filelist: {}", e)
    })?;

    // ： ffmpeg tile
    // Step 2: Use ffmpeg tile filter to combine frames into a grid image
    let tile_filter = format!("tile={}x{}:padding={}", cols, rows, tile_pad);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&filelist_path)
        .args(["-vf", &tile_filter, "-frames:v", "1"]);

    // Set quality parameters based on output format
    match format.as_str() {
        "jpg" => {
            cmd.args(["-q:v", "3"]);
        }
        "webp" => {
            cmd.args(["-quality", &quality.to_string()]);
        }
        _ => {}
    }

    cmd.arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let out = cmd.output().map_err(|e| {
        cleanup();
        format!("Failed to spawn ffmpeg (tile): {}", e)
    })?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        cleanup();
        return Err(format!("ffmpeg tile failed:\n{}", stderr.trim()));
    }

    // Clean up temporary frame files
    cleanup();
    // contact sheet ， meta  module_outputs
    // Output the contact sheet image path so the backend can store it in meta's module_outputs
    println!("OUTPUT:{}", output_path.display());
    Ok(())
}

/// ： `--describe` 。
/// Entry point: handle `--describe` argument or execute main logic.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--describe") {
        print!("{}", DESCRIBE);
        return;
    }
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
