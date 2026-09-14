use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::db::connection;
use crate::error::{AppError, Result};
use crate::utils::file;

// ---------------------------------------------------------------------------
// 应用指针配置（存放在系统应用数据目录，指向当前曲谱库位置）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub maximized: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub library_path: Option<String>,
    pub last_window: Option<WindowState>,
    pub last_song_id: Option<i64>,
}

pub async fn read_app_config(app: &AppHandle) -> Result<AppConfig> {
    let path = file::app_config_path(app)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(_) => Ok(AppConfig::default()),
    }
}

pub async fn write_app_config(app: &AppHandle, cfg: &AppConfig) -> Result<()> {
    let path = file::app_config_path(app)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec_pretty(cfg)?;
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// settings.json（与数据库同目录）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    pub auto_start: bool,
    pub library_path: String,
    pub language: String,
    pub restore_window: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            library_path: String::new(),
            language: "zh-CN".to_string(),
            restore_window: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,              // system | light | dark
    pub default_view_mode: String,  // fixed | scroll | dual_horizontal
    pub bg_color: String,
    pub zoom_mode: String,          // fit_width | fit_height | actual
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            default_view_mode: "fixed".to_string(),
            bg_color: "#1A1A1A".to_string(),
            zoom_mode: "fit_height".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiControlSettings {
    pub camera_id: i64,
    pub sensitivity: String, // low | medium | high
    pub cooldown_ms: i64,
    pub show_preview: bool,
    pub page_effect: bool,
}

impl Default for AiControlSettings {
    fn default() -> Self {
        Self {
            camera_id: 0,
            sensitivity: "medium".to_string(),
            cooldown_ms: 1500,
            show_preview: true,
            page_effect: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutSettings {
    pub next_page: String,
    pub prev_page: String,
    pub toggle_fullscreen: String,
    pub toggle_autoscroll: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            next_page: "ArrowRight".to_string(),
            prev_page: "ArrowLeft".to_string(),
            toggle_fullscreen: "F11".to_string(),
            toggle_autoscroll: "Space".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub ai_control: AiControlSettings,
    pub shortcuts: ShortcutSettings,
}

// ---------------------------------------------------------------------------
// 曲谱库路径管理
// ---------------------------------------------------------------------------

/// 默认曲谱库目录：用户文档目录/FingertipTabs
pub fn default_library_path() -> Result<PathBuf> {
    let docs = dirs::document_dir()
        .ok_or_else(|| AppError::msg("无法获取系统文档目录"))?;
    Ok(docs.join("FingertipTabs"))
}

/// 解析当前曲谱库目录（首次调用时写入指针配置）
pub async fn get_library_path(app: &AppHandle) -> Result<PathBuf> {
    let cfg = read_app_config(app).await?;
    if let Some(p) = cfg.library_path.as_deref().filter(|s| !s.trim().is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let default = default_library_path()?;
    let mut cfg = cfg;
    cfg.library_path = Some(default.to_string_lossy().into_owned());
    write_app_config(app, &cfg).await?;
    Ok(default)
}

/// 初始化曲谱库目录结构、settings.json 与数据库连接（应用启动时调用）
pub async fn ensure_library_ready(app: &AppHandle) -> Result<SqlitePool> {
    let lib = get_library_path(app).await?;
    file::ensure_dir(&lib.join("Resources/Images")).await?;
    file::ensure_dir(&lib.join("Resources/Thumbnails")).await?;
    file::ensure_dir(&lib.join("Backups")).await?;

    let settings_path = lib.join("settings.json");
    if !tokio::fs::try_exists(&settings_path).await.unwrap_or(false) {
        let mut settings = AppSettings::default();
        settings.general.library_path = lib.to_string_lossy().into_owned();
        save_settings_file(&settings_path, &settings).await?;
    }

    let db_path = lib.join("FingertipTabs.db");
    connection::init_db(&db_path).await
}

pub async fn load_settings_file(path: &Path) -> Result<AppSettings> {
    let bytes = tokio::fs::read(path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub async fn save_settings_file(path: &Path, settings: &AppSettings) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(settings)?;
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri 命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings> {
    let lib = get_library_path(&app).await?;
    let settings_path = lib.join("settings.json");
    let mut settings = match load_settings_file(&settings_path).await {
        Ok(s) => s,
        Err(_) => AppSettings::default(),
    };
    settings.general.library_path = lib.to_string_lossy().into_owned();
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<()> {
    use tauri_plugin_autostart::ManagerExt;

    let lib = get_library_path(&app).await?;
    let mut settings = settings;
    settings.general.library_path = lib.to_string_lossy().into_owned();
    save_settings_file(&lib.join("settings.json"), &settings).await?;

    // 同步开机自启状态
    let auto = app.autolaunch();
    if settings.general.auto_start {
        auto.enable().map_err(|e| AppError::msg(format!("设置开机自启失败: {e}")))?;
    } else {
        auto.disable().map_err(|e| AppError::msg(format!("关闭开机自启失败: {e}")))?;
    }
    Ok(())
}

/// 修改存储路径：移动 Resources 与数据库文件到新目录（建议迁移后重启应用）
#[tauri::command]
pub async fn migrate_library(
    app: AppHandle,
    state: tauri::State<'_, crate::db::connection::AppState>,
    new_path: String,
) -> Result<()> {
    let old = get_library_path(&app).await?;
    let new_path = PathBuf::from(new_path.trim());
    if new_path == old {
        return Err(AppError::msg("新路径与当前路径相同"));
    }
    if new_path.as_os_str().is_empty() {
        return Err(AppError::msg("路径不能为空"));
    }
    tokio::fs::create_dir_all(&new_path).await?;
    // 目标目录必须为空，避免覆盖已有数据
    let mut entries = tokio::fs::read_dir(&new_path).await?;
    if entries.next_entry().await?.is_some() {
        return Err(AppError::msg("目标目录不为空，为避免数据覆盖请选择空目录"));
    }

    // 关闭数据库连接池，保证 Windows 下文件可移动
    state.pool.close().await;
    let lib = old;
    for name in ["FingertipTabs.db", "settings.json"] {
        let src = lib.join(name);
        if tokio::fs::try_exists(&src).await.unwrap_or(false) {
            file::move_path(&src, &new_path.join(name))?;
        }
    }
    let resources = lib.join("Resources");
    if tokio::fs::try_exists(&resources).await.unwrap_or(false) {
        file::move_path(&resources, &new_path.join("Resources"))?;
    }

    // 更新指针配置与 settings.json 中的路径记录
    let mut cfg = read_app_config(&app).await?;
    cfg.library_path = Some(new_path.to_string_lossy().into_owned());
    write_app_config(&app, &cfg).await?;

    let settings_path = new_path.join("settings.json");
    let mut settings = load_settings_file(&settings_path).await.unwrap_or_default();
    settings.general.library_path = new_path.to_string_lossy().into_owned();
    save_settings_file(&settings_path, &settings).await?;

    Ok(())
}

/// 一键备份：将整个曲谱库打包为 ZIP
#[tauri::command]
pub async fn backup_library(app: AppHandle, dest: String) -> Result<String> {
    let lib = get_library_path(&app).await?;
    let dest = PathBuf::from(dest);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let file = std::fs::File::create(&dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let prefix = lib.to_string_lossy().to_string() + std::path::MAIN_SEPARATOR_STR;
    let mut added = 0usize;
    for entry in walkdir::WalkDir::new(&lib).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = path.to_string_lossy().to_string();
        let rel = rel.strip_prefix(&prefix).unwrap_or(&rel);
        // 跳过备份目录自身，避免递归打包
        if rel.starts_with("Backups") {
            continue;
        }
        if zip
            .start_file(rel.replace('\\', "/"), options)
            .is_err()
        {
            continue;
        }
        let mut f = std::fs::File::open(path)?;
        std::io::copy(&mut f, &mut zip)?;
        added += 1;
    }
    zip.finish()?;
    Ok(format!("{}（{added} 个文件）", dest.display()))
}

/// 从 ZIP 备份恢复曲谱库（覆盖当前库，恢复后建议重启应用）
#[tauri::command]
pub async fn restore_library(
    app: AppHandle,
    state: tauri::State<'_, crate::db::connection::AppState>,
    src: String,
) -> Result<()> {
    let lib = get_library_path(&app).await?;
    let src = PathBuf::from(src);
    if !tokio::fs::try_exists(&src).await.unwrap_or(false) {
        return Err(AppError::msg("备份文件不存在"));
    }

    // 覆盖数据库文件前先关闭连接池
    state.pool.close().await;

    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::open(&src)?;
        let mut zip = zip::ZipArchive::new(file)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let target = lib.join(&rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&target)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| AppError::msg(format!("恢复任务失败: {e}")))?
}
