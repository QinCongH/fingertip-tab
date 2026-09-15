// Tauri 后端命令的类型封装（与 src-tauri/src/commands/*.rs 一一对应）
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 数据类型
// ---------------------------------------------------------------------------

export interface PageDto {
  uuid: string;
  fileType: string;
  sortOrder: number;
}

export interface SongDto {
  id: number;
  uuid: string;
  title: string;
  artist: string;
  album: string;
  tuning: string;
  bpm: number | null;
  tags: string;
  fileType: string;
  /** 'image'：图片曲谱；'gp'：Guitar Pro 曲谱 */
  format: string;
  /** GP 曲谱练习状态 JSON（速度/音轨/循环等） */
  playerState: string | null;
  createdAt: number;
  updatedAt: number;
  pages: PageDto[];
  collectionIds: number[];
}

export interface CollectionDto {
  id: number;
  name: string;
  songCount: number;
  createdAt: number;
}

export interface SongMetaInput {
  title: string;
  artist: string;
  album: string;
  tuning: string;
  bpm: number | null;
  tags: string;
}

/** 与后端 AppSettings（settings.json, snake_case）对应 */
export interface GeneralSettings {
  auto_start: boolean;
  library_path: string;
  language: string; // zh-CN | en
  restore_window: boolean;
}

export interface AppearanceSettings {
  theme: string; // system | light | dark
  default_view_mode: string; // fixed | scroll | dual_horizontal
  bg_color: string;
  zoom_mode: string; // fit_width | fit_height | actual
}

export interface AiControlSettings {
  camera_id: number;
  sensitivity: string; // low | medium | high
  cooldown_ms: number;
  show_preview: boolean;
  page_effect: boolean;
}

export interface ShortcutSettings {
  next_page: string;
  prev_page: string;
  toggle_fullscreen: string;
  toggle_autoscroll: string;
}

export interface GpPlayerSettings {
  /** 谱面缩放 0.5–2.0 */
  zoom: number;
  show_standard_notation: boolean;
  show_chord_names: boolean;
  /** 预备拍拍数：0 关闭 / 1 / 2 / 4 */
  count_in_beats: number;
  /** continuous | off_screen | smooth | off */
  scroll_mode: string;
  metronome_volume: number;
  /** 上次导入选择的曲谱类型：image | gp */
  last_import_format: string;
}

export interface AppSettings {
  general: GeneralSettings;
  appearance: AppearanceSettings;
  ai_control: AiControlSettings;
  shortcuts: ShortcutSettings;
  gp_player: GpPlayerSettings;
}

export interface WindowState {
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
  maximized: boolean | null;
}

export interface AppConfig {
  libraryPath: string | null;
  lastWindow: WindowState | null;
  lastSongId: number | null;
}

export interface CaptureResult {
  data: string;
  width: number;
  height: number;
}

// ---------------------------------------------------------------------------
// 命令封装
// ---------------------------------------------------------------------------

export const api = {
  // 曲谱
  importSongs: (
    paths: string[],
    extraImages: { data: string; ext: string }[],
    meta: SongMetaInput,
    collectionIds: number[],
  ) => invoke<SongDto>("import_songs", { paths, extraImages, meta, collectionIds }),
  importGpSong: (path: string, meta: SongMetaInput, collectionIds: number[]) =>
    invoke<SongDto>("import_gp_song", { path, meta, collectionIds }),
  readGpFile: (uuid: string, fileType: string) =>
    invoke<string>("read_gp_file", { uuid, fileType }),
  updatePlayerState: (id: number, playerState: string | null) =>
    invoke<void>("update_player_state", { id, playerState }),
  readImageBase64: (path: string) => invoke<string>("read_image_base64", { path }),
  getSongs: (collectionId: number | null, search: string | null) =>
    invoke<SongDto[]>("get_songs", { collectionId, search }),
  getSong: (id: number) => invoke<SongDto>("get_song", { id }),
  updateSong: (id: number, meta: SongMetaInput, collectionIds: number[]) =>
    invoke<void>("update_song", { id, meta, collectionIds }),
  setSongPages: (id: number, uuids: string[]) =>
    invoke<SongDto>("set_song_pages", { id, uuids }),
  addPagesToSong: (id: number, paths: string[]) =>
    invoke<SongDto>("add_pages_to_song", { id, paths }),
  deleteSong: (id: number) => invoke<void>("delete_song", { id }),

  // 分组
  getCollections: () => invoke<CollectionDto[]>("get_collections"),
  createCollection: (name: string) =>
    invoke<CollectionDto>("create_collection", { name }),
  renameCollection: (id: number, name: string) =>
    invoke<void>("rename_collection", { id, name }),
  deleteCollection: (id: number) =>
    invoke<void>("delete_collection", { id }),

  // 设置
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) =>
    invoke<void>("save_settings", { settings }),
  migrateLibrary: (newPath: string) =>
    invoke<void>("migrate_library", { newPath }),
  backupLibrary: (dest: string) => invoke<string>("backup_library", { dest }),
  restoreLibrary: (src: string) => invoke<void>("restore_library", { src }),

  // 系统
  captureScreen: () => invoke<CaptureResult>("capture_screen"),
  getAppConfig: () => invoke<AppConfig>("get_app_config"),
  saveAppConfig: (config: AppConfig) => invoke<void>("save_app_config", { config }),
  restartApp: () => invoke<void>("restart_app"),
};

// ---------------------------------------------------------------------------
// 资源 URL（asset protocol）
// ---------------------------------------------------------------------------

export function pageImageUrl(
  libraryPath: string,
  uuid: string,
  fileType: string,
): string {
  const p = `${libraryPath.replace(/[\\/]+$/, "")}/Resources/Images/${uuid}.${fileType}`;
  return convertFileSrc(p);
}

export function thumbImageUrl(
  libraryPath: string,
  uuid: string,
): string {
  const p = `${libraryPath.replace(/[\\/]+$/, "")}/Resources/Thumbnails/${uuid}_thumb.jpg`;
  return convertFileSrc(p);
}
