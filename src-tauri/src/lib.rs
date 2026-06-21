pub mod ffmpeg;
pub mod probe;
pub mod render;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").expect("no main window");
                window.open_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe::probe_media,
            probe::probe_media_batch,
            render::render_project,
            ffmpeg::check_ffmpeg,
            ffmpeg::reveal_in_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kantan Video Edit");
}
