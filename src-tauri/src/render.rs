use crate::ffmpeg::{resolve_ffmpeg, resolve_ffprobe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filters {
    brightness: f64,
    contrast: f64,
    saturation: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextOverlay {
    text: String,
    x: f64,
    y: f64,
    font_size: f64,
    color: String,
    #[allow(dead_code)]
    bold: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Clip {
    id: String,
    media_id: String,
    #[allow(dead_code)]
    name: String,
    source_start: f64,
    source_end: f64,
    timeline_start: f64,
    volume: f64,
    filters: Filters,
    text_overlay: Option<TextOverlay>,
    fade_in: f64,
    fade_out: f64,
    is_image: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
    id: String,
    #[serde(rename = "type")]
    track_type: String,
    #[allow(dead_code)]
    name: String,
    clips: Vec<Clip>,
    muted: bool,
    hidden: bool,
    locked: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaAsset {
    id: String,
    path: String,
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "type")]
    media_type: String,
    #[allow(dead_code)]
    duration: f64,
    #[allow(dead_code)]
    width: Option<u32>,
    #[allow(dead_code)]
    height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[allow(dead_code)]
    name: String,
    width: u32,
    height: u32,
    fps: u32,
    tracks: Vec<Track>,
    assets: HashMap<String, MediaAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    output_path: String,
    format: String,
    video_codec: String,
    audio_codec: String,
    crf: u32,
    preset: String,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderProgress {
    progress: f64,
    stage: String,
    done: bool,
    error: Option<String>,
}

fn has_audio_stream(app: &AppHandle, path: &str) -> bool {
    let ffprobe = match resolve_ffprobe(app) {
        Some(p) => p,
        None => return true, // assume yes if we can't check
    };
    let out = match Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
            path,
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return true,
    };
    let s = String::from_utf8_lossy(&out.stdout);
    s.contains("audio")
}

fn escape_drawtext(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('%', "\\%")
}

fn project_duration(p: &Project) -> f64 {
    let mut max = 0.0_f64;
    for t in &p.tracks {
        for c in &t.clips {
            let end = c.timeline_start + (c.source_end - c.source_start);
            if end > max {
                max = end;
            }
        }
    }
    max
}

#[tauri::command]
pub fn render_project(
    app: AppHandle,
    project: Project,
    settings: ExportSettings,
) -> Result<(), String> {
    let total = project_duration(&project);
    if total <= 0.0 {
        return Err("Timeline is empty".to_string());
    }
    if settings.output_path.is_empty() {
        return Err("No output path".to_string());
    }

    let ffmpeg = resolve_ffmpeg(&app).ok_or_else(|| "FFmpeg not found".to_string())?;

    // Emit starting
    let _ = app.emit(
        "kve://render-progress",
        RenderProgress {
            progress: 0.0,
            stage: "Preparing".to_string(),
            done: false,
            error: None,
        },
    );

    // ---- Collect inputs ----
    // Each entry: (global_input_index, clip, asset_path, is_image, has_audio)
    struct Input {
        idx: usize,
        clip: Clip,
        path: String,
        is_image: bool,
        has_audio: bool,
    }
    let mut inputs: Vec<Input> = Vec::new();
    let mut idx = 0usize;
    for track in &project.tracks {
        if track.hidden && track.track_type == "video" {
            // still consume indices for audio? hidden video track: skip its clips entirely from video,
            // but audio of a hidden video track should still play? Convention: hidden = no video, audio still plays.
            // Keep clips as inputs (audio) but don't render video.
        }
        for clip in &track.clips {
            let asset = match project.assets.get(&clip.media_id) {
                Some(a) => a,
                None => continue,
            };
            let is_image = asset.media_type == "image" || clip.is_image == Some(true);
            let has_audio = if is_image {
                false
            } else if asset.media_type == "audio" {
                true
            } else {
                has_audio_stream(&app, &asset.path)
            };
            inputs.push(Input {
                idx,
                clip: clip.clone(),
                path: asset.path.clone(),
                is_image,
                has_audio,
            });
            idx += 1;
        }
    }

    // Determine whether we have any video or audio to render
    let have_video = project.tracks.iter().any(|t| {
        t.track_type == "video"
            && !t.hidden
            && t.clips.iter().any(|c| {
                project
                    .assets
                    .get(&c.media_id)
                    .map(|a| a.media_type != "audio")
                    .unwrap_or(false)
            })
    });
    let have_audio = inputs
        .iter()
        .any(|i| i.has_audio && !track_muted_for_clip(&project, &i.clip.id));

    if !have_video && !have_audio {
        return Err("Nothing to export".to_string());
    }

    // ---- Build ffmpeg command ----
    let out_w = settings.width.unwrap_or(project.width);
    let out_h = settings.height.unwrap_or(project.height);
    let out_fps = settings.fps.unwrap_or(project.fps).max(1);

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y");

    // Inputs
    for input in &inputs {
        let dur = (input.clip.source_end - input.clip.source_start).max(0.05);
        if input.is_image {
            cmd.args(["-loop", "1", "-t", &fmt_dur(dur), "-i", &input.path]);
        } else {
            cmd.args([
                "-ss",
                &fmt_dur(input.clip.source_start),
                "-t",
                &fmt_dur(dur),
                "-i",
                &input.path,
            ]);
        }
    }

    // ---- Build filter_complex ----
    let mut graph = String::new();

    // Video base
    if have_video {
        graph.push_str(&format!(
            "color=c=black:s={w}x{h}:d={d}:r={r}[base];",
            w = out_w,
            h = out_h,
            d = fmt_dur(total),
            r = out_fps
        ));
    }

    // Per-clip video filters + overlay chain
    let mut last_video = "base".to_string();
    let mut overlay_count = 0usize;

    // video inputs in track order (bottom track first → drawn first → appears below)
    let mut v_label = 0usize;
    for track in &project.tracks {
        if track.track_type != "video" || track.hidden {
            continue;
        }
        for clip in &track.clips {
            let asset = match project.assets.get(&clip.media_id) {
                Some(a) => a,
                None => continue,
            };
            if asset.media_type == "audio" {
                continue;
            }
            // find the input index for this clip
            let input = match inputs.iter().find(|i| i.clip.id == clip.id) {
                Some(i) => i,
                None => continue,
            };
            let dur = (clip.source_end - clip.source_start).max(0.05);

            // video filter chain
            let mut chain = format!("[{idx}:v]", idx = input.idx);
            chain.push_str(&format!(
                "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,",
                w = out_w,
                h = out_h
            ));
            // eq
            chain.push_str(&format!(
                "eq=brightness={b}:contrast={c}:saturation={s}",
                b = clip.filters.brightness,
                c = 1.0 + clip.filters.contrast,
                s = 1.0 + clip.filters.saturation
            ));
            // fades
            if clip.fade_in > 0.0 {
                chain.push_str(&format!(",fade=t=in:st=0:d={}", fmt_dur(clip.fade_in)));
            }
            if clip.fade_out > 0.0 {
                let st = (dur - clip.fade_out).max(0.0);
                chain.push_str(&format!(
                    ",fade=t=out:st={}:d={}",
                    fmt_dur(st),
                    fmt_dur(clip.fade_out)
                ));
            }
            // text overlay
            if let Some(txt) = &clip.text_overlay {
                if !txt.text.trim().is_empty() {
                    chain.push_str(&format!(
                        ",drawtext=text={txt}:x=(main_w*{px})-tw/2:y=(main_h*{py})-th/2:fontsize={fs}:fontcolor={color}:borderw=2:bordercolor=black@0.6:line_spacing=4",
                        txt = escape_drawtext(&txt.text),
                        px = txt.x,
                        py = txt.y,
                        fs = (out_h as f64 * txt.font_size).round() as u32,
                        color = txt.color
                    ));
                }
            }
            let label = format!("vv{}", v_label);
            v_label += 1;
            chain.push_str(&format!("[{}];", label));

            graph.push_str(&chain);

            // overlay onto current base
            let next = format!("vo{}", overlay_count);
            overlay_count += 1;
            graph.push_str(&format!(
                "[{prev}][{label}]overlay=x=0:y=0:enable='between(t,{s},{e})'[{next}];",
                prev = last_video,
                label = label,
                s = fmt_dur(clip.timeline_start),
                e = fmt_dur(clip.timeline_start + dur),
                next = next
            ));
            last_video = next;
        }
    }

    // Audio streams
    let mut audio_labels: Vec<String> = Vec::new();
    let mut a_label = 0usize;
    for input in &inputs {
        if !input.has_audio {
            continue;
        }
        if track_muted_for_clip(&project, &input.clip.id) {
            continue;
        }
        let dur = (input.clip.source_end - input.clip.source_start).max(0.05);
        let mut chain = format!("[{idx}:a]", idx = input.idx);
        chain.push_str("aresample=async=1");
        if (input.clip.volume - 1.0).abs() > 0.001 {
            chain.push_str(&format!(",volume={}", input.clip.volume));
        }
        if input.clip.fade_in > 0.0 {
            chain.push_str(&format!(",afade=t=in:st=0:d={}", fmt_dur(input.clip.fade_in)));
        }
        if input.clip.fade_out > 0.0 {
            let st = (dur - input.clip.fade_out).max(0.0);
            chain.push_str(&format!(
                ",afade=t=out:st={}:d={}",
                fmt_dur(st),
                fmt_dur(input.clip.fade_out)
            ));
        }
        // delay to position on timeline (ms)
        let delay_ms = (input.clip.timeline_start * 1000.0).round() as u64;
        chain.push_str(&format!(",adelay={ms}:{ms}", ms = delay_ms));
        let label = format!("aa{}", a_label);
        a_label += 1;
        chain.push_str(&format!("[{}];", label));
        graph.push_str(&chain);
        audio_labels.push(label);
    }

    // Mix audio
    if have_audio && !audio_labels.is_empty() {
        if audio_labels.len() == 1 {
            graph.push_str(&format!("[{}]anull[aout];", audio_labels[0]));
        } else {
            graph.push_str(&format!(
                "[{}]amix=inputs={n}:duration=longest:normalize=0[aout];",
                audio_labels.join(""),
                n = audio_labels.len()
            ));
        }
    }

    // Final video label
    let video_out = if have_video {
        if overlay_count == 0 {
            // no video clips overlaid (shouldn't happen if have_video) → use base
            graph.push_str("[base]null[vout];");
            "vout".to_string()
        } else {
            // re-label last overlay output to vout
            // last_video is "voN"; map it directly
            last_video.clone()
        }
    } else {
        String::new()
    };

    // Remove trailing semicolon
    while graph.ends_with(';') {
        graph.pop();
    }

    cmd.arg("-filter_complex").arg(&graph);

    // Mapping
    if have_video {
        cmd.arg("-map").arg(format!("[{}]", video_out));
    }
    if have_audio {
        cmd.arg("-map").arg("[aout]");
    }

    // Output encoding options
    if have_video {
        cmd.args(["-c:v", &settings.video_codec]);
        if settings.video_codec.starts_with("libx26") {
            cmd.args(["-crf", &settings.crf.to_string()]);
            cmd.args(["-preset", &settings.preset]);
        }
        cmd.args(["-pix_fmt", "yuv420p"]);
        cmd.args(["-r", &out_fps.to_string()]);
    }
    if have_audio {
        cmd.args(["-c:a", &settings.audio_codec]);
        cmd.args(["-b:a", "192k"]);
    }
    cmd.args(["-movflags", "+faststart"]);
    cmd.arg(&settings.output_path);

    log::info!("ffmpeg args: {:?}", cmd);
    let mut child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start ffmpeg: {}", e))?;

    let stderr = child.stderr.take().ok_or("no stderr")?;
    let app_clone = app.clone();
    let total_dur = total;
    let stage_label = "Rendering".to_string();

    // Spawn a thread to read progress from stderr
    let _progress_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if let Some(time) = parse_ffmpeg_time(&line) {
                let prog = (time / total_dur).clamp(0.0, 0.999);
                let _ = app_clone.emit(
                    "kve://render-progress",
                    RenderProgress {
                        progress: prog,
                        stage: stage_label.clone(),
                        done: false,
                        error: None,
                    },
                );
            }
            // small throttle
            std::thread::sleep(Duration::from_millis(20));
        }
    });

    let status = child.wait().map_err(|e| format!("ffmpeg wait: {}", e))?;

    if status.success() {
        let _ = app.emit(
            "kve://render-progress",
            RenderProgress {
                progress: 1.0,
                stage: "Done".to_string(),
                done: true,
                error: None,
            },
        );
        Ok(())
    } else {
        let msg = format!("ffmpeg exited with code {:?}", status.code());
        let _ = app.emit(
            "kve://render-progress",
            RenderProgress {
                progress: 0.0,
                stage: "Error".to_string(),
                done: false,
                error: Some(msg.clone()),
            },
        );
        Err(msg)
    }
}

fn track_muted_for_clip(project: &Project, clip_id: &str) -> bool {
    for track in &project.tracks {
        if track.clips.iter().any(|c| c.id == clip_id) {
            return track.muted;
        }
    }
    false
}

fn parse_ffmpeg_time(line: &str) -> Option<f64> {
    // lines like: "frame=  123 fps= ... time=00:00:05.12 bitrate= ..."
    let mut t = line.split("time=");
    let _ = t.next();
    let rest = t.next()?;
    let val = rest.split_whitespace().next()?;
    // HH:MM:SS.ms
    let parts: Vec<&str> = val.split(':').collect();
    match parts.len() {
        3 => {
            let h: f64 = parts[0].parse().ok()?;
            let m: f64 = parts[1].parse().ok()?;
            let s: f64 = parts[2].parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        1 => parts[0].parse().ok(),
        _ => None,
    }
}

fn fmt_dur(d: f64) -> String {
    format!("{:.3}", d.max(0.0))
}
