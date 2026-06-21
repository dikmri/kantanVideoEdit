pub mod ffmpeg;
pub mod probe;
pub mod render;

use tauri::{AppHandle, Manager};
use tauri_plugin_log::{Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_targets = vec![
        // Write to the OS-specific log directory:
        //   Windows: %APPDATA%\app.kantan.videoedit\logs\
        //   macOS:   ~/Library/Logs/app.kantan.videoedit/
        //   Linux:   ~/.local/share/app.kantan.videoedit/logs/ (or XDG)
        Target::new(TargetKind::LogDir {
            file_name: Some("kantan-video-edit".to_string()),
        }),
        // Also mirror to the terminal (useful when running `tauri dev`)
        Target::new(TargetKind::Stdout),
        // Forward frontend log() calls into the same sinks
        Target::new(TargetKind::Webview),
    ];

    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets(log_targets)
                .level(log_level)
                .level_for("kantan_video_edit_lib", log::LevelFilter::Trace)
                .level_for("tao", log::LevelFilter::Warn)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Log where the log file lives so the user can find it easily.
            if let Ok(log_dir) = app.path().app_log_dir() {
                log::info!("Log directory: {}", log_dir.display());
            }
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe::probe_media,
            probe::probe_media_batch,
            render::render_project,
            ffmpeg::check_ffmpeg,
            ffmpeg::reveal_in_folder,
            ffmpeg::get_log_dir,
            ffmpeg::write_frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kantan Video Edit");
}
