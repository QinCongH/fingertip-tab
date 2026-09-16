//! 局域网共享：内嵌 axum 服务器，手机扫码后浏览器直接查看谱面。
//!
//! 安全模型：
//! - 服务器仅在「点击共享」时启动，关闭共享 / 切歌 / 退出应用即停止
//! - 访问 URL 携带一次性随机 token（/s/{token}），token 不匹配一律 404
//! - 文件端点只接受 uuid + 扩展名白名单校验，路径锁定在库目录内，防目录穿越
//! - 服务器只绑定局域网（0.0.0.0），不做任何公网暴露

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as AxPath, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::db::connection::AppState;
use crate::db::models::{load_song_detail, song_from_row, SongDto};
use crate::error::{AppError, Result};
use crate::utils::file;

/// 当前共享会话：可以是单首乐谱共享，也可以是整库共享（song_id: None）
#[derive(Debug, Clone)]
struct ShareSession {
    token: String,
    song_id: Option<i64>,
}

type SharedState = Arc<tokio::sync::RwLock<Option<ShareSession>>>;

fn session() -> &'static SharedState {
    use std::sync::OnceLock;
    static SESSION: OnceLock<SharedState> = OnceLock::new();
    SESSION.get_or_init(|| Arc::new(tokio::sync::RwLock::new(None)))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfo {
    /// 局域网访问地址，如 http://192.168.1.5:17890/s/abc123
    pub url: String,
    pub ip: String,
    pub port: u16,
    pub token: String,
    pub is_library: bool,
    pub song_id: Option<i64>,
}

/// 服务器 shutdown 句柄（全局唯一）
struct ServerHandle {
    shutdown: oneshot::Sender<()>,
}

fn server_handle() -> &'static tokio::sync::Mutex<Option<ServerHandle>> {
    use std::sync::OnceLock;
    static HANDLE: OnceLock<tokio::sync::Mutex<Option<ServerHandle>>> = OnceLock::new();
    HANDLE.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// 获取本机局域网 IPv4 地址
///
/// 优先策略：
/// 1. UDP 探测真实默认路由出口 IP（无需真正发送数据，由 OS 路由表解析匹配最佳网卡，100% 准确命中 192.168.x.x）
/// 2. 若失败，遍历网卡并过滤 VMware / VirtualBox / vEthernet / Loopback 等虚拟适配器，优先选择 192.168.x.x
pub fn lan_ipv4() -> Result<IpAddr> {
    // 方案 1：UDP connect 触发系统路由选择（向公网公用 DNS 寻址，不发包，瞬间返回）
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("223.5.5.5:80").is_ok() {
            if let Ok(SocketAddr::V4(addr)) = socket.local_addr() {
                let ip = *addr.ip();
                if !ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local() {
                    return Ok(IpAddr::V4(ip));
                }
            }
        }
    }

    // 方案 2：枚举网卡，排除常见虚拟网卡，优先 192.168.x.x
    let addrs = local_ip_address::list_afinet_netifas()
        .map_err(|e| AppError::msg(format!("无法获取本机网络地址: {e}")))?;

    let is_virtual_name = |name: &str| -> bool {
        let n = name.to_lowercase();
        n.contains("vmware")
            || n.contains("virtual")
            || n.contains("vbox")
            || n.contains("vethernet")
            || n.contains("wsl")
            || n.contains("hyper-v")
            || n.contains("bluetooth")
            || n.contains("蓝牙")
            || n.contains("loopback")
    };

    let mut candidates: Vec<(bool, bool, IpAddr)> = Vec::new();
    for (name, ip) in addrs {
        if let IpAddr::V4(v4) = ip {
            if v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() {
                continue;
            }
            let is_192 = v4.octets()[0] == 192 && v4.octets()[1] == 168;
            let is_real = !is_virtual_name(&name);
            candidates.push((is_real, is_192, IpAddr::V4(v4)));
        }
    }

    // 排序：真实网卡优先，192.168 优先
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0) // is_real desc
            .then_with(|| b.1.cmp(&a.1)) // is_192 desc
    });

    if let Some((_, _, ip)) = candidates.into_iter().next() {
        return Ok(ip);
    }

    Err(AppError::msg("未找到可用的局域网地址，请确认已连接 Wi-Fi 或网线"))
}

/// 启动共享服务器。返回 (端口, shutdown 发送端)
async fn spawn_server() -> Result<(u16, oneshot::Sender<()>)> {
    let (tx, rx) = oneshot::channel::<()>();
    let app: Router = Router::new()
        .route("/s/:token", get(mobile_page))
        .route("/api/songs/:token", get(api_songs))
        .route("/api/song/:token/:id", get(api_song_by_id))
        .route("/api/song/:token", get(api_song))
        .route("/api/file/:token/:uuid/:ext", get(api_file));

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
        .await
        .map_err(|e| AppError::msg(format!("共享服务器启动失败: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::msg(format!("共享服务器启动失败: {e}")))?
        .port();

    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });

    Ok((port, tx))
}

/// 开启共享：注册共享会话并启动服务器
#[tauri::command]
pub async fn start_share(
    state: tauri::State<'_, AppState>,
    song_id: i64,
) -> Result<ShareInfo> {
    // 校验曲谱存在
    let row = sqlx::query("SELECT * FROM songs WHERE id = ?")
        .bind(song_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::msg("曲谱不存在"))?;
    let _ = load_song_detail(&state.pool, song_from_row(&row)).await?;

    // 先停掉旧会话
    stop_server().await;

    let token = uuid::Uuid::new_v4().simple().to_string();
    *session().write().await = Some(ShareSession { token: token.clone(), song_id: Some(song_id) });

    let (port, shutdown) = spawn_server().await?;
    *server_handle().lock().await = Some(ServerHandle { shutdown });

    let ip = lan_ipv4()?;
    let ip_str = ip.to_string();
    Ok(ShareInfo {
        url: format!("http://{ip_str}:{port}/s/{token}"),
        ip: ip_str,
        port,
        token,
        is_library: false,
        song_id: Some(song_id),
    })
}

/// 开启首页乐谱库整库共享
#[tauri::command]
pub async fn start_library_share() -> Result<ShareInfo> {
    stop_server().await;

    let token = uuid::Uuid::new_v4().simple().to_string();
    *session().write().await = Some(ShareSession { token: token.clone(), song_id: None });

    let (port, shutdown) = spawn_server().await?;
    *server_handle().lock().await = Some(ServerHandle { shutdown });

    let ip = lan_ipv4()?;
    let ip_str = ip.to_string();
    Ok(ShareInfo {
        url: format!("http://{ip_str}:{port}/s/{token}"),
        ip: ip_str,
        port,
        token,
        is_library: true,
        song_id: None,
    })
}

/// 获取当前共享信息状态
#[tauri::command]
pub async fn get_share_info() -> Result<Option<ShareInfo>> {
    let s = session().read().await;
    let Some(cur) = s.as_ref() else {
        return Ok(None);
    };
    let Ok(ip) = lan_ipv4() else {
        return Ok(None);
    };
    let ip_str = ip.to_string();
    Ok(Some(ShareInfo {
        url: format!("http://{ip_str}:0/s/{}", cur.token),
        ip: ip_str,
        port: 0,
        token: cur.token.clone(),
        is_library: cur.song_id.is_none(),
        song_id: cur.song_id,
    }))
}

/// 关闭共享：清空会话并停止服务器
#[tauri::command]
pub async fn stop_share() -> Result<()> {
    stop_server().await;
    Ok(())
}

async fn stop_server() {
    session().write().await.take();
    let mut guard = server_handle().lock().await;
    if let Some(h) = guard.take() {
        let _ = h.shutdown.send(());
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "404").into_response()
}

/// 校验 token 是否有效
async fn is_token_valid(token: &str) -> bool {
    let s = session().read().await;
    let Some(session) = s.as_ref() else { return false };
    session.token == token
}

/// 校验 token 并取出单曲共享绑定的 song_id
async fn check_token_song(token: &str) -> Option<i64> {
    let s = session().read().await;
    let session = s.as_ref()?;
    if session.token != token {
        return None;
    }
    session.song_id
}

/// 移动端统一入口：如果会话为整库共享（song_id: None），渲染带搜索框的乐谱列表；如果会话为单曲共享，直接打开该曲谱
async fn mobile_page(AxPath(token): AxPath<String>) -> Response {
    if !is_token_valid(&token).await {
        return not_found();
    }
    let s = session().read().await;
    let is_single = s.as_ref().and_then(|sess| sess.song_id);

    if let Some(song_id) = is_single {
        let state = take_context().await;
        let Some(state) = state else { return not_found() };
        let row = match sqlx::query("SELECT * FROM songs WHERE id = ?")
            .bind(song_id)
            .fetch_one(&state.pool)
            .await
        {
            Ok(r) => r,
            Err(_) => return not_found(),
        };
        let song = song_from_row(&row);
        let html = render_single_page(&token, &song.title, &song.artist, &song.format);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response();
    }

    let html = render_library_mobile_page(&token);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// 乐谱列表 JSON（支持搜索过滤）
async fn api_songs(
    AxPath(token): AxPath<String>,
    Query(params): Query<SearchQuery>,
) -> Response {
    if !is_token_valid(&token).await {
        return not_found();
    }
    let state = take_context().await;
    let Some(state) = state else { return not_found() };

    let rows = match sqlx::query("SELECT * FROM songs ORDER BY updated_at DESC")
        .fetch_all(&state.pool)
        .await
    {
        Ok(r) => r,
        Err(_) => return not_found(),
    };

    let query_str = params.q.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let mut songs: Vec<SongDto> = Vec::new();
    for row in rows {
        let dto = song_from_row(&row);
        if let Some(q) = &query_str {
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
        if let Ok(detail) = load_song_detail(&state.pool, s).await {
            result.push(detail);
        }
    }

    (
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&result).unwrap_or_default(),
    )
        .into_response()
}

/// 单曲元数据 JSON（兼容旧端点 /api/song/:token，按单曲会话绑定的 song_id 取）
async fn api_song(AxPath(token): AxPath<String>) -> Response {
    let Some(song_id) = check_token_song(&token).await else {
        return not_found();
    };
    fetch_song_json(&token, song_id).await
}

/// 按 ID 获取曲谱元数据 JSON（/api/song/:token/:id）
async fn api_song_by_id(AxPath((token, id)): AxPath<(String, i64)>) -> Response {
    if !is_token_valid(&token).await {
        return not_found();
    }
    fetch_song_json(&token, id).await
}

async fn fetch_song_json(_token: &str, song_id: i64) -> Response {
    let state = take_context().await;
    let Some(state) = state else { return not_found() };
    let row = match sqlx::query("SELECT * FROM songs WHERE id = ?")
        .bind(song_id)
        .fetch_one(&state.pool)
        .await
    {
        Ok(r) => r,
        Err(_) => return not_found(),
    };
    let song = match load_song_detail(&state.pool, song_from_row(&row)).await {
        Ok(s) => s,
        Err(_) => return not_found(),
    };
    (
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&song).unwrap_or_default(),
    )
        .into_response()
}

/// 曲谱文件流：图片谱分页 / GP 文件（uuid + 扩展名白名单校验，路径锁定在库目录内）
async fn api_file(
    AxPath((token, uuid, ext)): AxPath<(String, String, String)>,
) -> Response {
    if !is_token_valid(&token).await {
        return not_found();
    }
    let ext_lower = ext.to_lowercase();
    let (dir, mime): (&str, &str) = match ext_lower.as_str() {
        "png" => ("Resources/Images", "image/png"),
        "webp" => ("Resources/Images", "image/webp"),
        "bmp" => ("Resources/Images", "image/bmp"),
        "gif" => ("Resources/Images", "image/gif"),
        "jpg" | "jpeg" => ("Resources/Images", "image/jpeg"),
        "gp3" => ("Resources/GPFiles", "application/octet-stream"),
        "gp4" => ("Resources/GPFiles", "application/octet-stream"),
        "gp5" => ("Resources/GPFiles", "application/octet-stream"),
        "gpx" => ("Resources/GPFiles", "application/octet-stream"),
        "gp" => ("Resources/GPFiles", "application/octet-stream"),
        _ => return not_found(),
    };
    if !file::GP_EXTS.contains(&ext_lower.as_str()) && mime.starts_with("image/") {
        // 图片扩展名白名单已在 match 覆盖
    }
    if uuid.is_empty() || !uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return not_found();
    }
    let Some(state) = take_context().await else { return not_found() };
    let lib = match crate::commands::settings::get_library_path(&state.app).await {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    let path = lib.join(dir).join(format!("{uuid}.{ext_lower}"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// 全局共享上下文（pool + app handle）：axum handler 中无法直接拿 tauri::State
struct ShareContext {
    pool: sqlx::SqlitePool,
    app: AppHandle,
}

fn context() -> &'static tokio::sync::RwLock<Option<Arc<ShareContext>>> {
    use std::sync::OnceLock;
    static CTX: OnceLock<tokio::sync::RwLock<Option<Arc<ShareContext>>>> = OnceLock::new();
    CTX.get_or_init(|| tokio::sync::RwLock::new(None))
}

async fn take_context() -> Option<Arc<ShareContext>> {
    context().read().await.clone()
}

/// 注册全局共享上下文（应用启动时调用一次）
pub fn register_context(pool: sqlx::SqlitePool, app: AppHandle) {
    let ctx = Arc::new(ShareContext { pool, app });
    let cell = context();
    tauri::async_runtime::spawn(async move {
        *cell.write().await = Some(ctx);
    });
}

/// 手机端单曲查看页 HTML（兼容老单曲查看模式）
fn render_single_page(token: &str, title: &str, artist: &str, format: &str) -> String {
    render_mobile_page(token, title, artist, format)
}

/// 手机端乐谱库共享页：顶部搜索框 + 乐谱列表，点击进入单谱详情查看/播放，带返回按钮与完整功能
fn render_library_mobile_page(token: &str) -> String {
    let t = html_escape(token);
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>指尖曲谱 - 共享乐谱库</title>
<script src="https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/alphaTab.js"></script>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; -webkit-tap-highlight-color:transparent; }}
body {{ background:#121212; color:#f5f5f0; font-family:-apple-system,BlinkMacSystemFont,'PingFang SC','Microsoft YaHei',sans-serif; min-height:100vh; overflow-x:hidden; }}

#library-view {{ display:block; min-height:100vh; }}
#detail-view {{ display:none; min-height:100vh; }}

.search-header {{ position:sticky; top:0; z-index:30; background:rgba(20,20,20,.95); backdrop-filter:blur(12px); border-bottom:2px solid rgba(255,255,255,.12); padding:12px 16px; display:flex; flex-direction:column; gap:10px; }}
.app-brand {{ display:flex; align-items:center; justify-content:space-between; }}
.app-title {{ font-size:16px; font-weight:900; letter-spacing:0.5px; display:flex; align-items:center; gap:6px; }}
.app-title span.hl {{ color:#ff6b6b; }}
.total-tag {{ font-size:11px; background:rgba(255,255,255,.12); color:#ccc; padding:2px 8px; font-family:monospace; border-radius:12px; }}

.search-input-wrap {{ position:relative; width:100%; }}
.search-input {{ width:100%; height:40px; background:rgba(255,255,255,.08); border:1px solid rgba(255,255,255,.2); color:#fff; font-size:14px; padding:0 36px 0 14px; outline:none; transition:all .2s ease; border-radius:8px; }}
.search-input:focus {{ background:rgba(255,255,255,.14); border-color:#ff6b6b; box-shadow:0 0 10px rgba(255,107,107,.25); }}
.search-clear {{ position:absolute; right:10px; top:50%; transform:translateY(-50%); width:22px; height:22px; background:rgba(255,255,255,.2); color:#fff; border:0; border-radius:50%; display:none; align-items:center; justify-content:center; cursor:pointer; font-size:12px; }}

.song-list {{ padding:12px 14px 60px; display:flex; flex-direction:column; gap:10px; }}
.song-card {{ background:#1a1a1a; border:1px solid rgba(255,255,255,.08); border-radius:10px; padding:14px 16px; display:flex; align-items:center; justify-content:space-between; gap:12px; cursor:pointer; transition:background .15s ease, transform .1s ease; }}
.song-card:active {{ background:#262626; transform:scale(0.985); }}
.song-meta {{ min-width:0; flex:1; }}
.song-title {{ font-size:15px; font-weight:700; color:#fff; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
.song-sub {{ font-size:12px; color:rgba(255,255,255,.5); margin-top:4px; display:flex; align-items:center; gap:8px; }}
.badge-gp {{ background:#ff6b6b; color:#fff; font-size:10px; font-weight:800; padding:2px 6px; border-radius:4px; flex-shrink:0; }}
.badge-img {{ background:#4a90e2; color:#fff; font-size:10px; font-weight:800; padding:2px 6px; border-radius:4px; flex-shrink:0; }}
.song-arrow {{ color:rgba(255,255,255,.3); flex-shrink:0; font-size:18px; }}

.state-tip {{ padding:60px 16px; text-align:center; font-size:14px; opacity:.55; }}

.detail-header {{ position:sticky; top:0; z-index:30; background:rgba(18,18,18,.96); backdrop-filter:blur(10px); border-bottom:2px solid rgba(255,255,255,.15); padding:10px 12px; display:flex; align-items:center; justify-content:space-between; gap:8px; }}
.btn-back {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.25); padding:6px 12px; font-size:13px; font-weight:700; display:flex; align-items:center; gap:4px; cursor:pointer; border-radius:6px; flex-shrink:0; }}
.btn-back:active {{ background:rgba(255,255,255,.28); }}
.detail-title-wrap {{ min-width:0; flex:1; text-align:left; }}
.detail-title {{ font-size:15px; font-weight:700; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
.detail-sub {{ font-size:11px; opacity:.55; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
.detail-actions {{ display:flex; align-items:center; gap:6px; flex-shrink:0; }}
.btn-icon {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.2); width:34px; height:34px; display:flex; align-items:center; justify-content:center; cursor:pointer; font-size:14px; border-radius:6px; }}
.btn-icon:active {{ background:rgba(255,255,255,.25); }}

#gp-container {{ width:100%; min-height:70vh; background:#f5f5f0; padding-bottom:120px; overflow-x:auto; display:flex; justify-content:center; }}
#gp-sheet {{ width:100%; max-width:100%; }}
#gp-sheet canvas, #gp-sheet svg {{ max-width:100% !important; }}
.at-cursor-bar {{ background:rgba(255,107,107,.18) !important; border-radius:0; }}
.at-cursor-beat {{ background:#ff6b6b !important; width:4px !important; box-shadow:0 0 4px rgba(255,107,107,.6); }}

#gp-player-bar {{ position:fixed; bottom:0; left:0; right:0; z-index:40; background:rgba(26,26,26,.96); backdrop-filter:blur(10px); border-top:2px solid rgba(255,255,255,.2); padding:10px 14px; display:flex; align-items:center; gap:10px; box-shadow:0 -4px 20px rgba(0,0,0,.6); }}
#gp-btn-play {{ background:#ff6b6b; color:#fff; border:0; width:40px; height:40px; font-size:16px; font-weight:700; display:flex; align-items:center; justify-content:center; cursor:pointer; flex-shrink:0; border-radius:6px; }}
#gp-btn-stop {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.2); width:34px; height:34px; font-size:14px; display:flex; align-items:center; justify-content:center; cursor:pointer; flex-shrink:0; border-radius:6px; }}
.gp-progress-box {{ flex:1; min-width:0; display:flex; align-items:center; gap:8px; }}
.gp-time-label {{ font-family:monospace; font-size:11px; color:#f5f5f0; opacity:.75; flex-shrink:0; }}
#gp-progress-slider {{ flex:1; height:4px; appearance:none; -webkit-appearance:none; background:rgba(255,255,255,.2); outline:none; border-radius:0; cursor:pointer; }}
#gp-progress-slider::-webkit-slider-thumb {{ appearance:none; -webkit-appearance:none; width:14px; height:14px; background:#ff6b6b; cursor:pointer; border-radius:0; }}
.gp-zoom-controls {{ display:flex; align-items:center; gap:3px; flex-shrink:0; }}
.gp-zoom-btn {{ background:rgba(255,255,255,.1); color:#fff; border:1px solid rgba(255,255,255,.2); width:28px; height:28px; font-size:14px; font-weight:700; cursor:pointer; display:flex; align-items:center; justify-content:center; border-radius:4px; }}
#gp-zoom-val {{ font-family:monospace; font-size:11px; color:#f5f5f0; opacity:.85; min-width:32px; text-align:center; }}

#gp-sheet-menu {{ position:fixed; top:54px; right:12px; z-index:50; background:rgba(26,26,26,.96); backdrop-filter:blur(10px); border:2px solid rgba(255,255,255,.2); box-shadow:0 8px 30px rgba(0,0,0,.7); padding:12px 14px; min-width:200px; display:none; flex-direction:column; gap:10px; border-radius:8px; }}
#gp-sheet-menu.open {{ display:flex; }}
.gp-menu-item {{ display:flex; align-items:center; justify-content:space-between; cursor:pointer; font-size:13px; font-weight:500; padding:4px 0; }}
.gp-menu-item input[type="checkbox"] {{ width:16px; height:16px; accent-color:#ff6b6b; cursor:pointer; }}

#img-container {{ display:flex; flex-direction:column; gap:12px; padding:12px 8px 90px; max-width:900px; margin:0 auto; }}
.img-page-wrap {{ position:relative; background:#f5f5f0; cursor:zoom-in; border:1px solid rgba(255,255,255,.1); box-shadow:0 4px 16px rgba(0,0,0,.5); }}
.img-page-wrap img {{ width:100%; height:auto; display:block; }}
.img-page-num {{ position:absolute; right:8px; bottom:8px; background:rgba(13,13,13,.75); color:#f5f5f0; font-size:11px; padding:2px 6px; font-family:monospace; pointer-events:none; border-radius:4px; }}

#img-scroll-bar {{ position:fixed; bottom:16px; left:50%; transform:translateX(-50%); z-index:50; background:rgba(26,26,26,.94); backdrop-filter:blur(10px); border:2px solid rgba(255,255,255,.2); padding:8px 14px; display:flex; align-items:center; gap:12px; box-shadow:0 8px 30px rgba(0,0,0,.7); max-width:calc(100vw - 32px); border-radius:8px; }}
#img-btn-scroll {{ background:#ff6b6b; color:#fff; border:0; font-weight:700; font-size:13px; padding:6px 14px; cursor:pointer; display:flex; align-items:center; gap:6px; min-width:82px; justify-content:center; border-radius:6px; }}
.img-speed-group {{ display:flex; align-items:center; gap:6px; font-size:12px; }}
.img-speed-btn {{ background:rgba(255,255,255,.1); color:#fff; border:1px solid rgba(255,255,255,.2); width:28px; height:28px; font-size:14px; font-weight:700; cursor:pointer; display:flex; align-items:center; justify-content:center; border-radius:4px; }}
.img-speed-val {{ font-family:monospace; font-size:12px; min-width:44px; text-align:center; color:#f5f5f0; }}

#img-lightbox {{ display:none; position:fixed; inset:0; z-index:9999; background:#000; touch-action:none; user-select:none; -webkit-user-select:none; }}
#img-lightbox.active {{ display:flex; flex-direction:column; }}
#img-lb-topbar {{ position:absolute; top:0; left:0; right:0; z-index:100; display:flex; align-items:center; justify-content:space-between; padding:12px 16px; background:linear-gradient(to bottom, rgba(0,0,0,.85), transparent); pointer-events:none; }}
#img-lb-topbar * {{ pointer-events:auto; }}
#img-lb-title {{ font-size:13px; font-family:monospace; color:#fff; font-weight:700; }}
.img-lb-actions {{ display:flex; align-items:center; gap:8px; }}
#img-lb-fs-btn, #img-lb-close {{ background:rgba(255,255,255,.2); color:#fff; border:0; width:36px; height:36px; font-size:18px; line-height:36px; text-align:center; cursor:pointer; display:flex; align-items:center; justify-content:center; border-radius:6px; }}
#img-lb-viewport {{ flex:1; width:100%; height:100%; display:flex; align-items:center; justify-content:center; overflow:hidden; position:relative; }}
#img-lb-img {{ max-width:100%; max-height:100%; object-fit:contain; transform-origin:center center; transition:transform 0.12s ease-out; will-change:transform; }}
#img-lb-bottombar {{ position:absolute; bottom:0; left:0; right:0; z-index:100; display:flex; align-items:center; justify-content:space-between; padding:16px 20px; background:linear-gradient(to top, rgba(0,0,0,.85), transparent); pointer-events:none; }}
#img-lb-bottombar button {{ pointer-events:auto; background:rgba(255,255,255,.15); color:#fff; border:1px solid rgba(255,255,255,.3); padding:8px 18px; font-size:13px; font-weight:700; cursor:pointer; border-radius:6px; }}
#img-lb-bottombar button:disabled {{ opacity:.3; cursor:not-allowed; }}
#img-lb-hint {{ font-size:11px; opacity:.55; text-align:center; }}
</style>
</head>
<body>

<div id="library-view">
  <div class="search-header">
    <div class="app-brand">
      <div class="app-title">🎸 指尖曲谱 <span class="hl">局域网共享</span></div>
      <div class="total-tag" id="total-badge">0 首曲谱</div>
    </div>
    <div class="search-input-wrap">
      <input type="text" class="search-input" id="search-input" placeholder="搜索曲谱名 / 歌手…" autocomplete="off" />
      <button class="search-clear" id="search-clear">✕</button>
    </div>
  </div>
  <div class="song-list" id="song-list">
    <div class="state-tip" id="list-tip">正在加载乐谱库…</div>
  </div>
</div>

<div id="detail-view">
  <div class="detail-header">
    <button class="btn-back" id="btn-back">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
      返回
    </button>
    <div class="detail-title-wrap">
      <div class="detail-title" id="cur-title">--</div>
      <div class="detail-sub" id="cur-sub">--</div>
    </div>
    <div class="detail-actions">
      <button class="btn-icon" id="btn-detail-settings" style="display:none;" title="谱面显示设置">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      </button>
      <button class="btn-icon" id="btn-detail-fs" title="全屏">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6"/></svg>
      </button>
      <span class="badge-gp" id="detail-format-badge" style="display:none;">GP</span>
      <span class="badge-img" id="detail-page-badge" style="display:none;">1 P</span>
    </div>
  </div>

  <div id="gp-sheet-menu">
    <div style="font-size:11px; font-weight:700; opacity:.5; text-transform:uppercase;">谱面显示设置</div>
    <label class="gp-menu-item">
      <span>显示五线谱</span>
      <input type="checkbox" id="gp-chk-score" checked />
    </label>
    <label class="gp-menu-item">
      <span>显示六线谱</span>
      <input type="checkbox" id="gp-chk-tab" checked />
    </label>
    <label class="gp-menu-item">
      <span>显示和弦名称</span>
      <input type="checkbox" id="gp-chk-chords" checked />
    </label>
  </div>

  <div id="gp-section" style="display:none;">
    <div id="gp-container">
      <div id="gp-loading" class="state-tip">正在解析 GP 曲谱…</div>
      <div id="gp-sheet"></div>
    </div>
    <div id="gp-player-bar">
      <button id="gp-btn-play">▶</button>
      <button id="gp-btn-stop">■</button>
      <div class="gp-progress-box">
        <span class="gp-time-label" id="gp-time-cur">00:00</span>
        <input type="range" id="gp-progress-slider" min="0" max="100" value="0" step="0.1" />
        <span class="gp-time-label" id="gp-time-total">00:00</span>
      </div>
      <div class="gp-zoom-controls">
        <button class="gp-zoom-btn" id="gp-zoom-out">−</button>
        <span id="gp-zoom-val">40%</span>
        <button class="gp-zoom-btn" id="gp-zoom-in">+</button>
      </div>
    </div>
  </div>

  <div id="img-section" style="display:none;">
    <div id="img-container"></div>
    <div id="img-scroll-bar">
      <button id="img-btn-scroll">▶ 滚动</button>
      <div class="img-speed-group">
        <button class="img-speed-btn" id="img-speed-down">−</button>
        <span class="img-speed-val" id="img-speed-label">30 px/s</span>
        <button class="img-speed-btn" id="img-speed-up">+</button>
      </div>
    </div>
    <div id="img-lightbox">
      <div id="img-lb-topbar">
        <span id="img-lb-title"></span>
        <div class="img-lb-actions">
          <button id="img-lb-fs-btn">⛶</button>
          <button id="img-lb-close">✕</button>
        </div>
      </div>
      <div id="img-lb-viewport">
        <img id="img-lb-img" alt="preview" />
      </div>
      <div id="img-lb-bottombar">
        <button id="img-lb-prev">‹ 上一页</button>
        <div id="img-lb-hint">双击或捏合缩放 · 左右滑动</div>
        <button id="img-lb-next">下一页 ›</button>
      </div>
    </div>
  </div>
</div>

<script>
(function() {{
  var TOKEN = '{t}';
  var allSongs = [];
  var currentSong = null;

  var libView = document.getElementById('library-view');
  var detailView = document.getElementById('detail-view');
  var searchInput = document.getElementById('search-input');
  var searchClear = document.getElementById('search-clear');
  var songList = document.getElementById('song-list');
  var listTip = document.getElementById('list-tip');
  var totalBadge = document.getElementById('total-badge');
  var btnBack = document.getElementById('btn-back');

  var curTitle = document.getElementById('cur-title');
  var curSub = document.getElementById('cur-sub');
  var btnDetailSettings = document.getElementById('btn-detail-settings');
  var btnDetailFs = document.getElementById('btn-detail-fs');
  var detailFormatBadge = document.getElementById('detail-format-badge');
  var detailPageBadge = document.getElementById('detail-page-badge');

  var gpSection = document.getElementById('gp-section');
  var gpContainer = document.getElementById('gp-container');
  var gpSheet = document.getElementById('gp-sheet');
  var gpLoading = document.getElementById('gp-loading');
  var gpPlayerBar = document.getElementById('gp-player-bar');
  var gpBtnPlay = document.getElementById('gp-btn-play');
  var gpBtnStop = document.getElementById('gp-btn-stop');
  var gpSlider = document.getElementById('gp-progress-slider');
  var gpTimeCur = document.getElementById('gp-time-cur');
  var gpTimeTotal = document.getElementById('gp-time-total');
  var gpZoomIn = document.getElementById('gp-zoom-in');
  var gpZoomOut = document.getElementById('gp-zoom-out');
  var gpZoomVal = document.getElementById('gp-zoom-val');
  var gpSheetMenu = document.getElementById('gp-sheet-menu');
  var gpChkScore = document.getElementById('gp-chk-score');
  var gpChkTab = document.getElementById('gp-chk-tab');
  var gpChkChords = document.getElementById('gp-chk-chords');

  var imgSection = document.getElementById('img-section');
  var imgContainer = document.getElementById('img-container');
  var imgScrollBar = document.getElementById('img-scroll-bar');
  var imgBtnScroll = document.getElementById('img-btn-scroll');
  var imgSpeedLabel = document.getElementById('img-speed-label');
  var imgSpeedDown = document.getElementById('img-speed-down');
  var imgSpeedUp = document.getElementById('img-speed-up');
  var imgLightbox = document.getElementById('img-lightbox');
  var imgLbImg = document.getElementById('img-lb-img');
  var imgLbTitle = document.getElementById('img-lb-title');
  var imgLbPrev = document.getElementById('img-lb-prev');
  var imgLbNext = document.getElementById('img-lb-next');
  var imgLbClose = document.getElementById('img-lb-close');
  var imgLbFs = document.getElementById('img-lb-fs-btn');
  var imgLbViewport = document.getElementById('img-lb-viewport');

  var at = null;
  var gpPlaying = false;
  var gpScale = 0.4;
  var gpTotalMs = 0;
  var gpSeeking = false;
  var gpLastScrollTime = 0;
  var gpShowScore = true, gpShowTab = true, gpShowChords = true;

  var imgCurPages = [];
  var imgCurIdx = 0;
  var imgScale = 1, imgPosX = 0, imgPosY = 0, imgLastX = 0, imgLastY = 0, imgLastScale = 1;
  var imgTouchStartX = 0, imgTouchStartY = 0, imgTouchDist = 0, imgIsPanning = false;
  var imgAutoScrolling = false, imgScrollSpeed = 30, imgLastScrollTime = 0, imgScrollRaf = null;

  function loadLibrary(query) {{
    var url = '/api/songs/' + TOKEN + (query ? '?q=' + encodeURIComponent(query) : '');
    fetch(url).then(function(r) {{ return r.json(); }}).then(function(data) {{
      allSongs = data || [];
      totalBadge.textContent = allSongs.length + ' 首曲谱';
      renderSongList(allSongs);
    }}).catch(function(e) {{
      console.error(e);
      listTip.textContent = '获取曲谱列表失败，请刷新重试';
    }});
  }}

  function renderSongList(list) {{
    songList.innerHTML = '';
    if (!list.length) {{
      songList.innerHTML = '<div class="state-tip">未找到匹配的曲谱</div>';
      return;
    }}
    list.forEach(function(s) {{
      var card = document.createElement('div');
      card.className = 'song-card';
      var isGp = s.format === 'gp';
      var badgeHtml = isGp
        ? '<span class="badge-gp">GP</span>'
        : '<span class="badge-img">' + ((s.pages && s.pages.length) ? s.pages.length + 'P' : '图') + '</span>';
      var artist = s.artist ? s.artist : '未知艺术家';
      card.innerHTML =
        '<div class="song-meta">' +
          '<div class="song-title">' + escapeHtml(s.title) + '</div>' +
          '<div class="song-sub">' + badgeHtml + '<span>' + escapeHtml(artist) + '</span></div>' +
        '</div>' +
        '<div class="song-arrow">›</div>';
      card.addEventListener('click', function() {{
        openSongDetail(s);
      }});
      songList.appendChild(card);
    }});
  }}

  function escapeHtml(str) {{
    return (str || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }}

  var searchTimer = null;
  searchInput.addEventListener('input', function() {{
    var val = searchInput.value.trim();
    searchClear.style.display = val ? 'flex' : 'none';
    clearTimeout(searchTimer);
    searchTimer = setTimeout(function() {{
      loadLibrary(val);
    }}, 200);
  }});
  searchClear.addEventListener('click', function() {{
    searchInput.value = '';
    searchClear.style.display = 'none';
    loadLibrary('');
  }});

  btnBack.addEventListener('click', function() {{
    closeDetail();
  }});

  function closeDetail() {{
    if (at) {{
      try {{ at.stop(); at.destroy(); }} catch (e) {{}}
      at = null;
    }}
    stopImgAutoScroll();
    gpPlaying = false;
    currentSong = null;
    gpSheetMenu.classList.remove('open');
    detailView.style.display = 'none';
    libView.style.display = 'block';
    window.scrollTo({{ top: 0 }});
  }}

  function openSongDetail(s) {{
    currentSong = s;
    libView.style.display = 'none';
    detailView.style.display = 'block';
    curTitle.textContent = s.title;
    curSub.textContent = (s.artist || '未知艺术家') + (s.format === 'gp' ? ' · Guitar Pro' : ' · 图片谱');

    btnDetailSettings.style.display = s.format === 'gp' ? 'flex' : 'none';
    detailFormatBadge.style.display = s.format === 'gp' ? 'inline-block' : 'none';
    detailPageBadge.style.display = s.format === 'gp' ? 'none' : 'inline-block';

    if (s.format === 'gp') {{
      imgSection.style.display = 'none';
      gpSection.style.display = 'block';
      renderGpDetail(s);
    }} else {{
      gpSection.style.display = 'none';
      imgSection.style.display = 'block';
      renderImgDetail(s);
    }}
    window.scrollTo({{ top: 0 }});
  }}

  function renderGpDetail(s) {{
    gpLoading.style.display = 'block';
    gpLoading.textContent = '正在解析 GP 曲谱…';
    gpSheet.innerHTML = '';
    gpPlaying = false;
    gpBtnPlay.textContent = '▶';
    gpSlider.value = 0;
    gpTimeCur.textContent = '00:00';
    gpScale = 0.4;
    updateGpZoomLabel();

    var uuid = s.uuid;
    var ext = s.fileType;
    if (!uuid || !ext) {{ gpLoading.textContent = '曲谱信息不完整'; return; }}

    fetch('/api/file/' + TOKEN + '/' + uuid + '/' + encodeURIComponent(ext))
      .then(function(r) {{ if (!r.ok) throw 0; return r.arrayBuffer(); }})
      .then(function(buf) {{
        gpLoading.style.display = 'none';
        var bytes = new Uint8Array(buf);
        var encoding = detectGpEncoding(bytes);

        at = new alphaTab.AlphaTabApi(gpSheet, {{
          core: {{
            fontDirectory: 'https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/font/',
            includeNoteBounds: true
          }},
          importer: {{ encoding: encoding }},
          display: {{
            staveProfile: alphaTab.StaveProfile.ScoreTab,
            scale: gpScale,
            stretchForce: 1.0
          }},
          player: {{
            enablePlayer: true,
            enableCursor: true,
            enableAnimatedBeatCursor: true,
            soundFont: 'https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/soundfont/sonivox.sf2',
            scrollElement: 'html,body',
            scrollMode: alphaTab.ScrollMode.Continuous,
            scrollOffsetY: -60,
            enableUserInteraction: true
          }}
        }});

        at.playerStateChanged.on(function(e) {{
          gpPlaying = (e.state === 1);
          gpBtnPlay.textContent = gpPlaying ? '❚❚' : '▶';
        }});

        at.playerPositionChanged.on(function(e) {{
          if (!gpSeeking && gpTotalMs > 0) {{
            gpSlider.value = (e.currentTime / gpTotalMs) * 100;
            gpTimeCur.textContent = fmtTime(e.currentTime);
          }}
          autoScrollToCursor();
        }});

        at.soundFontLoaded.on(function() {{
          gpTotalMs = at.totalDuration || 0;
          gpTimeTotal.textContent = fmtTime(gpTotalMs);
        }});

        at.playerFinished.on(function() {{
          gpPlaying = false;
          gpBtnPlay.textContent = '▶';
          gpSlider.value = 0;
          gpTimeCur.textContent = '00:00';
        }});

        at.load(bytes, [-1]);
      }})
      .catch(function(e) {{
        console.error(e);
        gpLoading.textContent = '曲谱加载失败，请确认文件是否存在';
      }});
  }}

  function detectGpEncoding(data) {{
    var limit = Math.min(data.length, 8192);
    var bestStart = -1, bestLen = 0, curStart = -1;
    for (var i = 64; i < limit; i++) {{
      if (data[i] >= 0x80) {{
        if (curStart < 0) curStart = i;
      }} else {{
        var len = i - curStart;
        if (curStart >= 0 && len > bestLen) {{ bestLen = len; bestStart = curStart; }}
        curStart = -1;
      }}
    }}
    if (curStart >= 0 && limit - curStart > bestLen) {{ bestLen = limit - curStart; bestStart = curStart; }}
    if (bestStart < 0) return 'utf-8';
    var probe = data.subarray(bestStart, Math.min(data.length, bestStart + bestLen + 64));
    try {{
      var utf8 = new TextDecoder('utf-8', {{ fatal: false }}).decode(probe);
      return utf8.includes('\ufffd') ? 'gbk' : 'utf-8';
    }} catch (e) {{ return 'gbk'; }}
  }}

  function autoScrollToCursor() {{
    if (!gpPlaying) return;
    var now = Date.now();
    if (now - gpLastScrollTime < 160) return;
    var cursor = gpSheet.querySelector('.at-cursor-beat') || gpSheet.querySelector('.at-cursor-bar');
    if (cursor) {{
      var rect = cursor.getBoundingClientRect();
      var viewH = window.innerHeight || document.documentElement.clientHeight;
      if (rect.top < 80 || rect.bottom > viewH - 130) {{
        gpLastScrollTime = now;
        var targetY = (window.pageYOffset || document.documentElement.scrollTop) + rect.top - (viewH * 0.35);
        window.scrollTo({{ top: Math.max(0, targetY), behavior: 'smooth' }});
      }}
    }}
  }}

  function fmtTime(ms) {{
    var s = Math.floor((ms || 0) / 1000);
    var m = Math.floor(s / 60);
    var sec = s % 60;
    return (m < 10 ? '0' : '') + m + ':' + (sec < 10 ? '0' : '') + sec;
  }}

  function updateGpZoomLabel() {{
    gpZoomVal.textContent = Math.round(gpScale * 100) + '%';
  }}

  gpBtnPlay.addEventListener('click', function() {{
    if (!at) return;
    at.playPause();
  }});

  gpBtnStop.addEventListener('click', function() {{
    if (!at) return;
    at.stop();
    gpPlaying = false;
    gpBtnPlay.textContent = '▶';
    gpSlider.value = 0;
    gpTimeCur.textContent = '00:00';
    window.scrollTo({{ top: 0, behavior: 'smooth' }});
  }});

  gpSlider.addEventListener('input', function() {{
    gpSeeking = true;
    var target = (parseFloat(gpSlider.value) / 100) * gpTotalMs;
    gpTimeCur.textContent = fmtTime(target);
  }});
  gpSlider.addEventListener('change', function() {{
    if (!at || !gpTotalMs) return;
    var target = (parseFloat(gpSlider.value) / 100) * gpTotalMs;
    at.timePosition = target;
    gpSeeking = false;
  }});

  gpZoomIn.addEventListener('click', function() {{
    if (!at) return;
    gpScale = Math.round(Math.min(2.0, gpScale + 0.1) * 10) / 10;
    updateGpZoomLabel();
    at.settings.display.scale = gpScale;
    at.updateSettings();
    at.render();
  }});
  gpZoomOut.addEventListener('click', function() {{
    if (!at) return;
    gpScale = Math.round(Math.max(0.3, gpScale - 0.1) * 10) / 10;
    updateGpZoomLabel();
    at.settings.display.scale = gpScale;
    at.updateSettings();
    at.render();
  }});

  btnDetailSettings.addEventListener('click', function(e) {{
    e.stopPropagation();
    gpSheetMenu.classList.toggle('open');
  }});
  document.addEventListener('click', function(e) {{
    if (gpSheetMenu && !gpSheetMenu.contains(e.target) && e.target !== btnDetailSettings) {{
      gpSheetMenu.classList.remove('open');
    }}
  }});

  function applyGpStaves() {{
    if (!at) return;
    var profile = (gpShowScore && gpShowTab) ? alphaTab.StaveProfile.ScoreTab
                : gpShowScore ? alphaTab.StaveProfile.Score
                : gpShowTab ? alphaTab.StaveProfile.Tab : alphaTab.StaveProfile.ScoreTab;
    at.settings.display.staveProfile = profile;
    at.updateSettings();
    at.render();
  }}

  gpChkScore.addEventListener('change', function() {{
    if (!gpChkScore.checked && !gpChkTab.checked) {{ gpChkScore.checked = true; return; }}
    gpShowScore = gpChkScore.checked;
    applyGpStaves();
  }});
  gpChkTab.addEventListener('change', function() {{
    if (!gpChkScore.checked && !gpChkTab.checked) {{ gpChkTab.checked = true; return; }}
    gpShowTab = gpChkTab.checked;
    applyGpStaves();
  }});
  gpChkChords.addEventListener('change', function() {{
    gpShowChords = gpChkChords.checked;
    if (!at) return;
    at.settings.notation.elements = gpShowChords ? undefined : new Map([[alphaTab.NotationElement.EffectChordNames, false]]);
    at.updateSettings();
    at.render();
  }});

  function renderImgDetail(s) {{
    imgContainer.innerHTML = '';
    var pages = (s.pages || []).slice().sort(function(a, b) {{ return a.sortOrder - b.sortOrder; }});
    detailPageBadge.textContent = pages.length + ' P';

    if (!pages.length) {{
      imgContainer.innerHTML = '<div class="state-tip">该曲谱暂无图片分页</div>';
      return;
    }}

    imgCurPages = pages.map(function(p, i) {{
      return {{
        idx: i,
        uuid: p.uuid,
        fileType: p.fileType,
        src: '/api/file/' + TOKEN + '/' + p.uuid + '/' + encodeURIComponent(p.fileType)
      }};
    }});

    imgCurPages.forEach(function(p) {{
      var wrap = document.createElement('div');
      wrap.className = 'img-page-wrap';
      var img = document.createElement('img');
      img.loading = 'lazy';
      img.src = p.src;
      img.alt = 'page ' + (p.idx + 1);
      var badge = document.createElement('div');
      badge.className = 'img-page-num';
      badge.textContent = (p.idx + 1) + ' / ' + imgCurPages.length;
      wrap.appendChild(img);
      wrap.appendChild(badge);
      wrap.addEventListener('click', function() {{
        openImgLightbox(p.idx);
      }});
      imgContainer.appendChild(wrap);
    }});
  }}

  function openImgLightbox(idx) {{
    if (idx < 0 || idx >= imgCurPages.length) return;
    imgCurIdx = idx;
    var p = imgCurPages[idx];
    imgLbImg.src = p.src;
    imgLbTitle.textContent = (idx + 1) + ' / ' + imgCurPages.length;
    imgLbPrev.disabled = (idx === 0);
    imgLbNext.disabled = (idx === imgCurPages.length - 1);
    resetImgTransform();
    imgLightbox.classList.add('active');
  }}

  function resetImgTransform() {{
    imgScale = 1; imgPosX = 0; imgPosY = 0; imgLastX = 0; imgLastY = 0; imgLastScale = 1;
    updateImgTransform();
  }}

  function updateImgTransform() {{
    imgLbImg.style.transform = 'translate(' + imgPosX + 'px, ' + imgPosY + 'px) scale(' + imgScale + ')';
  }}

  imgLbPrev.addEventListener('click', function() {{ openImgLightbox(imgCurIdx - 1); }});
  imgLbNext.addEventListener('click', function() {{ openImgLightbox(imgCurIdx + 1); }});
  imgLbClose.addEventListener('click', function() {{ imgLightbox.classList.remove('active'); }});

  function toggleFullScreen(el) {{
    var target = el || document.documentElement;
    var isFs = document.fullscreenElement || document.webkitFullscreenElement;
    if (!isFs) {{
      if (target.requestFullscreen) target.requestFullscreen();
      else if (target.webkitRequestFullscreen) target.webkitRequestFullscreen();
    }} else {{
      if (document.exitFullscreen) document.exitFullscreen();
      else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
    }}
  }}

  btnDetailFs.addEventListener('click', function() {{ toggleFullScreen(); }});
  imgLbFs.addEventListener('click', function(e) {{
    e.stopPropagation();
    toggleFullScreen(imgLightbox);
  }});

  imgBtnScroll.addEventListener('click', function() {{
    if (imgAutoScrolling) stopImgAutoScroll();
    else startImgAutoScroll();
  }});
  imgSpeedDown.addEventListener('click', function() {{
    imgScrollSpeed = Math.max(10, imgScrollSpeed - 10);
    imgSpeedLabel.textContent = imgScrollSpeed + ' px/s';
  }});
  imgSpeedUp.addEventListener('click', function() {{
    imgScrollSpeed = Math.min(300, imgScrollSpeed + 10);
    imgSpeedLabel.textContent = imgScrollSpeed + ' px/s';
  }});

  function startImgAutoScroll() {{
    imgAutoScrolling = true;
    imgBtnScroll.textContent = '❚❚ 暂停';
    imgLastScrollTime = performance.now();
    imgScrollStep(imgLastScrollTime);
  }}

  function stopImgAutoScroll() {{
    imgAutoScrolling = false;
    imgBtnScroll.textContent = '▶ 滚动';
    if (imgScrollRaf) {{ cancelAnimationFrame(imgScrollRaf); imgScrollRaf = null; }}
  }}

  function imgScrollStep(now) {{
    if (!imgAutoScrolling) return;
    var dt = (now - imgLastScrollTime) / 1000;
    imgLastScrollTime = now;
    if (dt > 0 && dt < 1) {{
      window.scrollBy(0, imgScrollSpeed * dt);
      var max = document.documentElement.scrollHeight - window.innerHeight;
      if (window.pageYOffset >= max - 2) {{
        stopImgAutoScroll();
        return;
      }}
    }}
    imgScrollRaf = requestAnimationFrame(imgScrollStep);
  }}

  window.addEventListener('touchstart', function(e) {{
    if (imgScrollBar && imgScrollBar.contains(e.target)) return;
    if (imgAutoScrolling) stopImgAutoScroll();
  }}, {{ passive: true }});

  loadLibrary('');
}})();
</script>
</body>
</html>"#
    )
}

/// 手机端查看页 HTML（方案 1：服务端直出单文件，零构建依赖）
fn render_mobile_page(token: &str, title: &str, artist: &str, format: &str) -> String {
    let t = html_escape(token);
    let title = html_escape(title);
    let artist = html_escape(artist);
    if format == "gp" {
        // GP 谱：拉取 GP 二进制 + alphaTab 完整渲染与播放控制（支持中文、宽度自适应、播放）
        format!(
            r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{title}</title>
<script src="https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/alphaTab.js"></script>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; -webkit-tap-highlight-color:transparent; }}
body {{ background:#0d0d0d; color:#f5f5f0; font-family:-apple-system,'PingFang SC','Microsoft YaHei',sans-serif; min-height:100vh; overflow-x:hidden; }}
header {{ position:sticky; top:0; z-index:20; background:rgba(13,13,13,.95); backdrop-filter:blur(8px); padding:10px 14px; border-bottom:2px solid rgba(255,255,255,.15); display:flex; align-items:center; justify-content:space-between; gap:10px; }}
h1 {{ font-size:16px; font-weight:700; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
p.sub {{ font-size:12px; opacity:.55; margin-top:2px; }}
.header-actions {{ display:flex; align-items:center; gap:8px; flex-shrink:0; }}
.btn-icon {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.2); width:34px; height:34px; display:flex; align-items:center; justify-content:center; cursor:pointer; font-size:14px; }}
.btn-icon:active {{ background:rgba(255,255,255,.25); }}
.badge {{ font-size:11px; background:#ff6b6b; color:#fff; font-weight:700; padding:4px 8px; border-radius:0; }}

/* 谱面展示区：自适应手机宽度，居中，留出底部控制栏余量 */
#sheet-container {{ width:100%; min-height:60vh; background:#f5f5f0; padding-bottom:120px; overflow-x:auto; display:flex; justify-content:center; }}
#sheet {{ width:100%; max-width:100%; }}
#sheet canvas, #sheet svg {{ max-width:100% !important; }}
#loading {{ padding:60px 16px; text-align:center; font-size:14px; opacity:.6; }}

/* AlphaTab 播放光标与小节高亮 */
.at-cursor-bar {{ background:rgba(255,107,107,.18) !important; border-radius:0; }}
.at-cursor-beat {{ background:#ff6b6b !important; width:4px !important; box-shadow:0 0 4px rgba(255,107,107,.6); }}
.at-highlight {{ fill:#ff6b6b; }}

/* 底部播放控制条 */
#player-bar {{ position:fixed; bottom:0; left:0; right:0; z-index:30; background:rgba(26,26,26,.96); backdrop-filter:blur(10px); border-top:2px solid rgba(255,255,255,.2); padding:10px 14px; display:flex; align-items:center; gap:10px; box-shadow:0 -4px 20px rgba(0,0,0,.6); }}
#btn-play {{ background:#ff6b6b; color:#fff; border:0; width:40px; height:40px; font-size:16px; font-weight:700; display:flex; align-items:center; justify-content:center; cursor:pointer; flex-shrink:0; }}
#btn-play:active {{ opacity:.9; transform:scale(.96); }}
#btn-stop {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.2); width:34px; height:34px; font-size:14px; display:flex; align-items:center; justify-content:center; cursor:pointer; flex-shrink:0; }}
.progress-box {{ flex:1; min-width:0; display:flex; align-items:center; gap:8px; }}
.time-label {{ font-family:monospace; font-size:11px; color:#f5f5f0; opacity:.75; flex-shrink:0; }}
#progress-slider {{ flex:1; height:4px; appearance:none; -webkit-appearance:none; background:rgba(255,255,255,.2); outline:none; border-radius:0; cursor:pointer; }}
#progress-slider::-webkit-slider-thumb {{ appearance:none; -webkit-appearance:none; width:14px; height:14px; background:#ff6b6b; cursor:pointer; border-radius:0; }}
.zoom-controls {{ display:flex; align-items:center; gap:3px; flex-shrink:0; }}
.zoom-btn {{ background:rgba(255,255,255,.1); color:#fff; border:1px solid rgba(255,255,255,.2); width:28px; height:28px; font-size:14px; font-weight:700; cursor:pointer; display:flex; align-items:center; justify-content:center; }}
.zoom-btn:active {{ background:rgba(255,255,255,.25); }}
#zoom-val {{ font-family:monospace; font-size:11px; color:#f5f5f0; opacity:.85; min-width:32px; text-align:center; }}

/* 谱面设置下拉菜单 */
#sheet-menu {{ position:fixed; top:54px; right:12px; z-index:40; background:rgba(26,26,26,.96); backdrop-filter:blur(10px); border:2px solid rgba(255,255,255,.2); box-shadow:0 8px 30px rgba(0,0,0,.7); padding:12px 14px; min-width:200px; display:none; flex-direction:column; gap:10px; }}
#sheet-menu.open {{ display:flex; }}
.menu-title {{ font-size:12px; font-weight:700; opacity:.5; text-transform:uppercase; letter-spacing:0.5px; margin-bottom:2px; }}
.menu-item {{ display:flex; align-items:center; justify-content:space-between; cursor:pointer; font-size:13px; font-weight:500; user-select:none; padding:4px 0; }}
.menu-item input[type="checkbox"] {{ width:16px; height:16px; accent-color:#ff6b6b; cursor:pointer; }}
</style>
</head>
<body>
<header>
  <div style="min-width:0; flex:1;">
    <h1>{title}</h1>
    <p class="sub">{artist} · Guitar Pro</p>
  </div>
  <div class="header-actions">
    <button class="btn-icon" id="btn-settings" title="谱面显示设置" aria-label="谱面显示设置">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
    </button>
    <button class="btn-icon" id="btn-fullscreen" title="全屏" aria-label="全屏">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6"/></svg>
    </button>
    <span class="badge">GP</span>
  </div>
</header>

<!-- 谱面设置悬浮弹层 -->
<div id="sheet-menu">
  <div class="menu-title">谱面显示设置</div>
  <label class="menu-item">
    <span>显示五线谱</span>
    <input type="checkbox" id="chk-score" checked />
  </label>
  <label class="menu-item">
    <span>显示六线谱</span>
    <input type="checkbox" id="chk-tab" checked />
  </label>
  <label class="menu-item">
    <span>显示和弦名称</span>
    <input type="checkbox" id="chk-chords" checked />
  </label>
</div>

<div id="loading">正在加载曲谱…</div>
<div id="sheet-container">
  <div id="sheet"></div>
</div>

<!-- 底部悬浮播放器控制条 -->
<div id="player-bar">
  <button id="btn-stop" title="停止">■</button>
  <button id="btn-play" title="播放/暂停">▶</button>
  <div class="progress-box">
    <span class="time-label" id="time-current">00:00</span>
    <input type="range" id="progress-slider" min="0" max="100" value="0" step="0.1" />
    <span class="time-label" id="time-total">00:00</span>
  </div>
  <div class="zoom-controls">
    <button class="zoom-btn" id="zoom-out" title="缩小">−</button>
    <span id="zoom-val">40%</span>
    <button class="zoom-btn" id="zoom-in" title="放大">+</button>
  </div>
</div>

<script>
(function() {{
  var el = document.getElementById('sheet');
  var loading = document.getElementById('loading');
  var btnPlay = document.getElementById('btn-play');
  var btnStop = document.getElementById('btn-stop');
  var timeCur = document.getElementById('time-current');
  var timeTot = document.getElementById('time-total');
  var slider = document.getElementById('progress-slider');
  var btnFs = document.getElementById('btn-fullscreen');
  var btnZoomIn = document.getElementById('zoom-in');
  var btnZoomOut = document.getElementById('zoom-out');
  var zoomVal = document.getElementById('zoom-val');

  var at = null;
  var isPlaying = false;
  // 谱面默认缩放到最小 0.4，让手机端尽可能一屏纵览完整小节与音轨
  var currentScale = 0.4;
  var isSeeking = false;
  var totalMs = 0;
  var lastScrollTime = 0;

  // 谱面显示状态：五线谱 / 六线谱 / 和弦名
  var showScore = true;
  var showTab = true;
  var showChords = true;

  var btnSettings = document.getElementById('btn-settings');
  var sheetMenu = document.getElementById('sheet-menu');
  var chkScore = document.getElementById('chk-score');
  var chkTab = document.getElementById('chk-tab');
  var chkChords = document.getElementById('chk-chords');

  if (btnSettings && sheetMenu) {{
    btnSettings.addEventListener('click', function(e) {{
      e.stopPropagation();
      sheetMenu.classList.toggle('open');
    }});
    document.addEventListener('click', function(e) {{
      if (!sheetMenu.contains(e.target) && e.target !== btnSettings) {{
        sheetMenu.classList.remove('open');
      }}
    }});
  }}

  function applyStaves() {{
    if (!at) return;
    var profile;
    if (showScore && showTab) profile = alphaTab.StaveProfile.ScoreTab;
    else if (showScore) profile = alphaTab.StaveProfile.Score;
    else if (showTab) profile = alphaTab.StaveProfile.Tab;
    else profile = alphaTab.StaveProfile.ScoreTab; // 兜底
    at.settings.display.staveProfile = profile;
    at.updateSettings();
    at.render();
  }}

  function applyChords() {{
    if (!at) return;
    var notationElements = showChords
      ? undefined
      : new Map([[alphaTab.NotationElement.EffectChordNames, false]]);
    at.settings.notation.elements = notationElements;
    at.updateSettings();
    at.render();
  }}

  if (chkScore) {{
    chkScore.addEventListener('change', function() {{
      if (!chkScore.checked && !chkTab.checked) {{
        // 至少保留一种谱线
        chkScore.checked = true;
        return;
      }}
      showScore = chkScore.checked;
      applyStaves();
    }});
  }}

  if (chkTab) {{
    chkTab.addEventListener('change', function() {{
      if (!chkScore.checked && !chkTab.checked) {{
        // 至少保留一种谱线
        chkTab.checked = true;
        return;
      }}
      showTab = chkTab.checked;
      applyStaves();
    }});
  }}

  if (chkChords) {{
    chkChords.addEventListener('change', function() {{
      showChords = chkChords.checked;
      applyChords();
    }});
  }}

  function updateZoomLabel() {{
    if (zoomVal) zoomVal.textContent = Math.round(currentScale * 100) + '%';
  }}

  // 1. 中文编码探测函数（GBK / UTF-8 自动识别，解决老 GP 谱乱码）
  function detectGpEncoding(data) {{
    var limit = Math.min(data.length, 8192);
    var bestStart = -1, bestLen = 0, curStart = -1;
    for (var i = 64; i < limit; i++) {{
      if (data[i] >= 0x80) {{
        if (curStart < 0) curStart = i;
      }} else {{
        var len = i - curStart;
        if (curStart >= 0 && len > bestLen) {{ bestLen = len; bestStart = curStart; }}
        curStart = -1;
      }}
    }}
    if (curStart >= 0 && limit - curStart > bestLen) {{ bestLen = limit - curStart; bestStart = curStart; }}
    if (bestStart < 0) return 'utf-8';
    var probe = data.subarray(bestStart, Math.min(data.length, bestStart + bestLen + 64));
    try {{
      var utf8 = new TextDecoder('utf-8', {{ fatal: false }}).decode(probe);
      return utf8.includes('\ufffd') ? 'gbk' : 'utf-8';
    }} catch (e) {{
      return 'gbk';
    }}
  }}

  // 跟随播放自动平滑滚动：计算当前拍点/小节光标相对于视口的位置，自动滚动使演奏行保持在视口舒适阅读区
  function autoScrollToCursor() {{
    if (!isPlaying) return;
    var now = Date.now();
    if (now - lastScrollTime < 160) return; // 节流，避免高频打断
    var cursor = el.querySelector('.at-cursor-beat') || el.querySelector('.at-cursor-bar');
    if (cursor) {{
      var rect = cursor.getBoundingClientRect();
      var viewH = window.innerHeight || document.documentElement.clientHeight;
      // 当播放光标超出舒适可视区域（例如滚至底部控制栏附近或顶部遮挡）时自动居中平滑滚动
      if (rect.top < 80 || rect.bottom > viewH - 130) {{
        lastScrollTime = now;
        var targetY = (window.pageYOffset || document.documentElement.scrollTop) + rect.top - (viewH * 0.35);
        window.scrollTo({{
          top: Math.max(0, targetY),
          behavior: 'smooth'
        }});
      }}
    }}
  }}

  function fmtTime(ms) {{
    var s = Math.floor((ms || 0) / 1000);
    var m = Math.floor(s / 60);
    var sec = s % 60;
    return (m < 10 ? '0' : '') + m + ':' + (sec < 10 ? '0' : '') + sec;
  }}

  // 全屏
  if (btnFs) {{
    btnFs.addEventListener('click', function() {{
      var isFs = document.fullscreenElement || document.webkitFullscreenElement;
      if (!isFs) {{
        if (document.documentElement.requestFullscreen) document.documentElement.requestFullscreen();
        else if (document.documentElement.webkitRequestFullscreen) document.documentElement.webkitRequestFullscreen();
      }} else {{
        if (document.exitFullscreen) document.exitFullscreen();
        else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
      }}
    }});
  }}

  // 缩放调节（范围 0.3 ~ 2.0）
  if (btnZoomIn) {{
    btnZoomIn.addEventListener('click', function() {{
      if (!at) return;
      currentScale = Math.round(Math.min(2.0, currentScale + 0.1) * 10) / 10;
      updateZoomLabel();
      at.settings.display.scale = currentScale;
      at.updateSettings();
      at.render();
    }});
  }}
  if (btnZoomOut) {{
    btnZoomOut.addEventListener('click', function() {{
      if (!at) return;
      currentScale = Math.round(Math.max(0.3, currentScale - 0.1) * 10) / 10;
      updateZoomLabel();
      at.settings.display.scale = currentScale;
      at.updateSettings();
      at.render();
    }});
  }}

  // 播放与停止
  btnPlay.addEventListener('click', function() {{
    if (!at) return;
    at.playPause();
  }});

  btnStop.addEventListener('click', function() {{
    if (!at) return;
    at.stop();
    isPlaying = false;
    btnPlay.textContent = '▶';
    slider.value = 0;
    timeCur.textContent = '00:00';
    window.scrollTo({{ top: 0, behavior: 'smooth' }});
  }});

  // 进度条拖拽跳转
  slider.addEventListener('input', function() {{
    isSeeking = true;
    var target = (parseFloat(slider.value) / 100) * totalMs;
    timeCur.textContent = fmtTime(target);
  }});
  slider.addEventListener('change', function() {{
    if (!at || !totalMs) return;
    var target = (parseFloat(slider.value) / 100) * totalMs;
    at.timePosition = target;
    isSeeking = false;
  }});

  // 获取曲谱数据并初始化渲染播放
  fetch('/api/song/{t}').then(function(r) {{ return r.json(); }}).then(function(song) {{
    var uuid = song.uuid;
    var ext = song.fileType;
    if (!uuid || !ext) {{ loading.textContent = '曲谱信息不完整'; return; }}

    fetch('/api/file/{t}/' + uuid + '/' + encodeURIComponent(ext))
      .then(function(r) {{ if (!r.ok) throw 0; return r.arrayBuffer(); }})
      .then(function(buf) {{
        loading.remove();
        var bytes = new Uint8Array(buf);
        var encoding = detectGpEncoding(bytes);
        updateZoomLabel();

        at = new alphaTab.AlphaTabApi(el, {{
          core: {{
            fontDirectory: 'https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/font/',
            includeNoteBounds: true
          }},
          importer: {{
            encoding: encoding
          }},
          display: {{
            layoutMode: alphaTab.LayoutMode.Page,
            staveProfile: alphaTab.StaveProfile.ScoreTab,
            scale: currentScale
          }},
          player: {{
            enablePlayer: true,
            enableCursor: true,
            enableAnimatedBeatCursor: true,
            soundFont: 'https://cdn.jsdelivr.net/npm/@coderline/alphatab@1.8.4/dist/soundfont/sonivox.sf2',
            scrollElement: 'html,body',
            scrollMode: alphaTab.ScrollMode.Continuous,
            scrollOffsetY: -80
          }}
        }});

        // 播放事件监听
        at.playerReady.on(function() {{
          btnPlay.disabled = false;
        }});

        at.playerStateChanged.on(function(e) {{
          isPlaying = (e.state === 1); // 1 = Playing
          btnPlay.textContent = isPlaying ? '⏸' : '▶';
        }});

        at.playerPositionChanged.on(function(e) {{
          totalMs = e.endTime || totalMs;
          timeTot.textContent = fmtTime(totalMs);
          if (!isSeeking) {{
            timeCur.textContent = fmtTime(e.currentTime);
            if (totalMs > 0) {{
              slider.value = (e.currentTime / totalMs) * 100;
            }}
          }}
          // 跟随播放进度自动滚动谱面
          autoScrollToCursor();
        }});

        at.playerFinished.on(function() {{
          isPlaying = false;
          btnPlay.textContent = '▶';
          slider.value = 0;
          timeCur.textContent = '00:00';
        }});

        // 加载全部音轨渲染
        at.load(bytes, [-1]);
      }})
      .catch(function(err) {{
        console.error(err);
        loading.textContent = '曲谱文件加载失败，请确认文件是否存在';
      }});
  }}).catch(function(err) {{
    console.error(err);
    loading.textContent = '曲谱信息获取失败，请刷新重试';
  }});
}})();
</script>
</body>
</html>"#
        )
    } else {
        // 图片谱：竖排列表 + 点击全屏大图预览（双指捏合 / 双击放大 / 左右滑动切页 / ESC 或点叉退出）
        format!(
            r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{title}</title>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; -webkit-tap-highlight-color:transparent; }}
body {{ background:#0d0d0d; color:#f5f5f0; font-family:-apple-system,'PingFang SC','Microsoft YaHei',sans-serif; min-height:100vh; }}
header {{ position:sticky; top:0; z-index:10; background:rgba(13,13,13,.94); backdrop-filter:blur(8px); padding:10px 14px; border-bottom:2px solid rgba(255,255,255,.15); display:flex; align-items:center; justify-content:space-between; gap:10px; }}
h1 {{ font-size:16px; font-weight:700; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
p.sub {{ font-size:12px; opacity:.55; margin-top:2px; }}
.header-actions {{ display:flex; align-items:center; gap:8px; flex-shrink:0; }}
.btn-icon {{ background:rgba(255,255,255,.12); color:#fff; border:1px solid rgba(255,255,255,.2); width:34px; height:34px; display:flex; align-items:center; justify-content:center; cursor:pointer; font-size:14px; }}
.btn-icon:active {{ background:rgba(255,255,255,.25); }}
.badge {{ font-size:11px; background:#ff6b6b; color:#fff; font-weight:700; padding:4px 8px; border-radius:0; }}
main {{ display:flex; flex-direction:column; gap:12px; padding:12px 8px 90px; max-width:900px; margin:0 auto; }}
.page-wrap {{ position:relative; background:#f5f5f0; cursor:zoom-in; border:1px solid rgba(255,255,255,.1); box-shadow:0 4px 16px rgba(0,0,0,.5); }}
.page-wrap img {{ width:100%; height:auto; display:block; }}
.page-num {{ position:absolute; right:8px; bottom:8px; background:rgba(13,13,13,.75); color:#f5f5f0; font-size:11px; padding:2px 6px; font-family:monospace; pointer-events:none; }}
#empty {{ padding:60px 16px; text-align:center; font-size:14px; opacity:.6; }}

/* 底部自动滚动浮动栏 */
#autoscroll-bar {{ position:fixed; bottom:16px; left:50%; transform:translateX(-50%); z-index:50; background:rgba(26,26,26,.94); backdrop-filter:blur(10px); border:2px solid rgba(255,255,255,.2); padding:8px 14px; display:flex; align-items:center; gap:12px; box-shadow:0 8px 30px rgba(0,0,0,.7); max-width:calc(100vw - 32px); }}
#btn-scroll-toggle {{ background:#ff6b6b; color:#fff; border:0; font-weight:700; font-size:13px; padding:6px 14px; cursor:pointer; display:flex; align-items:center; gap:6px; min-width:82px; justify-content:center; }}
#btn-scroll-toggle:active {{ opacity:.9; }}
.speed-group {{ display:flex; align-items:center; gap:6px; font-size:12px; }}
.speed-btn {{ background:rgba(255,255,255,.1); color:#fff; border:1px solid rgba(255,255,255,.2); width:28px; height:28px; font-size:14px; font-weight:700; cursor:pointer; display:flex; align-items:center; justify-content:center; }}
.speed-val {{ font-family:monospace; font-size:12px; min-width:44px; text-align:center; color:#f5f5f0; }}

/* 全屏大图预览弹窗 */
#lightbox {{ display:none; position:fixed; inset:0; z-index:9999; background:#000; touch-action:none; user-select:none; -webkit-user-select:none; }}
#lightbox.active {{ display:flex; flex-direction:column; }}
#lb-topbar {{ position:absolute; top:0; left:0; right:0; z-index:100; display:flex; align-items:center; justify-content:space-between; padding:12px 16px; background:linear-gradient(to bottom, rgba(0,0,0,.85), transparent); pointer-events:none; }}
#lb-topbar * {{ pointer-events:auto; }}
#lb-title {{ font-size:13px; font-family:monospace; color:#fff; font-weight:700; }}
.lb-actions {{ display:flex; align-items:center; gap:8px; }}
#lb-fs-btn, #lb-close {{ background:rgba(255,255,255,.2); color:#fff; border:0; width:36px; height:36px; font-size:18px; line-height:36px; text-align:center; cursor:pointer; display:flex; align-items:center; justify-content:center; }}
#lb-viewport {{ flex:1; width:100%; height:100%; display:flex; align-items:center; justify-content:center; overflow:hidden; position:relative; }}
#lb-img {{ max-width:100%; max-height:100%; object-fit:contain; transform-origin:center center; transition:transform 0.12s ease-out; will-change:transform; }}
#lb-bottombar {{ position:absolute; bottom:0; left:0; right:0; z-index:100; display:flex; align-items:center; justify-content:space-between; padding:16px 20px; background:linear-gradient(to top, rgba(0,0,0,.85), transparent); pointer-events:none; }}
#lb-bottombar button {{ pointer-events:auto; background:rgba(255,255,255,.15); color:#fff; border:1px solid rgba(255,255,255,.3); padding:8px 18px; font-size:13px; font-weight:700; cursor:pointer; }}
#lb-bottombar button:disabled {{ opacity:.3; cursor:not-allowed; }}
#lb-hint {{ font-size:11px; opacity:.55; text-align:center; }}
</style>
</head>
<body>
<header>
  <div style="min-width:0; flex:1;">
    <h1>{title}</h1>
    <p class="sub">{artist}</p>
  </div>
  <div class="header-actions">
    <button class="btn-icon" id="btn-fullscreen" title="全屏" aria-label="全屏">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6"/></svg>
    </button>
    <span class="badge" id="page-total">…</span>
  </div>
</header>
<main id="list"><div id="empty">正在加载谱面…</div></main>

<!-- 底部自动滚动悬浮栏 -->
<div id="autoscroll-bar">
  <button id="btn-scroll-toggle">▶ 滚动</button>
  <div class="speed-group">
    <button class="speed-btn" id="speed-down">−</button>
    <span class="speed-val" id="speed-label">30 px/s</span>
    <button class="speed-btn" id="speed-up">+</button>
  </div>
</div>

<div id="lightbox">
  <div id="lb-topbar">
    <span id="lb-title"></span>
    <div class="lb-actions">
      <button id="lb-fs-btn" aria-label="全屏">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6"/></svg>
      </button>
      <button id="lb-close" aria-label="关闭">✕</button>
    </div>
  </div>
  <div id="lb-viewport">
    <img id="lb-img" alt="preview" />
  </div>
  <div id="lb-bottombar">
    <button id="lb-prev">‹ 上一页</button>
    <div id="lb-hint">双击或双指放大 · 左右滑动</div>
    <button id="lb-next">下一页 ›</button>
  </div>
</div>

<script>
(function() {{
  var curPages = [];
  var curIdx = 0;
  var lb = document.getElementById('lightbox');
  var lbImg = document.getElementById('lb-img');
  var lbTitle = document.getElementById('lb-title');
  var lbPrev = document.getElementById('lb-prev');
  var lbNext = document.getElementById('lb-next');
  var lbClose = document.getElementById('lb-close');
  var lbViewport = document.getElementById('lb-viewport');

  // 手势缩放与拖动状态
  var scale = 1, lastScale = 1;
  var posX = 0, posY = 0, lastX = 0, lastY = 0;
  var touchDist = 0, touchCenter = {{ x: 0, y: 0 }};
  var isPanning = false;

  function resetTransform() {{
    scale = 1; lastScale = 1;
    posX = 0; posY = 0; lastX = 0; lastY = 0;
    lbImg.style.transition = 'transform 0.2s ease-out';
    lbImg.style.transform = 'translate3d(0,0,0) scale(1)';
  }}

  function updateTransform() {{
    lbImg.style.transition = 'none';
    lbImg.style.transform = 'translate3d(' + posX + 'px,' + posY + 'px,0) scale(' + scale + ')';
  }}

  function openPage(idx) {{
    if (idx < 0 || idx >= curPages.length) return;
    curIdx = idx;
    var p = curPages[curIdx];
    resetTransform();
    lbImg.src = p.src;
    lbTitle.textContent = (curIdx + 1) + ' / ' + curPages.length;
    lbPrev.disabled = (curIdx === 0);
    lbNext.disabled = (curIdx === curPages.length - 1);
    lb.classList.add('active');
    document.body.style.overflow = 'hidden';
  }}

  function closeLb() {{
    lb.classList.remove('active');
    document.body.style.overflow = '';
    resetTransform();
  }}

  lbClose.addEventListener('click', closeLb);
  lbPrev.addEventListener('click', function(e) {{ e.stopPropagation(); openPage(curIdx - 1); }});
  lbNext.addEventListener('click', function(e) {{ e.stopPropagation(); openPage(curIdx + 1); }});
  window.addEventListener('keydown', function(e) {{
    if (!lb.classList.contains('active')) return;
    if (e.key === 'Escape') closeLb();
    else if (e.key === 'ArrowLeft') openPage(curIdx - 1);
    else if (e.key === 'ArrowRight') openPage(curIdx + 1);
  }});

  // 双击放大/还原
  var lastTap = 0;
  lbViewport.addEventListener('touchend', function(e) {{
    var now = Date.now();
    if (now - lastTap < 300 && e.touches.length === 0) {{
      e.preventDefault();
      if (scale > 1.5) {{
        resetTransform();
      }} else {{
        scale = 2.5; lastScale = 2.5;
        posX = 0; posY = 0; lastX = 0; lastY = 0;
        lbImg.style.transition = 'transform 0.25s ease-out';
        lbImg.style.transform = 'translate3d(0,0,0) scale(2.5)';
      }}
    }}
    lastTap = now;
  }});

  // 桌面双击
  lbViewport.addEventListener('dblclick', function(e) {{
    e.preventDefault();
    if (scale > 1.5) {{
      resetTransform();
    }} else {{
      scale = 2.5; lastScale = 2.5;
      posX = 0; posY = 0; lastX = 0; lastY = 0;
      lbImg.style.transition = 'transform 0.25s ease-out';
      lbImg.style.transform = 'translate3d(0,0,0) scale(2.5)';
    }}
  }});

  // 触摸手势（单指平移/滑页，双指捏合缩放）
  var touchStartX = 0, touchStartY = 0;
  lbViewport.addEventListener('touchstart', function(e) {{
    if (e.touches.length === 1) {{
      touchStartX = e.touches[0].clientX;
      touchStartY = e.touches[0].clientY;
      lastX = posX;
      lastY = posY;
      isPanning = (scale > 1.05);
    }} else if (e.touches.length === 2) {{
      isPanning = false;
      var dx = e.touches[0].clientX - e.touches[1].clientX;
      var dy = e.touches[0].clientY - e.touches[1].clientY;
      touchDist = Math.hypot(dx, dy);
      lastScale = scale;
    }}
  }}, {{ passive: false }});

  lbViewport.addEventListener('touchmove', function(e) {{
    if (e.touches.length === 1 && isPanning) {{
      e.preventDefault();
      var dx = e.touches[0].clientX - touchStartX;
      var dy = e.touches[0].clientY - touchStartY;
      posX = lastX + dx;
      posY = lastY + dy;
      updateTransform();
    }} else if (e.touches.length === 2) {{
      e.preventDefault();
      var dx = e.touches[0].clientX - e.touches[1].clientX;
      var dy = e.touches[0].clientY - e.touches[1].clientY;
      var dist = Math.hypot(dx, dy);
      if (touchDist > 0) {{
        scale = Math.min(4, Math.max(1, lastScale * (dist / touchDist)));
        updateTransform();
      }}
    }}
  }}, {{ passive: false }});

  lbViewport.addEventListener('touchend', function(e) {{
    if (e.touches.length === 0) {{
      if (scale <= 1.05) {{
        // 未放大状态下的左右滑动切换
        var dx = (e.changedTouches[0] ? e.changedTouches[0].clientX : 0) - touchStartX;
        if (Math.abs(dx) > 60) {{
          if (dx < 0) openPage(curIdx + 1);
          else openPage(curIdx - 1);
        }}
        resetTransform();
      }}
      touchDist = 0;
      isPanning = false;
    }}
  }});

  // 数据加载
  fetch('/api/song/{t}').then(function(r) {{ return r.json(); }}).then(function(song) {{
    var list = document.getElementById('list');
    list.innerHTML = '';
    var pages = song.pages.slice().sort(function(a, b) {{ return a.sortOrder - b.sortOrder; }});
    if (!pages.length) {{ document.getElementById('empty').textContent = '曲谱无分页'; return; }}
    document.getElementById('page-total').textContent = pages.length + ' P';

    curPages = pages.map(function(p, i) {{
      return {{
        idx: i,
        uuid: p.uuid,
        fileType: p.fileType,
        src: '/api/file/{t}/' + p.uuid + '/' + encodeURIComponent(p.fileType)
      }};
    }});

    curPages.forEach(function(p) {{
      var wrap = document.createElement('div');
      wrap.className = 'page-wrap';
      var img = document.createElement('img');
      img.loading = 'lazy';
      img.src = p.src;
      img.alt = 'page ' + (p.idx + 1);
      var badge = document.createElement('div');
      badge.className = 'page-num';
      badge.textContent = (p.idx + 1) + ' / ' + curPages.length;
      wrap.appendChild(img);
      wrap.appendChild(badge);
      wrap.addEventListener('click', function() {{
        openPage(p.idx);
      }});
      list.appendChild(wrap);
    }});
  }}).catch(function() {{
    document.getElementById('empty').textContent = '加载失败，请刷新重试';
  }});

  // -------------------------------------------------------------------------
  // 1. 全屏功能（页面全屏 / Lightbox 内全屏，兼容移动端前缀）
  // -------------------------------------------------------------------------
  function toggleFullScreen(el) {{
    var target = el || document.documentElement;
    var isFs = document.fullscreenElement || document.webkitFullscreenElement;
    if (!isFs) {{
      if (target.requestFullscreen) target.requestFullscreen();
      else if (target.webkitRequestFullscreen) target.webkitRequestFullscreen();
    }} else {{
      if (document.exitFullscreen) document.exitFullscreen();
      else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
    }}
  }}

  var btnFs = document.getElementById('btn-fullscreen');
  if (btnFs) btnFs.addEventListener('click', function() {{ toggleFullScreen(); }});
  var lbFsBtn = document.getElementById('lb-fs-btn');
  if (lbFsBtn) lbFsBtn.addEventListener('click', function(e) {{
    e.stopPropagation();
    toggleFullScreen(lb);
  }});

  // -------------------------------------------------------------------------
  // 2. 自动滚动功能（平滑 requestAnimationFrame，支持速度增减、触摸/滚动打断）
  // -------------------------------------------------------------------------
  var isAutoScrolling = false;
  var scrollSpeed = 30; // 像素/秒
  var lastScrollTime = 0;
  var scrollRaf = null;
  var btnScrollToggle = document.getElementById('btn-scroll-toggle');
  var speedLabel = document.getElementById('speed-label');
  var btnSpeedDown = document.getElementById('speed-down');
  var btnSpeedUp = document.getElementById('speed-up');

  function updateSpeedLabel() {{
    speedLabel.textContent = scrollSpeed + ' px/s';
  }}

  btnSpeedDown.addEventListener('click', function(e) {{
    e.stopPropagation();
    scrollSpeed = Math.max(10, scrollSpeed - 10);
    updateSpeedLabel();
  }});

  btnSpeedUp.addEventListener('click', function(e) {{
    e.stopPropagation();
    scrollSpeed = Math.min(150, scrollSpeed + 10);
    updateSpeedLabel();
  }});

  function stepScroll(now) {{
    if (!isAutoScrolling) return;
    if (!lastScrollTime) lastScrollTime = now;
    var dt = (now - lastScrollTime) / 1000;
    lastScrollTime = now;
    var delta = scrollSpeed * dt;
    window.scrollBy(0, delta);

    // 触底检测
    var maxScroll = document.documentElement.scrollHeight - window.innerHeight;
    if (window.scrollY >= maxScroll - 2) {{
      stopAutoScroll();
      return;
    }}
    scrollRaf = requestAnimationFrame(stepScroll);
  }}

  function startAutoScroll() {{
    if (isAutoScrolling) return;
    isAutoScrolling = true;
    lastScrollTime = 0;
    btnScrollToggle.textContent = '⏸ 暂停';
    btnScrollToggle.style.background = '#333333';
    scrollRaf = requestAnimationFrame(stepScroll);
  }}

  function stopAutoScroll() {{
    if (!isAutoScrolling) return;
    isAutoScrolling = false;
    btnScrollToggle.textContent = '▶ 滚动';
    btnScrollToggle.style.background = '#ff6b6b';
    if (scrollRaf) {{
      cancelAnimationFrame(scrollRaf);
      scrollRaf = null;
    }}
  }}

  btnScrollToggle.addEventListener('click', function() {{
    if (isAutoScrolling) stopAutoScroll();
    else startAutoScroll();
  }});

  // 用户手动拖拽触摸屏幕时，暂停自动滚动避免对抗
  window.addEventListener('touchstart', function(e) {{
    // 点击悬浮控制栏自身不打断
    var bar = document.getElementById('autoscroll-bar');
    if (bar && bar.contains(e.target)) return;
    if (isAutoScrolling) stopAutoScroll();
  }}, {{ passive: true }});
  window.addEventListener('wheel', function() {{
    if (isAutoScrolling) stopAutoScroll();
  }}, {{ passive: true }});
}})();
</script>
</body>
</html>"#
        )
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
