use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

/// 曲谱分页 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDto {
    pub uuid: String,
    pub file_type: String,
    pub sort_order: i64,
}

/// 曲谱 DTO（含分页与所属分组）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongDto {
    pub id: i64,
    pub uuid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub tuning: String,
    pub bpm: Option<i64>,
    pub tags: String,
    pub file_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub pages: Vec<PageDto>,
    pub collection_ids: Vec<i64>,
}

/// 分组 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    pub id: i64,
    pub name: String,
    pub song_count: i64,
    pub created_at: i64,
}

/// 导入/编辑时的元数据输入
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SongMetaInput {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub tuning: String,
    pub bpm: Option<i64>,
    pub tags: String,
}

impl Default for SongMetaInput {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            tuning: "Standard".to_string(),
            bpm: None,
            tags: String::new(),
        }
    }
}

fn row_to_song(row: &SqliteRow) -> SongDto {
    SongDto {
        id: row.get("id"),
        uuid: row.get("uuid"),
        title: row.get("title"),
        artist: row.get("artist"),
        album: row.get("album"),
        tuning: row.get("tuning"),
        bpm: row.get("bpm"),
        tags: row.get("tags"),
        file_type: row.get("file_type"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        pages: Vec::new(),
        collection_ids: Vec::new(),
    }
}

/// 公开的行转换助手
pub fn song_from_row(row: &SqliteRow) -> SongDto {
    row_to_song(row)
}

/// 从数据库行加载一首曲谱的分页与分组关联
pub async fn load_song_detail(pool: &sqlx::SqlitePool, mut song: SongDto) -> crate::error::Result<SongDto> {
    let page_rows = sqlx::query(
        "SELECT uuid, file_type, sort_order FROM pages WHERE song_id = ? ORDER BY sort_order ASC",
    )
    .bind(song.id)
    .fetch_all(pool)
    .await?;

    song.pages = page_rows
        .iter()
        .map(|r| PageDto {
            uuid: r.get("uuid"),
            file_type: r.get("file_type"),
            sort_order: r.get("sort_order"),
        })
        .collect();

    let link_rows = sqlx::query("SELECT collection_id FROM song_collections WHERE song_id = ?")
        .bind(song.id)
        .fetch_all(pool)
        .await?;

    song.collection_ids = link_rows.iter().map(|r| r.get::<i64, _>("collection_id")).collect();

    Ok(song)
}
