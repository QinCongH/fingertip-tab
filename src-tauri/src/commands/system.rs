use base64::Engine;
use serde::Serialize;
use tauri::AppHandle;

use crate::commands::settings::{read_app_config, write_app_config, AppConfig, WindowState};
use crate::error::{AppError, Result};

/// 屏幕截图结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResult {
    pub data: String,
    pub width: u32,
    pub height: u32,
}

/// 捕获主显示器屏幕（截图录入）
#[tauri::command]
pub async fn capture_screen() -> Result<CaptureResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let monitors = xcap::Monitor::all()
            .map_err(|e| AppError::msg(format!("无法获取显示器列表: {e}")))?;
        let monitor = monitors
            .first()
            .ok_or_else(|| AppError::msg("未找到可用显示器"))?;
        let img = monitor
            .capture_image()
            .map_err(|e| AppError::msg(format!("屏幕捕获失败: {e}")))?;
        // xcap 内部使用的 image crate 版本可能不同，取原始像素后重建
        let (width, height) = (img.width(), img.height());
        let raw = img.into_raw();
        let rgba = ::image::RgbaImage::from_raw(width, height, raw)
            .ok_or_else(|| AppError::msg("屏幕图像数据无效"))?;

        let mut png = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut png, ::image::ImageFormat::Png)
            .map_err(|e| AppError::msg(format!("图片编码失败: {e}")))?;

        let data = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png.into_inner())
        );
        Ok(CaptureResult { data, width, height })
    })
    .await
    .map_err(|e| AppError::msg(format!("截图任务失败: {e}")))?
}

/// 读取应用配置（上次窗口状态、最后打开的曲谱等）
#[tauri::command]
pub async fn get_app_config(app: AppHandle) -> Result<AppConfig> {
    read_app_config(&app).await
}

/// 保存应用配置（窗口关闭时持久化窗口状态）
#[tauri::command]
pub async fn save_app_config(app: AppHandle, config: AppConfig) -> Result<()> {
    write_app_config(&app, &config).await
}

/// 恢复窗口状态
#[tauri::command]
pub async fn apply_window_state(app: AppHandle, state: WindowState) -> Result<()> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        if state.maximized == Some(true) {
            let _ = window.maximize();
        } else {
            if let (Some(w), Some(h)) = (state.width, state.height) {
                if w > 0.0 && h > 0.0 {
                    let _ = window.set_size(tauri::LogicalSize::new(w, h));
                }
            }
            if let (Some(x), Some(y)) = (state.x, state.y) {
                let _ = window.set_position(tauri::LogicalPosition::new(x, y));
            }
        }
    }
    Ok(())
}

/// 重启应用（存储迁移/恢复备份后使用）
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}
