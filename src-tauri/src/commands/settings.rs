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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GpPlayerSettings {
    /// 谱面缩放 0.5–2.0
    pub zoom: f64,
    /// 是否显示五线谱
    pub show_standard_notation: bool,
    /// 是否显示和弦名
    pub show_chord_names: bool,
    /// 预备拍拍数：0 关闭 / 1 / 2 / 4
    pub count_in_beats: i64,
    /// 默认滚动模式：continuous | off_screen | smooth | off
    pub scroll_mode: String,
    /// 节拍器音量 0–100
    pub metronome_volume: i64,
    /// 上次导入选择的曲谱类型：image | gp
    pub last_import_format: String,
}

impl Default for GpPlayerSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            show_standard_notation: true,
            show_chord_names: true,
            count_in_beats: 0,
            scroll_mode: "smooth".to_string(),
            metronome_volume: 60,
            last_import_format: "image".to_string(),
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
    pub gp_player: GpPlayerSettings,
}

// ---------------------------------------------------------------------------
// 曲谱库路径管理
// ---------------------------------------------------------------------------

/// 目标目录探测结果，供前端判断是「加载已有工作区」还是「初始化新工作区」
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProbe {
    pub exists: bool,
    pub empty: bool,
    pub has_db: bool,
    pub has_resources: bool,
}

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
    ensure_library_ready_at(app, lib).await
}

/// 在指定目录初始化曲谱库结构（切换工作区时复用；对已有库为幂等操作）
/// `app` 保留用于后续按库加载扩展设置项，当前初始化过程无需访问全局状态
pub async fn ensure_library_ready_at(_app: &AppHandle, lib: PathBuf) -> Result<SqlitePool> {
    file::ensure_dir(&lib.join("Resources/Images")).await?;
    file::ensure_dir(&lib.join("Resources/Thumbnails")).await?;
    file::ensure_dir(&lib.join("Resources/GPFiles")).await?;
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

    // 同步开机自启状态；dev 模式下注册表项可能不存在，失败仅告警不阻断
    let auto = app.autolaunch();
    if settings.general.auto_start {
        if let Err(e) = auto.enable() {
            log::warn!("设置开机自启失败: {e}");
        }
    } else {
        if let Err(e) = auto.disable() {
            log::warn!("关闭开机自启失败: {e}");
        }
    }
    Ok(())
}

/// 探测指定目录是否为可用的曲谱库（以数据库文件为准）
pub async fn probe_library(path: &Path) -> LibraryProbe {
    let db_path = path.join("FingertipTabs.db");
    let has_db = tokio::fs::try_exists(&db_path).await.unwrap_or(false);

    // 目录不存在或为空 → 可直接初始化为新工作区
    let empty = match tokio::fs::read_dir(path).await {
        Ok(mut entries) => entries.next_entry().await.map(|_| false).unwrap_or(true),
        Err(_) => true,
    };

    LibraryProbe {
        exists: tokio::fs::try_exists(path).await.unwrap_or(false),
        empty,
        has_db,
        has_resources: tokio::fs::try_exists(path.join("Resources")).await.unwrap_or(false),
    }
}

/// 选定文件夹作为工作区：符合曲谱库结构则直接加载，否则初始化为新工作区。
/// 与旧的「迁移」语义不同——本操作不移动任何文件，原库数据原地保留。
#[tauri::command]
pub async fn switch_library(
    app: AppHandle,
    state: tauri::State<'_, crate::db::connection::AppState>,
    new_path: String,
    init_if_missing: bool,
) -> Result<()> {
    let old = get_library_path(&app).await?;
    let new_path = PathBuf::from(new_path.trim());
    if new_path.as_os_str().is_empty() {
        return Err(AppError::msg("路径不能为空"));
    }
    if new_path == old {
        return Err(AppError::msg("新路径与当前路径相同"));
    }

    // 校验目标目录：非空且不含数据库文件时，需前端确认后才能初始化
    let probe = probe_library(&new_path).await;
    if !probe.has_db && !probe.empty && !init_if_missing {
        return Err(AppError::msg("DIR_NOT_EMPTY"));
    }

    // 关闭当前连接池（Windows 下文件被占用会导致后续操作失败）
    state.pool.close().await;

    // 初始化目录结构 + settings.json + 数据库；已是曲谱库时此过程幂等
    let pool = ensure_library_ready_at(&app, new_path.clone()).await?;

    // 同步共享上下文（内部 HTTP 服务也持有连接池）
    let pool_for_share = pool.clone();
    let app_for_share = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::share::register_context(pool_for_share, app_for_share);
    });

    // 更新应用级指针配置，供下次启动读取
    let mut cfg = read_app_config(&app).await?;
    cfg.library_path = Some(new_path.to_string_lossy().into_owned());
    write_app_config(&app, &cfg).await?;

    Ok(())
}

/// 探测目录是否已是曲谱库，供前端在确认前展示差异化提示
#[tauri::command]
pub async fn probe_library_path(path: String) -> Result<LibraryProbe> {
    let p = PathBuf::from(path.trim());
    if p.as_os_str().is_empty() {
        return Err(AppError::msg("路径不能为空"));
    }
    Ok(probe_library(&p).await)
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
