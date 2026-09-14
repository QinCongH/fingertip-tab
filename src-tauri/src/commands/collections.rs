use crate::db::connection::AppState;
use crate::db::models::CollectionDto;
use crate::error::{AppError, Result};

use sqlx::Row;

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 获取全部分组（含每个分组的曲谱数量）
#[tauri::command]
pub async fn get_collections(state: tauri::State<'_, AppState>) -> Result<Vec<CollectionDto>> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.created_at, COUNT(sc.song_id) AS cnt
         FROM collections c
         LEFT JOIN song_collections sc ON sc.collection_id = c.id
         GROUP BY c.id, c.name, c.created_at
         ORDER BY c.created_at ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| CollectionDto {
            id: r.get("id"),
            name: r.get("name"),
            song_count: r.get::<i64, _>("cnt"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// 创建分组
#[tauri::command]
pub async fn create_collection(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<CollectionDto> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::msg("分组名称不能为空"));
    }
    let exists: Option<i64> = sqlx::query("SELECT id FROM collections WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await?
        .map(|r| r.get("id"));
    if exists.is_some() {
        return Err(AppError::msg(format!("分组「{name}」已存在")));
    }
    let id = sqlx::query("INSERT INTO collections (name, created_at) VALUES (?, ?)")
        .bind(&name)
        .bind(now_ts())
        .execute(&state.pool)
        .await?
        .last_insert_rowid();
    Ok(CollectionDto { id, name, song_count: 0, created_at: now_ts() })
}

/// 重命名分组
#[tauri::command]
pub async fn rename_collection(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<()> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::msg("分组名称不能为空"));
    }
    sqlx::query("UPDATE collections SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

/// 删除分组（关联关系级联删除，曲谱本身不受影响）
#[tauri::command]
pub async fn delete_collection(state: tauri::State<'_, AppState>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM collections WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}
