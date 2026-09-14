// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod error;
mod utils;

use tauri::Manager;

use db::connection::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化曲谱库（目录结构 + settings.json + 数据库）
            let handle = app.handle().clone();
            let pool = tauri::async_runtime::block_on(async move {
                commands::settings::ensure_library_ready(&handle).await
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            app.manage(AppState { pool });

            // 启动时恢复窗口状态
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(cfg) = commands::settings::read_app_config(&handle).await {
                    if let Some(win) = cfg.last_window {
                        let _ = commands::system::apply_window_state(handle.clone(), win).await;
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_songs,
            commands::read_image_base64,
            commands::get_songs,
            commands::get_song,
            commands::update_song,
            commands::set_song_pages,
            commands::add_pages_to_song,
            commands::delete_song,
            commands::get_collections,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::get_settings,
            commands::save_settings,
            commands::migrate_library,
            commands::backup_library,
            commands::restore_library,
            commands::capture_screen,
            commands::get_app_config,
            commands::save_app_config,
            commands::apply_window_state,
            commands::restart_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
