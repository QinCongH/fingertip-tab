use std::path::{Path, PathBuf};

use base64::Engine;
use tauri::AppHandle;
use uuid::Uuid;

use crate::db::connection::AppState;
use crate::db::models::{load_song_detail, PageDto, SongDto, SongMetaInput};
use crate::error::{AppError, Result};
use sqlx::Row;
use crate::utils::{file, image};
use crate::commands::settings::get_library_path;

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct SavedPage {
    uuid: String,
    ext: String,
}

/// 把一张图片保存进曲谱库（生成 UUID 文件名 + 缩略图），返回 (uuid, ext)
async fn save_image_into_library(
    lib: &Path,
    src: &Path,
    bytes: Option<Vec<u8>>,
    ext_input: Option<String>,
) -> Result<SavedPage> {
    let ext = if bytes.is_some() {
        let raw = ext_input.ok_or_else(|| AppError::msg("缺少图片格式"))?;
        file::normalize_ext(&raw)?
    } else {
        file::extension_of(src)?
    };

    let images_dir = lib.join("Resources/Images");
    let thumbs_dir = lib.join("Resources/Thumbnails");
    file::ensure_dir(&images_dir).await?;
    file::ensure_dir(&thumbs_dir).await?;

    let uuid = Uuid::new_v4().to_string();
    let dest = images_dir.join(format!("{uuid}.{ext}"));

    if let Some(data) = bytes {
        let dest2 = dest.clone();
        let ext2 = ext.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let fmt = match ext2.as_str() {
                "png" => ::image::ImageFormat::Png,
                "webp" => ::image::ImageFormat::WebP,
                "bmp" => ::image::ImageFormat::Bmp,
                "gif" => ::image::ImageFormat::Gif,
                _ => ::image::ImageFormat::Jpeg,
            };
            image::save_image_bytes(&data, &dest2, fmt)
        })
        .await
        .map_err(|e| AppError::msg(format!("保存图片失败: {e}")))??;
    } else {
        tokio::fs::copy(src, &dest).await?;
    }

    // 生成缩略图（失败不阻断导入）
    let thumb = thumbs_dir.join(format!("{uuid}_thumb.jpg"));
    let thumb_dest = thumb.clone();
    let img_src = dest.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        image::generate_thumbnail(&img_src, &thumb_dest).ok()
    })
    .await;

    Ok(SavedPage { uuid, ext })
}

async fn cleanup_saved_pages(lib: &Path, pages: &[SavedPage]) {
    for p in pages {
        let _ = file::remove_file_if_exists(&lib.join("Resources/Images").join(format!("{}.{}", p.uuid, p.ext))).await;
        let _ = file::remove_file_if_exists(&lib.join("Resources/Thumbnails").join(format!("{}_thumb.jpg", p.uuid))).await;
    }
}

async fn insert_song_with_pages(
    state: &AppState,
    meta: &SongMetaInput,
    pages: &[SavedPage],
    collection_ids: &[i64],
) -> Result<i64> {
    let now = now_ts();
    let first = pages
        .first()
        .ok_or_else(|| AppError::msg("至少需要一张图片"))?;

    let mut tx = state.pool.begin().await?;
    let song_id: i64 = sqlx::query(
        "INSERT INTO songs (uuid, title, artist, album, tuning, bpm, tags, file_type, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&first.uuid)
    .bind(meta.title.trim())
    .bind(meta.artist.trim())
    .bind(meta.album.trim())
    .bind(meta.tuning.trim())
    .bind(meta.bpm)
    .bind(meta.tags.trim())
    .bind(&first.ext)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for (i, p) in pages.iter().enumerate() {
        sqlx::query("INSERT INTO pages (song_id, uuid, file_type, sort_order, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(song_id)
            .bind(&p.uuid)
            .bind(&p.ext)
            .bind(i as i64)
            .bind(now)
            .execute(&mut *tx)
            .await?;
    }

    for cid in collection_ids {
        sqlx::query("INSERT OR IGNORE INTO song_collections (song_id, collection_id) VALUES (?, ?)")
            .bind(song_id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(song_id)
}

/// 内存图片载荷（截图裁剪 / 剪贴板粘贴），data 为 dataURL 或裸 base64
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePayload {
    pub data: String,
    pub ext: String,
}

impl ImagePayload {
    fn decode(&self) -> Result<Vec<u8>> {
        let payload = self
            .data
            .strip_prefix("data:image/")
            .and_then(|rest| rest.split_once(";base64,"))
            .map(|(_, b64)| b64.to_string())
            .unwrap_or_else(|| self.data.clone());
        let bytes = base64::engine::general_purpose::STANDARD.decode(payload.as_bytes())?;
        if bytes.is_empty() {
            return Err(AppError::msg("图片数据为空"));
        }
        Ok(bytes)
    }
}

/// 导入一首曲谱：文件路径 + 内存图片（截图/粘贴）可任意组合，多张即多页
#[tauri::command]
pub async fn import_songs(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
    extra_images: Vec<ImagePayload>,
    meta: SongMetaInput,
    collection_ids: Vec<i64>,
) -> Result<SongDto> {
    if paths.is_empty() && extra_images.is_empty() {
        return Err(AppError::msg("未选择任何图片文件"));
    }
    let title = meta.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::msg("请填写歌名"));
    }

    let lib = get_library_path(&app).await?;
    let mut saved: Vec<SavedPage> = Vec::new();

    for p in &paths {
        let src = PathBuf::from(p);
        if !tokio::fs::try_exists(&src).await.unwrap_or(false) {
            cleanup_saved_pages(&lib, &saved).await;
            return Err(AppError::msg(format!("文件不存在: {p}")));
        }
        match save_image_into_library(&lib, &src, None, None).await {
            Ok(page) => saved.push(page),
            Err(e) => {
                cleanup_saved_pages(&lib, &saved).await;
                return Err(e);
            }
        }
    }

    for payload in &extra_images {
        let bytes = match payload.decode() {
            Ok(b) => b,
            Err(e) => {
                cleanup_saved_pages(&lib, &saved).await;
                return Err(e);
            }
        };
        match save_image_into_library(&lib, Path::new(""), Some(bytes), Some(payload.ext.clone()))
            .await
        {
            Ok(page) => saved.push(page),
            Err(e) => {
                cleanup_saved_pages(&lib, &saved).await;
                return Err(e);
            }
        }
    }

    let song_id = match insert_song_with_pages(&state, &meta, &saved, &collection_ids).await {
        Ok(id) => id,
        Err(e) => {
            cleanup_saved_pages(&lib, &saved).await;
            return Err(e);
        }
    };

    let song = sqlx::query("SELECT * FROM songs WHERE id = ?")
        .bind(song_id)
        .fetch_one(&state.pool)
        .await?;
    load_song_detail(&state.pool, crate::db::models::song_from_row(&song)).await
}

/// 读取本地图片为 dataURL（供前端 OCR 使用，绕开 asset 协议跨域限制）
#[tauri::command]
pub async fn read_image_base64(path: String) -> Result<String> {
    let src = PathBuf::from(&path);
    if !tokio::fs::try_exists(&src).await.unwrap_or(false) {
        return Err(AppError::msg(format!("文件不存在: {path}")));
    }
    let ext = file::extension_of(&src)?;
    let bytes = tokio::fs::read(&src).await?;
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    };
    use base64::engine::general_purpose::STANDARD as B64;
    Ok(format!(
        "data:{mime};base64,{}",
        B64.encode(bytes)
    ))
}

/// 查询曲谱列表（可按分组过滤、按标题/艺术家搜索）
#[tauri::command]
pub async fn get_songs(
    state: tauri::State<'_, AppState>,
    collection_id: Option<i64>,
    search: Option<String>,
) -> Result<Vec<SongDto>> {
    let rows = if let Some(cid) = collection_id {
        sqlx::query(
            "SELECT s.* FROM songs s
             JOIN song_collections sc ON sc.song_id = s.id
             WHERE sc.collection_id = ?
             ORDER BY s.updated_at DESC",
        )
        .bind(cid)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query("SELECT * FROM songs ORDER BY updated_at DESC")
            .fetch_all(&state.pool)
            .await?
    };

    let mut songs: Vec<SongDto> = Vec::new();
    let search = search.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    for row in rows {
        let dto = crate::db::models::song_from_row(&row);
        if let Some(q) = &search {
            let hit = dto.title.to_lowercase().contains(q)
                || dto.artist.to_lowercase().contains(q)
                || dto.tags.to_lowercase().contains(q);
            if !hit {
                continue;
            }
        }
        songs.push(dto);
    }

    let mut result = Vec::with_capacity(songs.len());
    for s in songs {
        result.push(load_song_detail(&state.pool, s).await?);
    }
    Ok(result)
}

/// 查询单首曲谱详情
#[tauri::command]
pub async fn get_song(state: tauri::State<'_, AppState>, id: i64) -> Result<SongDto> {
    let row = sqlx::query("SELECT * FROM songs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::msg("曲谱不存在"))?;
    load_song_detail(&state.pool, crate::db::models::song_from_row(&row)).await
}

/// 更新曲谱元数据与分组归属
#[tauri::command]
pub async fn update_song(
    state: tauri::State<'_, AppState>,
    id: i64,
    meta: SongMetaInput,
    collection_ids: Vec<i64>,
) -> Result<()> {
    let title = meta.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::msg("请填写歌名"));
    }
    let now = now_ts();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE songs SET title = ?, artist = ?, album = ?, tuning = ?, bpm = ?, tags = ?, updated_at = ? WHERE id = ?",
    )
    .bind(title)
    .bind(meta.artist.trim())
    .bind(meta.album.trim())
    .bind(meta.tuning.trim())
    .bind(meta.bpm)
    .bind(meta.tags.trim())
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM song_collections WHERE song_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for cid in collection_ids {
        sqlx::query("INSERT OR IGNORE INTO song_collections (song_id, collection_id) VALUES (?, ?)")
            .bind(id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 重排/删除分页：传入希望保留的全部分页 uuid（按新顺序）
#[tauri::command]
pub async fn set_song_pages(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    uuids: Vec<String>,
) -> Result<SongDto> {
    if uuids.is_empty() {
        return Err(AppError::msg("曲谱至少需要保留一页"));
    }
    let lib = get_library_path(&app).await?;

    let existing: Vec<PageDto> = sqlx::query("SELECT uuid, file_type, sort_order FROM pages WHERE song_id = ? ORDER BY sort_order ASC")
        .bind(id)
        .fetch_all(&state.pool)
        .await?
        .iter()
        .map(|r| PageDto {
            uuid: r.get("uuid"),
            file_type: r.get("file_type"),
            sort_order: r.get("sort_order"),
        })
        .collect();

    let mut tx = state.pool.begin().await?;
    let now = now_ts();
    for (i, uuid) in uuids.iter().enumerate() {
        sqlx::query("UPDATE pages SET sort_order = ? WHERE song_id = ? AND uuid = ?")
            .bind(i as i64)
            .bind(id)
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
    }
    // 删除被移除的页（连带物理文件）
    for page in &existing {
        if !uuids.contains(&page.uuid) {
            sqlx::query("DELETE FROM pages WHERE song_id = ? AND uuid = ?")
                .bind(id)
                .bind(&page.uuid)
                .execute(&mut *tx)
                .await?;
            let _ = file::remove_file_if_exists(&lib.join("Resources/Images").join(format!("{}.{}", page.uuid, page.file_type))).await;
            let _ = file::remove_file_if_exists(&lib.join("Resources/Thumbnails").join(format!("{}_thumb.jpg", page.uuid))).await;
        }
    }
    // 第一页作为封面
    let first = sqlx::query("SELECT uuid, file_type FROM pages WHERE song_id = ? ORDER BY sort_order ASC LIMIT 1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let first_uuid: String = first.get("uuid");
    let first_ext: String = first.get("file_type");
    sqlx::query("UPDATE songs SET uuid = ?, file_type = ?, updated_at = ? WHERE id = ?")
        .bind(&first_uuid)
        .bind(&first_ext)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let row = sqlx::query("SELECT * FROM songs WHERE id = ?").bind(id).fetch_one(&state.pool).await?;
    load_song_detail(&state.pool, crate::db::models::song_from_row(&row)).await
}

/// 为已有曲谱追加新的分页图片
#[tauri::command]
pub async fn add_pages_to_song(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    paths: Vec<String>,
) -> Result<SongDto> {
    if paths.is_empty() {
        return Err(AppError::msg("未选择任何图片文件"));
    }
    let lib = get_library_path(&app).await?;
    let mut saved: Vec<SavedPage> = Vec::new();
    for p in &paths {
        let src = PathBuf::from(p);
        match save_image_into_library(&lib, &src, None, None).await {
            Ok(page) => saved.push(page),
            Err(e) => {
                cleanup_saved_pages(&lib, &saved).await;
                return Err(e);
            }
        }
    }

    let max: Option<i64> = sqlx::query("SELECT MAX(sort_order) AS m FROM pages WHERE song_id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await?
        .get("m");
    let mut order = max.unwrap_or(-1) + 1;
    let now = now_ts();
    let mut tx = state.pool.begin().await?;
    for p in &saved {
        sqlx::query("INSERT INTO pages (song_id, uuid, file_type, sort_order, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(id)
            .bind(&p.uuid)
            .bind(&p.ext)
            .bind(order)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        order += 1;
    }
    sqlx::query("UPDATE songs SET updated_at = ? WHERE id = ?").bind(now).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;

    let row = sqlx::query("SELECT * FROM songs WHERE id = ?").bind(id).fetch_one(&state.pool).await?;
    load_song_detail(&state.pool, crate::db::models::song_from_row(&row)).await
}

/// 删除曲谱（连同分页图片与缩略图）
#[tauri::command]
pub async fn delete_song(app: AppHandle, state: tauri::State<'_, AppState>, id: i64) -> Result<()> {
    let lib = get_library_path(&app).await?;
    let pages: Vec<PageDto> = sqlx::query("SELECT uuid, file_type, sort_order FROM pages WHERE song_id = ?")
        .bind(id)
        .fetch_all(&state.pool)
        .await?
        .iter()
        .map(|r| PageDto {
            uuid: r.get("uuid"),
            file_type: r.get("file_type"),
            sort_order: r.get("sort_order"),
        })
        .collect();

    // 歌曲封面 uuid 对应的文件可能与 pages 不同（历史数据），也一并尝试清理
    let cover: Option<(String, String)> = sqlx::query("SELECT uuid, file_type FROM songs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .map(|r| (r.get("uuid"), r.get("file_type")));

    sqlx::query("DELETE FROM songs WHERE id = ?").bind(id).execute(&state.pool).await?;

    for p in &pages {
        let _ = file::remove_file_if_exists(&lib.join("Resources/Images").join(format!("{}.{}", p.uuid, p.file_type))).await;
        let _ = file::remove_file_if_exists(&lib.join("Resources/Thumbnails").join(format!("{}_thumb.jpg", p.uuid))).await;
    }
    if let Some((uuid, ext)) = cover {
        let _ = file::remove_file_if_exists(&lib.join("Resources/Images").join(format!("{uuid}.{ext}"))).await;
        let _ = file::remove_file_if_exists(&lib.join("Resources/Thumbnails").join(format!("{uuid}_thumb.jpg"))).await;
    }
    Ok(())
}
