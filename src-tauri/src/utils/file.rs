use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

/// 支持的图片扩展名白名单
pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// 支持的 Guitar Pro 扩展名白名单
pub const GP_EXTS: &[&str] = &["gp3", "gp4", "gp5", "gpx", "gp"];

pub fn extension_of(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| AppError::msg(format!("文件缺少扩展名: {}", path.display())))?;
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(AppError::msg(format!(
            "不支持的图片格式: .{ext}（支持 {}）",
            IMAGE_EXTS.join("/")
        )));
    }
    Ok(ext)
}

/// 校验并返回 GP 文件扩展名（小写）
pub fn gp_extension_of(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| AppError::msg(format!("文件缺少扩展名: {}", path.display())))?;
    if !GP_EXTS.contains(&ext.as_str()) {
        return Err(AppError::msg(format!(
            "不支持的曲谱格式: .{ext}（支持 {}）",
            GP_EXTS.join("/")
        )));
    }
    Ok(ext)
}

pub fn normalize_ext(ext: &str) -> Result<String> {
    let ext = ext.to_lowercase();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(AppError::msg(format!("不支持的图片格式: .{ext}")));
    }
    Ok(ext)
}

pub async fn ensure_dir(dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    Ok(())
}

pub async fn remove_file_if_exists(path: &Path) -> Result<()> {
    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

/// 跨盘符安全移动：优先 rename，失败（ExDev）则复制后删除
pub fn move_path(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            if src.is_dir() {
                copy_dir_recursive(src, dest)?;
                fs::remove_dir_all(src)?;
            } else {
                fs::copy(src, dest)?;
                fs::remove_file(src)?;
            }
            Ok(())
        }
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn app_config_path(app: &tauri::AppHandle) -> Result<PathBuf> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::msg(format!("无法获取应用数据目录: {e}")))?;
    Ok(dir.join("config.json"))
}
