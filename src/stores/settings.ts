import { ref, watch } from "vue";
import { defineStore } from "pinia";
import { api, type AppSettings } from "@/lib/tauri";
import { i18n } from "@/i18n";

export const DEFAULT_SETTINGS: AppSettings = {
  general: {
    auto_start: false,
    library_path: "",
    language: "zh-CN",
    restore_window: true,
  },
  appearance: {
    theme: "dark",
    default_view_mode: "fixed",
    bg_color: "#1A1A1A",
    zoom_mode: "fit_height",
  },
  ai_control: {
    camera_id: 0,
    sensitivity: "medium",
    cooldown_ms: 1500,
    show_preview: true,
    page_effect: true,
  },
  shortcuts: {
    next_page: "ArrowRight",
    prev_page: "ArrowLeft",
    toggle_fullscreen: "F11",
    toggle_autoscroll: "Space",
  },
  gp_player: {
    zoom: 1.0,
    show_standard_notation: true,
    show_chord_names: true,
    count_in_beats: 0,
    scroll_mode: "smooth",
    metronome_volume: 60,
    last_import_format: "image",
  },
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<AppSettings>(structuredClone(DEFAULT_SETTINGS));
  const loaded = ref(false);
  const saving = ref(false);

  let saveTimer: number | null = null;
  let systemDark: MediaQueryList | null = null;

  async function load() {
    try {
      const loaded = await api.getSettings();
      // 深合并默认设置：兼容老 settings.json 缺少的新增字段（如 gp_player）
      settings.value = {
        ...structuredClone(DEFAULT_SETTINGS),
        ...loaded,
        general: { ...DEFAULT_SETTINGS.general, ...loaded.general },
        appearance: { ...DEFAULT_SETTINGS.appearance, ...loaded.appearance },
        ai_control: { ...DEFAULT_SETTINGS.ai_control, ...loaded.ai_control },
        shortcuts: { ...DEFAULT_SETTINGS.shortcuts, ...loaded.shortcuts },
        gp_player: { ...DEFAULT_SETTINGS.gp_player, ...(loaded.gp_player ?? {}) },
      };
    } catch {
      settings.value = structuredClone(DEFAULT_SETTINGS);
    }
    loaded.value = true;
    applyLocale();
  }

  /** 修改后调用：防抖持久化 */
  function save(immediate = false) {
    if (saveTimer !== null) window.clearTimeout(saveTimer);
    const doSave = async () => {
      saving.value = true;
      try {
        await api.saveSettings(JSON.parse(JSON.stringify(settings.value)));
      } finally {
        saving.value = false;
      }
    };
    if (immediate) void doSave();
    else saveTimer = window.setTimeout(doSave, 400);
  }

  /** 跟随系统时应使用的主题名 */
  function resolvedThemeName(): "dark" | "light" {
    const theme = settings.value.appearance.theme;
    if (theme === "system") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    return theme === "light" ? "light" : "dark";
  }

  /** 语言即时生效 */
  function applyLocale() {
    i18n.locale = settings.value.general.language || "zh-CN";
  }

  /** 主题相关 DOM 类即时生效（Vuetify 部分由 App.vue 应用） */
  function applyThemeClass() {
    const name = resolvedThemeName();
    document.documentElement.classList.toggle("theme-light", name === "light");
    if (!systemDark) {
      systemDark = window.matchMedia("(prefers-color-scheme: dark)");
      systemDark.addEventListener("change", () => {
        if (settings.value.appearance.theme === "system") applyThemeClass();
      });
    }
  }

  async function resetToDefaults() {
    const libraryPath = settings.value.general.library_path;
    settings.value = structuredClone(DEFAULT_SETTINGS);
    settings.value.general.library_path = libraryPath;
    applyLocale();
    applyThemeClass();
    await api.saveSettings(JSON.parse(JSON.stringify(settings.value)));
  }

  watch(() => settings.value.general.language, applyLocale);

  return { settings, loaded, saving, load, save, applyLocale, applyThemeClass, resolvedThemeName, resetToDefaults };
});
