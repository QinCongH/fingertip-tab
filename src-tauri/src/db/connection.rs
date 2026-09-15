use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

use crate::error::Result;

/// 应用共享状态：数据库连接池
pub struct AppState {
    pub pool: SqlitePool,
}

/// 打开（必要时创建）SQLite 数据库并运行建表脚本
pub async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}",
        db_path.to_string_lossy().replace('\\', "/")
    ))?
    .create_if_missing(true)
    .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::raw_sql(include_str!("../../migrations/init.sql"))
        .execute(&pool)
        .await?;

    // 幂等迁移：老库补充 v0.3.0 新增列（重复执行报 duplicate column，忽略）
    for stmt in [
        "ALTER TABLE songs ADD COLUMN format TEXT NOT NULL DEFAULT 'image'",
        "ALTER TABLE songs ADD COLUMN player_state TEXT",
    ] {
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    Ok(pool)
}
