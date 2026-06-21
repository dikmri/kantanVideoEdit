use crate::ffmpeg::resolve_ffprobe;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub duration: f64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "type")]
    pub media_type: String,
    pub codec: Option<String>,
    pub fps: Option<f64>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

fn parse_frame_rate(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].parse().ok()?;
        let den: f64 = parts[1].parse().ok()?;
        if den > 0.0 {
            return Some(num / den);
        }
    }
    None
}

fn probe_internal(app: &AppHandle, path: &str) -> ProbeResult {
    let default = ProbeResult {
        duration: 0.0,
        width: None,
        height: None,
        media_type: "video".to_string(),
        codec: None,
        fps: None,
        thumbnail: None,
    };

    let ffprobe = match resolve_ffprobe(app) {
        Some(p) => p,
        None => return default,
    };

    let output = match Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return default,
    };

    let parsed: FfprobeOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(_) => return default,
    };

    let mut video_stream = None;
    let mut audio_stream = None;
    for s in &parsed.streams {
        match s.codec_type.as_deref() {
            Some("video") if video_stream.is_none() => video_stream = Some(s),
            Some("audio") if audio_stream.is_none() => audio_stream = Some(s),
            _ => {}
        }
    }

    let duration = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| {
            video_stream
                .and_then(|s| s.duration.as_deref())
                .and_then(|d| d.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    // Determine media type
    let is_image = video_stream.map(|s| {
        matches!(
            s.codec_name.as_deref(),
            Some("mjpeg" | "png" | "bmp" | "webp" | "gif" | "tiff" | "jpeg" | "jp2")
        )
    }) == Some(true);

    let (media_type, fps) = if let Some(vs) = video_stream {
        let t = if is_image {
            "image".to_string()
        } else {
            "video".to_string()
        };
        let f = vs
            .avg_frame_rate
            .as_deref()
            .and_then(parse_frame_rate)
            .or_else(|| vs.r_frame_rate.as_deref().and_then(parse_frame_rate));
        (t, f)
    } else if audio_stream.is_some() {
        ("audio".to_string(), None)
    } else {
        ("video".to_string(), None)
    };

    let width = video_stream.and_then(|s| s.width);
    let height = video_stream.and_then(|s| s.height);
    let codec = video_stream
        .and_then(|s| s.codec_name.clone())
        .or_else(|| audio_stream.and_then(|s| s.codec_name.clone()));

    ProbeResult {
        duration,
        width,
        height,
        media_type,
        codec,
        fps,
        thumbnail: None,
    }
}

fn make_thumbnail(app: &AppHandle, path: &str, duration: f64) -> Option<String> {
    let ffmpeg = crate::ffmpeg::resolve_ffmpeg(app)?;
    let tmp = std::env::temp_dir();
    let safe = path.replace(['/', '\\', ':', ' ', '.'], "_");
    let thumb_name = format!("kve_thumb_{}.jpg", safe);
    let thumb_path = tmp.join(thumb_name);

    // seek to ~10% or 1s
    let seek = if duration > 10.0 { duration * 0.1 } else { 0.0 };

    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-ss",
            &seek.to_string(),
            "-i",
            path,
            "-frames:v",
            "1",
            "-vf",
            "scale=320:-2",
            "-q:v",
            "4",
            thumb_path.to_string_lossy().as_ref(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;

    if status.success() && thumb_path.exists() {
        Some(thumb_path.to_string_lossy().to_string())
    } else {
        None
    }
}

#[tauri::command]
pub fn probe_media(app: AppHandle, path: String) -> ProbeResult {
    let mut result = probe_internal(&app, &path);
    if result.media_type == "video" || result.media_type == "image" {
        if let Some(thumb) = make_thumbnail(&app, &path, result.duration) {
            result.thumbnail = Some(thumb);
        }
    }
    result
}

#[tauri::command]
pub fn probe_media_batch(app: AppHandle, paths: Vec<String>) -> Vec<ProbeResult> {
    paths
        .iter()
        .map(|p| {
            let mut r = probe_internal(&app, p);
            if r.media_type == "video" || r.media_type == "image" {
                if let Some(thumb) = make_thumbnail(&app, p, r.duration) {
                    r.thumbnail = Some(thumb);
                }
            }
            r
        })
        .collect()
}

// silence unused import warning on some targets
#[allow(dead_code)]
fn _unused(_: PathBuf) {}
