use crate::ffmpeg::{resolve_ffmpeg, resolve_ffprobe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    id: String,
    path: String,
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "type")]
    media_type: String,
    #[allow(dead_code)]
    duration: f64,
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
    #[allow(dead_code)]
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
        None => return true,
    };
    let out = match Command::new(&ffprobe)
        .args([
            "-v", "error", "-select_streams", "a",
            "-show_entries", "stream=codec_type", "-of", "csv=p=0", path,
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return true,
    };
    String::from_utf8_lossy(&out.stdout).contains("audio")
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
            if end > max { max = end; }
        }
    }
    max
}

fn fmt_dur(d: f64) -> String { format!("{:.6}", d.max(0.0)) }

/// A processed clip ready for the concat list.
struct RenderClip {
    clip: Clip,
    asset_path: String,
    is_image: bool,
    has_audio: bool,
}

/// Collect all clips grouped by track, sorted by timeline position.
fn collect_clips(project: &Project, app: &AppHandle) -> (Vec<Vec<RenderClip>>, bool, bool) {
    let mut have_video = false;
    let mut have_audio = false;
    let mut track_clips: Vec<Vec<RenderClip>> = Vec::new();

    for track in &project.tracks {
        let mut clips: Vec<RenderClip> = Vec::new();
        for clip in &track.clips {
            let asset = match project.assets.get(&clip.media_id) {
                Some(a) => a,
                None => continue,
            };
            let is_image = asset.media_type == "image" || clip.is_image == Some(true);
            if track.track_type == "video" && !track.hidden && asset.media_type != "audio" {
                have_video = true;
            }
            let has_audio = if is_image { false } else if asset.media_type == "audio" { have_audio = true; true } else { let ha = has_audio_stream(app, &asset.path); if ha { have_audio = true; } ha };
            if track.track_type == "audio" && !track.muted && has_audio {
                have_audio = true;
            }
            clips.push(RenderClip {
                clip: clip.clone(),
                asset_path: asset.path.clone(),
                is_image,
                has_audio,
            });
        }
        clips.sort_by(|a, b| a.clip.timeline_start.partial_cmp(&b.clip.timeline_start).unwrap());
        track_clips.push(clips);
    }
    (track_clips, have_video, have_audio)
}

/// Build a pre-process command that normalises a single clip to a standardised
/// intermediate stream: CFR at out_fps, correct resolution, filters applied,
/// PTS starting at 0. Outputs to stdout (pipe:1) so we can pipe into the next stage.
fn build_preprocess_cmd(
    ffmpeg: &Path,
    clip: &Clip,
    asset_path: &str,
    is_image: bool,
    has_audio: bool,
    out_w: u32,
    out_h: u32,
    out_fps: u32,
    include_audio: bool,
) -> Command {
    let dur = (clip.source_end - clip.source_start).max(0.05);
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y");

    if is_image {
        cmd.args(["-loop", "1", "-t", &fmt_dur(dur), "-i", asset_path]);
    } else {
        // Input seeking + duration. -noautorotate avoids rotation metadata issues.
        cmd.args(["-noautorotate", "-ss", &fmt_dur(clip.source_start), "-t", &fmt_dur(dur), "-i", asset_path]);
    }

    // --- Video filter: normalise to CFR, scale, filter, fade, text ---
    let mut vf = format!(
        "fps={r},setpts=PTS-STARTPTS,scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1",
        r = out_fps, w = out_w, h = out_h
    );
    vf.push_str(&format!(
        ",eq=brightness={b}:contrast={c}:saturation={s}",
        b = clip.filters.brightness,
        c = 1.0 + clip.filters.contrast,
        s = 1.0 + clip.filters.saturation
    ));
    if clip.fade_in > 0.0 {
        vf.push_str(&format!(",fade=t=in:st=0:d={}", fmt_dur(clip.fade_in)));
    }
    if clip.fade_out > 0.0 {
        let st = (dur - clip.fade_out).max(0.0);
        vf.push_str(&format!(",fade=t=out:st={}:d={}", fmt_dur(st), fmt_dur(clip.fade_out)));
    }
    if let Some(txt) = &clip.text_overlay {
        if !txt.text.trim().is_empty() {
            vf.push_str(&format!(
                ",drawtext=text={txt}:x=(main_w*{px})-tw/2:y=(main_h*{py})-th/2:fontsize={fs}:fontcolor={color}:borderw=2:bordercolor=black@0.6:line_spacing=4",
                txt = escape_drawtext(&txt.text),
                px = txt.x, py = txt.y,
                fs = (out_h as f64 * txt.font_size).round() as u32,
                color = txt.color
            ));
        }
    }
    cmd.args(["-vf", &vf]);
    cmd.args(["-c:v", "rawvideo", "-pix_fmt", "yuv420p", "-f", "nut"]);

    // --- Audio ---
    if include_audio && has_audio {
        let mut af = format!("aresample=async=1,asetpts=PTS-STARTPTS");
        if (clip.volume - 1.0).abs() > 0.001 {
            af.push_str(&format!(",volume={}", clip.volume));
        }
        if clip.fade_in > 0.0 {
            af.push_str(&format!(",afade=t=in:st=0:d={}", fmt_dur(clip.fade_in)));
        }
        if clip.fade_out > 0.0 {
            let st = (dur - clip.fade_out).max(0.0);
            af.push_str(&format!(",afade=t=out:st={}:d={}", fmt_dur(st), fmt_dur(clip.fade_out)));
        }
        cmd.args(["-af", &af, "-c:a", "pcm_s16le"]);
    } else {
        cmd.arg("-an");
    }

    cmd.arg("pipe:1");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd
}

#[tauri::command]
pub fn render_project(
    app: AppHandle,
    project: Project,
    settings: ExportSettings,
) -> Result<(), String> {
    let total = project_duration(&project);
    if total <= 0.0 { return Err("Timeline is empty".to_string()); }
    if settings.output_path.is_empty() { return Err("No output path".to_string()); }

    let ffmpeg = resolve_ffmpeg(&app).ok_or_else(|| "FFmpeg not found".to_string())?;
    let out_w = settings.width.unwrap_or(project.width);
    let out_h = settings.height.unwrap_or(project.height);
    let out_fps = settings.fps.unwrap_or(project.fps).max(1);

    let _ = app.emit("kve://render-progress", RenderProgress {
        progress: 0.0, stage: "Preparing".to_string(), done: false, error: None,
    });

    let (track_clips, _have_video, _have_audio) = collect_clips(&project, &app);

    // We flatten clips into a single timeline: for each track, produce a
    // pre-processed intermediate file, then use the concat demuxer + offset
    // to place them on the timeline, finally encode with the chosen codec.

    // Strategy: pre-process each clip into a normalised .nut intermediate.
    // Then build a concat file that inserts black gaps between clips.
    // This avoids the overlay chain entirely and handles any fps VFR source.

    let tmp_dir = std::env::temp_dir().join("kantan-video-edit-render");
    let _ = fs::create_dir_all(&tmp_dir);
    // Clean old intermediates
    let _ = fs::remove_dir_all(&tmp_dir);
    let _ = fs::create_dir_all(&tmp_dir);

    let mut intermediate_files: Vec<String> = Vec::new(); // per-clip .nut paths
    let mut clip_count = 0usize;

    // --- Phase 1: Pre-process each clip to normalised NUT intermediates ---
    for (ti, clips) in track_clips.iter().enumerate() {
        let track = &project.tracks[ti];
        let include_audio = track.track_type == "audio" || (track.track_type == "video" && !track.muted);
        let skip_video = track.track_type == "video" && track.hidden;

        for rc in clips {
            let clip = &rc.clip;
            if skip_video && !rc.has_audio { continue; }
            // If this is an audio clip on a muted track, skip
            if track.track_type == "audio" && track.muted { continue; }

            let nut_path = tmp_dir.join(format!("clip_{:04}.nut", clip_count));
            clip_count += 1;

            let mut preprocess = build_preprocess_cmd(
                &ffmpeg, clip, &rc.asset_path, rc.is_image,
                rc.has_audio, out_w, out_h, out_fps, include_audio && rc.has_audio,
            );

            let output_nut = nut_path.to_string_lossy().to_string();

            // Write to file instead of pipe for concat demuxer compatibility
            preprocess.stdout(Stdio::null());
            preprocess.arg(&output_nut[..]);

            log::info!("Pre-processing clip {} -> {}", clip.id, output_nut);

            let status = preprocess.status().map_err(|e| format!("Pre-process failed: {}", e))?;
            if !status.success() {
                return Err(format!("Pre-process failed for clip {} (exit {:?})", clip.id, status.code()));
            }

            intermediate_files.push(output_nut);
        }
    }

    if intermediate_files.is_empty() {
        return Err("No clips to render".to_string());
    }

    // --- Phase 2: Build concat demuxer input ---
    // For each visible video track, build a concat entry that includes the clip
    // at the right timeline position with black padding for gaps.
    // For audio tracks, build separate concat entries.

    // Simpler approach: since all intermediates are normalised to the same
    // resolution/fps/codec, use concat directly.
    // For gap handling we generate black .nut segments.

    // Actually, the SIMPLEST robust approach for "clip1 then clip2 then clip3"
    // with possible gaps: use concat demuxer with -itsoffset for each clip.
    // But concat doesn't support per-file offset natively.
    //
    // Instead: generate black gap .nut files and interleave them with clips.

    // Build a flat list of (file_path, start_time) for the "main video track"
    // (the topmost visible video track), sorted by timeline position.
    // Everything else is layered on top via overlay from other tracks.
    //
    // Actually, for the common case (single video track, clips sequential),
    // just concat everything with black gap files in between.

    // Let's build a concat list for the dominant video track.
    // We pick the track with the most video content.

    let main_track_idx = project.tracks.iter().enumerate()
        .filter(|(_, t)| t.track_type == "video" && !t.hidden)
        .max_by_key(|(_, t)| t.clips.len())
        .map(|(i, _)| i);

    let main_clips: Vec<&RenderClip> = if let Some(mi) = main_track_idx {
        track_clips[mi].iter().collect()
    } else {
        Vec::new()
    };

    // Build concat list: for each clip on the main track, insert a black gap
    // if there's space before it, then the clip itself.
    let mut concat_entries: Vec<String> = Vec::new();
    let mut cursor = 0.0_f64; // current position on the timeline

    // Map: clip id -> intermediate nut file path
    let mut clip_nut_map: HashMap<String, String> = HashMap::new();
    let mut ci = 0usize;
    for (ti, clips) in track_clips.iter().enumerate() {
        let track = &project.tracks[ti];
        if track.track_type != "video" || track.hidden { ci += clips.len(); continue; }
        for rc in clips {
            if ci < intermediate_files.len() {
                clip_nut_map.insert(rc.clip.id.clone(), intermediate_files[ci].clone());
            }
            ci += 1;
        }
    }

    // Generate black gap .nut files
    for rc in main_clips {
        let gap = rc.clip.timeline_start - cursor;
        if gap > 0.02 {
            // Generate a black gap of `gap` seconds
            let gap_path = tmp_dir.join(format!("gap_{:.3}.nut", cursor));
            let gap_str = gap_path.to_string_lossy().to_string();
            let mut gap_cmd = Command::new(&ffmpeg);
            gap_cmd.args([
                "-y", "-f", "lavfi",
                "-i", &format!("color=c=black:s={}x{}:d={}:r={}", out_w, out_h, fmt_dur(gap), out_fps),
                "-f", "lavfi", "-i", &format!("anullsrc=r={}00:cl=stereo", out_fps),
                "-t", &fmt_dur(gap),
                "-c:v", "rawvideo", "-pix_fmt", "yuv420p",
                "-c:a", "pcm_s16le",
                "-f", "nut",
            ]);
            gap_cmd.arg(&gap_str);
            let st = gap_cmd.status().map_err(|e| format!("Gap gen: {}", e))?;
            if !st.success() {
                return Err(format!("Gap gen failed (exit {:?})", st.code()));
            }
            concat_entries.push(gap_str);
        }

        // Add the clip's intermediate
        if let Some(nut) = clip_nut_map.get(&rc.clip.id) {
            concat_entries.push(nut.clone());
        }

        let clip_dur = (rc.clip.source_end - rc.clip.source_start).max(0.05);
        cursor = rc.clip.timeline_start + clip_dur;
    }

    // Trailing gap to fill up to total duration
    if cursor < total - 0.02 {
        let gap = total - cursor;
        let gap_path = tmp_dir.join(format!("gap_trailing.nut"));
        let gap_str = gap_path.to_string_lossy().to_string();
        let mut gap_cmd = Command::new(&ffmpeg);
        gap_cmd.args([
            "-y", "-f", "lavfi",
            "-i", &format!("color=c=black:s={}x{}:d={}:r={}", out_w, out_h, fmt_dur(gap), out_fps),
            "-f", "lavfi", "-i", &format!("anullsrc=r={}00:cl=stereo", out_fps),
            "-t", &fmt_dur(gap),
            "-c:v", "rawvideo", "-pix_fmt", "yuv420p",
            "-c:a", "pcm_s16le",
            "-f", "nut",
        ]);
        gap_cmd.arg(&gap_str);
        let _ = gap_cmd.status();
        concat_entries.push(gap_str);
    }

    // Write concat list file
    let concat_path = tmp_dir.join("concat.txt");
    {
        let mut f = fs::File::create(&concat_path).map_err(|e| e.to_string())?;
        for entry in &concat_entries {
            // On Windows, use forward slashes and escape single quotes for concat
            let safe = entry.replace('\\', "/");
            writeln!(f, "file '{}'", safe).map_err(|e| e.to_string())?;
        }
    }

    // --- Phase 3: Concat + overlay additional tracks + final encode ---
    let concat_str = concat_path.to_string_lossy().to_string();

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y");
    cmd.args(["-f", "concat", "-safe", "0", "-i", &concat_str]);

    // Add additional video tracks as overlay inputs (tracks above the main one)
    let main_ti = main_track_idx.unwrap_or(0);
    let mut overlay_inputs: Vec<(usize, Clip)> = Vec::new(); // (input_index, clip)
    let mut extra_input_idx = 1; // 0 = concat stream
    for (ti, clips) in track_clips.iter().enumerate() {
        if ti == main_ti { continue; }
        let track = &project.tracks[ti];
        if track.track_type != "video" || track.hidden { continue; }
        for rc in clips {
            if let Some(nut) = clip_nut_map.get(&rc.clip.id) {
                cmd.args(["-i", nut]);
                overlay_inputs.push((extra_input_idx, rc.clip.clone()));
                extra_input_idx += 1;
            }
        }
    }

    // Add additional audio track inputs
    let mut audio_overlay_inputs: Vec<(usize, Clip)> = Vec::new();
    for (ti, clips) in track_clips.iter().enumerate() {
        let track = &project.tracks[ti];
        if track.track_type != "audio" || track.muted { continue; }
        for rc in clips {
            if let Some(nut) = clip_nut_map.get(&rc.clip.id) {
                cmd.args(["-i", nut]);
                audio_overlay_inputs.push((extra_input_idx, rc.clip.clone()));
                extra_input_idx += 1;
            }
        }
    }

    // Build filter_complex
    let mut graph = String::new();
    let mut last_label = "0:v".to_string(); // concat base video
    let mut overlay_n = 0usize;

    // Overlay additional video tracks
    for (inp_idx, clip) in &overlay_inputs {
        let dur = (clip.source_end - clip.source_start).max(0.05);
        let label = format!("ov{}", overlay_n);
        let next = format!("ovr{}", overlay_n);
        overlay_n += 1;
        // Scale + PTS normalise the overlay clip then composite it
        graph.push_str(&format!(
            "[{idx}:v]setpts=PTS+{delay}/TB[{label}];",
            idx = inp_idx,
            delay = fmt_dur(clip.timeline_start),
            label = label,
        ));
        graph.push_str(&format!(
            "[{prev}][{label}]overlay=x=0:y=0:enable='between(t,{s},{e})':format=auto[{next}];",
            prev = last_label,
            label = label,
            s = fmt_dur(clip.timeline_start),
            e = fmt_dur(clip.timeline_start + dur),
            next = next,
        ));
        last_label = next;
    }

    // Audio: mix base audio (from concat) with additional audio track inputs
    if !audio_overlay_inputs.is_empty() {
        let mut audio_labels = vec!["0:a".to_string()];
        for (i, (inp_idx, clip)) in audio_overlay_inputs.iter().enumerate() {
            let al = format!("ao{}", i);
            let ms = (clip.timeline_start * 1000.0).round() as u64;
            graph.push_str(&format!(
                "[{idx}:a]adelay={ms}|{ms},asetpts=PTS+{delay}/TB[{al}];",
                idx = inp_idx,
                ms = ms,
                delay = fmt_dur(clip.timeline_start),
                al = al,
            ));
            audio_labels.push(al);
        }
        let input_str: String = audio_labels.iter().map(|l| format!("[{}]", l)).collect();
        graph.push_str(&format!(
            "{}amix=inputs={n}:duration=longest:normalize=0[aout]",
            input_str,
            n = audio_labels.len(),
        ));
    } else {
        graph.push_str("[0:a]anull[aout]");
    }

    // Final video label mapping
    graph.push_str(&format!(";[{last}]null[vout]", last = last_label));

    log::info!("Render filter_complex: {}", graph);
    log::info!("Render concat file: {}", concat_str);

    cmd.args(["-filter_complex", &graph]);
    cmd.args(["-map", "[vout]", "-map", "[aout]"]);

    // Encoding
    cmd.args(["-c:v", &settings.video_codec]);
    if settings.video_codec.starts_with("libx26") {
        cmd.args(["-crf", &settings.crf.to_string()]);
        cmd.args(["-preset", &settings.preset]);
    }
    cmd.args(["-pix_fmt", "yuv420p", "-r", &out_fps.to_string()]);
    cmd.args(["-c:a", &settings.audio_codec]);
    cmd.args(["-b:a", "192k"]);
    cmd.args(["-movflags", "+faststart"]);
    cmd.arg(&settings.output_path);

    log::info!("Final ffmpeg args: {:?}", cmd);

    // Run with progress monitoring
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("ffmpeg spawn: {}", e))?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let app_clone = app.clone();
    let total_dur = total;

    std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if let Some(time) = parse_ffmpeg_time(&line) {
                let prog = (time / total_dur).clamp(0.0, 0.999);
                let _ = app_clone.emit("kve://render-progress", RenderProgress {
                    progress: prog, stage: "Rendering".to_string(), done: false, error: None,
                });
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });

    let status = child.wait().map_err(|e| format!("ffmpeg wait: {}", e))?;

    if status.success() {
        let _ = app.emit("kve://render-progress", RenderProgress {
            progress: 1.0, stage: "Done".to_string(), done: true, error: None,
        });
        // Cleanup temp files
        let _ = fs::remove_dir_all(&tmp_dir);
        Ok(())
    } else {
        let msg = format!("ffmpeg exited with code {:?}", status.code());
        let _ = app.emit("kve://render-progress", RenderProgress {
            progress: 0.0, stage: "Error".to_string(), done: false, error: Some(msg.clone()),
        });
        Err(msg)
    }
}

fn parse_ffmpeg_time(line: &str) -> Option<f64> {
    let mut t = line.split("time=");
    let _ = t.next();
    let rest = t.next()?;
    let val = rest.split_whitespace().next()?;
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
