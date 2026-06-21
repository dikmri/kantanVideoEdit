use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
pub struct FfmpegInfo {
    pub available: bool,
    pub version: String,
    pub path: String,
}

/// Resolve the FFmpeg executable path.
/// Order: environment override -> bundled sidecar -> system PATH.
pub fn resolve_ffmpeg(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KVE_FFMPEG_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }

    // Bundled sidecar: look in the resource/bin dir next to the executable.
    if let Some(res_dir) = app.path().resource_dir().ok() {
        let names = if cfg!(target_os = "windows") {
            vec!["ffmpeg.exe", "bin/ffmpeg.exe"]
        } else {
            vec!["ffmpeg", "bin/ffmpeg"]
        };
        for n in names {
            let candidate = res_dir.join(n);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // Also check next to the current executable (dev / portable builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let name = if cfg!(target_os = "windows") {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            };
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // System PATH
    which_ffmpeg()
}

fn which_ffmpeg() -> Option<PathBuf> {
    which::which("ffmpeg").ok()
}

/// Resolve the ffprobe executable alongside the resolved ffmpeg.
pub fn resolve_ffprobe(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KVE_FFPROBE_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Some(ff) = resolve_ffmpeg(app) {
        if let Some(stem) = ff.file_stem() {
            let probe_name = if cfg!(target_os = "windows") {
                "ffprobe.exe"
            } else {
                "ffprobe"
            };
            let sibling = ff
                .with_file_name(probe_name);
            if sibling.exists() {
                return Some(sibling);
            }
            // bin/ffmpeg -> bin/ffprobe
            let _ = stem;
        }
        // try replacing "ffmpeg" with "ffprobe" in the path
        let s = ff.to_string_lossy().replace("ffmpeg", "ffprobe");
        let candidate = PathBuf::from(s);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    which::which("ffprobe").ok()
}

#[tauri::command]
pub fn check_ffmpeg(app: AppHandle) -> FfmpegInfo {
    match resolve_ffmpeg(&app) {
        Some(path) => {
            let version = match Command::new(&path).arg("-version").output() {
                Ok(out) => {
                    let txt = String::from_utf8_lossy(&out.stdout);
                    txt.lines().next().unwrap_or("ffmpeg").to_string()
                }
                Err(_) => String::from("ffmpeg"),
            };
            FfmpegInfo {
                available: true,
                version,
                path: path.to_string_lossy().to_string(),
            }
        }
        None => FfmpegInfo {
            available: false,
            version: String::new(),
            path: String::new(),
        },
    }
}

#[tauri::command]
pub fn reveal_in_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    #[cfg(target_os = "windows")]
    {
        if p.is_dir() {
            Command::new("explorer")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            Command::new("explorer")
                .args(["/select,", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let target = if p.is_dir() { path.clone() } else {
            p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or(path.clone())
        };
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Return the absolute path of the log directory so the UI can display / reveal it.
#[tauri::command]
pub fn get_log_dir(app: AppHandle) -> Result<String, String> {
    app.path()
        .app_log_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Write a frontend log message into the Rust logger (→ log file).
/// Called via invoke so it works even before the log JS plugin is fully initialised.
#[tauri::command]
pub fn write_frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!("[frontend] {}", message),
        "warn" => log::warn!("[frontend] {}", message),
        "info" => log::info!("[frontend] {}", message),
        _ => log::debug!("[frontend] {}", message),
    }
}
