-- 指尖乐谱 数据库初始化脚本
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    tuning TEXT NOT NULL DEFAULT 'Standard',
    bpm INTEGER,
    tags TEXT NOT NULL DEFAULT '',
    file_type TEXT NOT NULL DEFAULT 'jpg',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_songs_title ON songs(title);
CREATE INDEX IF NOT EXISTS idx_songs_uuid ON songs(uuid);

CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

-- 谱子-分组 多对多关联表
CREATE TABLE IF NOT EXISTS song_collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    UNIQUE(song_id, collection_id)
);
CREATE INDEX IF NOT EXISTS idx_song_collections_song ON song_collections(song_id);
CREATE INDEX IF NOT EXISTS idx_song_collections_collection ON song_collections(collection_id);

-- 曲谱分页图片：一首谱子可以有多张图片（多页）
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    uuid TEXT NOT NULL,
    file_type TEXT NOT NULL DEFAULT 'jpg',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pages_song ON pages(song_id, sort_order);
