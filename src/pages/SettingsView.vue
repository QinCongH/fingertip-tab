<template>
  <div class="settings-root d-flex flex-column" :style="{ background: 'var(--st-black)' }">
    <!-- 顶部拖拽栏及窗口控制 -->
    <div class="settings-titlebar d-flex align-center justify-space-between px-6" data-tauri-drag-region>
      <div class="st-mono text-caption" data-tauri-drag-region style="opacity: 0.6">
        {{ t("settings.title") }} · {{ t("app.name") }}
      </div>
      <WindowControls />
    </div>

    <div class="d-flex flex-grow-1" style="height: calc(100vh - 42px); overflow: hidden">
      <!-- 左侧分类导航 -->
      <div class="side pa-6 d-flex flex-column" data-tauri-drag-region>
        <v-btn
          variant="text"
          prepend-icon="mdi-arrow-left"
          class="align-self-start mb-6"
          style="-webkit-app-region: no-drag"
          @click="router.push('/')"
        >
          {{ t("app.name") }}
        </v-btn>
        <div class="st-display text-h4 mb-8" data-tauri-drag-region>{{ t("settings.title") }}</div>

        <div
          v-for="sec in sections"
          :key="sec.value"
          class="st-nav-item pa-3 mb-1 d-flex align-center ga-3 st-clickable"
          :class="{ 'st-nav-item--active': active === sec.value }"
          style="-webkit-app-region: no-drag"
          @click="active = sec.value"
        >
          <v-icon size="20">{{ sec.icon }}</v-icon>
          <span class="text-body-2 font-weight-bold">{{ sec.title }}</span>
        </div>

        <v-spacer data-tauri-drag-region />
        <v-btn variant="tonal" prepend-icon="mdi-restore" block style="-webkit-app-region: no-drag" @click="resetOpen = true">
          {{ t("settings.reset") }}
        </v-btn>
      </div>

      <!-- 右侧内容 -->
      <div class="content pa-10" style="overflow-y: auto; height: 100%; flex: 1">
      <!-- ============ 通用 ============ -->
      <template v-if="active === 'general'">
        <div class="st-display text-h3 mb-8">{{ t("settings.tab_general") }}</div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.autostart") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.autostart_desc") }}</div>
          </div>
          <v-switch
            v-model="s.general.auto_start"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.restore_window") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.restore_window_desc") }}</div>
          </div>
          <v-switch
            v-model="s.general.restore_window"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="st-divider my-8" />

        <div class="text-h6 font-weight-black mb-4">{{ t("settings.storage_manage") }}</div>

        <div class="setting-row">
          <div class="min-width-0 flex-grow-1">
            <div class="font-weight-bold">{{ t("settings.library_path") }}</div>
            <div class="st-mono text-truncate mt-1" style="max-width: 560px">
              {{ s.general.library_path || "…" }}
            </div>
          </div>
          <v-btn variant="tonal" prepend-icon="mdi-folder-move-outline" @click="migrateOpen = true">
            {{ t("settings.change_path") }}
          </v-btn>
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.backup") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.backup_desc") }}</div>
          </div>
          <v-btn variant="tonal" prepend-icon="mdi-package-down" :loading="backing" @click="doBackup">
            {{ t("settings.backup") }}
          </v-btn>
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.restore_backup") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.restore_desc") }}</div>
          </div>
          <v-btn variant="tonal" prepend-icon="mdi-package-up" @click="restoreOpen = true">
            {{ t("settings.restore_backup") }}
          </v-btn>
        </div>

        <div class="st-divider my-8" />

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.language") }}</div>
          </div>
          <v-select
            v-model="s.general.language"
            :items="[
              { title: '简体中文', value: 'zh-CN' },
              { title: 'English', value: 'en' },
            ]"
            density="comfortable"
            variant="underlined"
            hide-details
            max-width="220"
            @update:model-value="onLanguageChange"
          />
        </div>
      </template>

      <!-- ============ 显示 ============ -->
      <template v-else-if="active === 'display'">
        <div class="st-display text-h3 mb-8">{{ t("settings.tab_display") }}</div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.theme") }}</div>
          <v-btn-toggle
            :model-value="s.appearance.theme"
            mandatory
            variant="outlined"
            @update:model-value="onThemeChange"
          >
            <v-btn value="system">{{ t("settings.theme_system") }}</v-btn>
            <v-btn value="light">{{ t("settings.theme_light") }}</v-btn>
            <v-btn value="dark">{{ t("settings.theme_dark") }}</v-btn>
          </v-btn-toggle>
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.default_mode") }}</div>
          <v-select
            v-model="s.appearance.default_view_mode"
            :items="[
              { title: t('viewer.mode_fixed'), value: 'fixed' },
              { title: t('viewer.mode_scroll'), value: 'scroll' },
              { title: t('viewer.mode_dual'), value: 'dual_horizontal' },
            ]"
            density="comfortable"
            variant="underlined"
            hide-details
            max-width="220"
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div class="flex-grow-1">
            <div class="font-weight-bold">{{ t("settings.bg_color") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.bg_color_desc") }}</div>
          </div>
          <div class="d-flex align-center ga-2">
            <div
              v-for="c in BG_PRESETS"
              :key="c"
              class="swatch"
              :class="{ active: s.appearance.bg_color.toLowerCase() === c.toLowerCase() }"
              :style="{ background: c }"
              @click="setBg(c)"
            />
            <label class="swatch custom d-flex align-center justify-center">
              <v-icon icon="mdi-eyedropper-variant" size="16" />
              <input
                type="color"
                class="color-input"
                :value="s.appearance.bg_color"
                @input="setBg(($event.target as HTMLInputElement).value)"
              />
            </label>
          </div>
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.zoom_behavior") }}</div>
          <v-select
            v-model="s.appearance.zoom_mode"
            :items="[
              { title: t('viewer.zoom_fit_width'), value: 'fit_width' },
              { title: t('viewer.zoom_fit_height'), value: 'fit_height' },
              { title: t('viewer.zoom_actual'), value: 'actual' },
            ]"
            density="comfortable"
            variant="underlined"
            hide-details
            max-width="220"
            @update:model-value="saveNow"
          />
        </div>
      </template>

      <!-- ============ AI 控制 ============ -->
      <template v-else-if="active === 'ai'">
        <div class="st-display text-h3 mb-8">{{ t("settings.tab_ai") }}</div>

        <div class="setting-row">
          <div class="flex-grow-1">
            <div class="font-weight-bold">{{ t("settings.camera") }}</div>
            <div v-if="!cameras.length" class="text-body-2" style="opacity: 0.5">
              {{ t("settings.camera_none") }}
            </div>
          </div>
          <div class="d-flex align-center ga-2" style="max-width: 380px; width: 380px">
            <v-select
              v-model="s.ai_control.camera_id"
              :items="cameraItems"
              density="comfortable"
              variant="underlined"
              hide-details
              :disabled="!cameras.length"
              @update:model-value="saveNow"
            />
            <v-btn icon="mdi-refresh" variant="tonal" :loading="probing" :title="t('settings.camera_refresh')" @click="probeCameras" />
          </div>
        </div>

        <div class="setting-row">
          <div class="flex-grow-1">
            <div class="font-weight-bold">{{ t("settings.sensitivity") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.sensitivity_hint") }}</div>
          </div>
          <v-btn-toggle
            :model-value="s.ai_control.sensitivity"
            mandatory
            variant="outlined"
            @update:model-value="onSensitivityChange"
          >
            <v-btn value="low">{{ t("settings.sens_low") }}</v-btn>
            <v-btn value="medium">{{ t("settings.sens_medium") }}</v-btn>
            <v-btn value="high">{{ t("settings.sens_high") }}</v-btn>
          </v-btn-toggle>
        </div>

        <div class="setting-row">
          <div class="flex-grow-1">
            <div class="font-weight-bold">
              {{ t("settings.cooldown") }}
              <span class="st-mono ml-2">{{ (s.ai_control.cooldown_ms / 1000).toFixed(1) }}s</span>
            </div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.cooldown_hint") }}</div>
          </div>
          <v-slider
            v-model="s.ai_control.cooldown_ms"
            :min="500"
            :max="3000"
            :step="100"
            show-ticks="always"
            tick-size="4"
            max-width="380"
            thumb-color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.show_preview") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.show_preview_desc") }}</div>
          </div>
          <v-switch
            v-model="s.ai_control.show_preview"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.page_effect") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.page_effect_desc") }}</div>
          </div>
          <v-switch
            v-model="s.ai_control.page_effect"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>
      </template>

      <!-- ============ GP 播放器 ============ -->
      <template v-else-if="active === 'gp'">
        <div class="st-display text-h3 mb-8">{{ t("settings.tab_gp") }}</div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.gp_stave") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.gp_stave_desc") }}</div>
          </div>
          <v-switch
            v-model="s.gp_player.show_standard_notation"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.gp_chords") }}</div>
          </div>
          <v-switch
            v-model="s.gp_player.show_chord_names"
            color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">
            {{ t("settings.gp_zoom") }}
            <span class="st-mono ml-2">{{ s.gp_player.zoom.toFixed(2) }}x</span>
          </div>
          <v-slider
            v-model="s.gp_player.zoom"
            :min="0.5"
            :max="2"
            :step="0.05"
            show-ticks="always"
            tick-size="4"
            max-width="380"
            thumb-color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.gp_count_in") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.gp_count_in_desc") }}</div>
          </div>
          <v-btn-toggle
            v-model="s.gp_player.count_in_beats"
            mandatory
            variant="outlined"
            @update:model-value="saveNow"
          >
            <v-btn :value="0">{{ t("common.off") }}</v-btn>
            <v-btn :value="1">1</v-btn>
            <v-btn :value="2">2</v-btn>
            <v-btn :value="4">4</v-btn>
          </v-btn-toggle>
        </div>

        <div class="setting-row">
          <div>
            <div class="font-weight-bold">{{ t("settings.gp_scroll") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.gp_scroll_desc") }}</div>
          </div>
          <v-select
            v-model="s.gp_player.scroll_mode"
            :items="[
              { title: t('gp.scroll_smooth'), value: 'smooth' },
              { title: t('gp.scroll_continuous'), value: 'continuous' },
              { title: t('gp.scroll_offscreen'), value: 'off_screen' },
              { title: t('gp.scroll_off'), value: 'off' },
            ]"
            density="comfortable"
            variant="underlined"
            hide-details
            max-width="220"
            @update:model-value="saveNow"
          />
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">
            {{ t("settings.gp_metronome_vol") }}
            <span class="st-mono ml-2">{{ s.gp_player.metronome_volume }}</span>
          </div>
          <v-slider
            v-model="s.gp_player.metronome_volume"
            :min="0"
            :max="100"
            :step="5"
            max-width="380"
            thumb-color="primary"
            hide-details
            @update:model-value="saveNow"
          />
        </div>
      </template>

      <!-- ============ 快捷键 ============ -->
      <template v-else-if="active === 'shortcuts'">
        <div class="st-display text-h3 mb-8">{{ t("settings.tab_shortcuts") }}</div>

        <div class="shortcut-row">
          <span class="text-body-1 font-weight-bold">{{ t("settings.shortcut_next") }}</span>
          <ShortcutInput
            :model-value="s.shortcuts.next_page"
            @update:model-value="(v) => setShortcut('next_page', v)"
          />
        </div>
        <div class="shortcut-row">
          <span class="text-body-1 font-weight-bold">{{ t("settings.shortcut_prev") }}</span>
          <ShortcutInput
            :model-value="s.shortcuts.prev_page"
            @update:model-value="(v) => setShortcut('prev_page', v)"
          />
        </div>
        <div class="shortcut-row">
          <span class="text-body-1 font-weight-bold">{{ t("settings.shortcut_fullscreen") }}</span>
          <ShortcutInput
            :model-value="s.shortcuts.toggle_fullscreen"
            @update:model-value="(v) => setShortcut('toggle_fullscreen', v)"
          />
        </div>
        <div class="shortcut-row">
          <span class="text-body-1 font-weight-bold">{{ t("settings.shortcut_autoscroll") }}</span>
          <ShortcutInput
            :model-value="s.shortcuts.toggle_autoscroll"
            @update:model-value="(v) => setShortcut('toggle_autoscroll', v)"
          />
        </div>
      </template>

      <!-- ============ 关于 ============ -->
      <template v-else>
        <div class="st-display text-h3 mb-2">{{ t("settings.tab_about") }}</div>
        <div class="st-mono mb-8">FINGERTIP TABS</div>

        <div class="d-flex align-center ga-4 mb-8">
          <div class="brand-big">
            <img src="/logo.svg" alt="Logo" class="brand-logo-img" />
          </div>
          <div>
            <div class="st-display text-h5">{{ t("app.name") }}</div>
            <div class="text-body-2" style="opacity: 0.5">{{ t("settings.about_desc") }}</div>
          </div>
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.version") }}</div>
          <div class="d-flex align-center ga-3">
            <span class="st-chip-coral">V{{ version }}</span>
            <v-btn variant="tonal" size="small" prepend-icon="mdi-cloud-download-outline" @click="openReleases">
              {{ t("settings.check_update") }}
            </v-btn>
          </div>
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.license") }}</div>
          <span class="st-mono">MIT License</span>
        </div>

        <div class="setting-row">
          <div class="font-weight-bold">{{ t("settings.help") }}</div>
          <v-btn variant="tonal" size="small" prepend-icon="mdi-book-open-outline" @click="openHelp">
            {{ t("settings.help") }}
          </v-btn>
        </div>
      </template>
    </div>

    <!-- ============ 弹窗 ============ -->

    <!-- 修改存储路径确认 -->
    <ConfirmDialog
      v-model="migrateOpen"
      :title="t('settings.migrate_confirm_title')"
      :text="t('settings.migrate_confirm_text')"
      danger
      @confirm="doMigrate"
    />
    <!-- 恢复备份确认 -->
    <ConfirmDialog
      v-model="restoreOpen"
      :title="t('settings.restore_backup')"
      :text="t('settings.restore_confirm_text')"
      danger
      @confirm="doRestore"
    />
    <!-- 恢复默认确认 -->
    <ConfirmDialog
      v-model="resetOpen"
      :title="t('settings.reset')"
      :text="t('settings.reset_confirm_text')"
      danger
      @confirm="doReset"
    />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "@/lib/tauri";
import { t } from "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { useToastStore, errText } from "@/stores/toast";
import ConfirmDialog from "@/components/ConfirmDialog.vue";
import ShortcutInput from "@/components/ShortcutInput.vue";
import WindowControls from "@/components/WindowControls.vue";
import { listCameras } from "@/lib/headshake";

const router = useRouter();
const settings = useSettingsStore();
const toast = useToastStore();

const s = settings.settings;

const sections = computed(() => [
  { value: "general", icon: "mdi-tune-vertical-variant", title: t("settings.tab_general") },
  { value: "display", icon: "mdi-television-classic", title: t("settings.tab_display") },
  { value: "ai", icon: "mdi-motion-sensor", title: t("settings.tab_ai") },
  { value: "gp", icon: "mdi-guitar-electric", title: t("settings.tab_gp") },
  { value: "shortcuts", icon: "mdi-keyboard", title: t("settings.tab_shortcuts") },
  { value: "about", icon: "mdi-information-outline", title: t("settings.tab_about") },
]);
const active = ref("general");

const BG_PRESETS = ["#000000", "#1A1A1A", "#333333", "#F5F5F0", "#FDF6E3"];

// ---------------------------------------------------------------------------
// 通用
// ---------------------------------------------------------------------------

function saveNow() {
  settings.save();
  toast.success(t("settings.saved"));
}

function onLanguageChange() {
  settings.applyLocale();
  saveNow();
}

const migrateOpen = ref(false);
const migrating = ref(false);

async function doMigrate() {
  const picked = await open({ directory: true, multiple: false, title: t("settings.migrate_confirm_title") });
  if (!picked || typeof picked !== "string") return;
  migrating.value = true;
  toast.push(t("settings.migrating"), "info", 15000);
  try {
    await api.migrateLibrary(picked);
    await api.restartApp();
  } catch (e) {
    toast.error(t("settings.migrate_failed", { msg: errText(e) }));
  } finally {
    migrating.value = false;
  }
}

const backing = ref(false);
async function doBackup() {
  const dest = await save({
    title: t("settings.backup"),
    defaultPath: `FingertipTabs-backup-${new Date().toISOString().slice(0, 10)}.zip`,
    filters: [{ name: "ZIP", extensions: ["zip"] }],
  });
  if (!dest) return;
  backing.value = true;
  try {
    const result = await api.backupLibrary(dest);
    toast.success(t("settings.backup_done", { path: result }));
  } catch (e) {
    toast.error(errText(e));
  } finally {
    backing.value = false;
  }
}

const restoreOpen = ref(false);
async function doRestore() {
  const picked = await open({
    multiple: false,
    filters: [{ name: "ZIP", extensions: ["zip"] }],
    title: t("settings.restore_backup"),
  });
  if (!picked || typeof picked !== "string") return;
  try {
    await api.restoreLibrary(picked);
    await api.restartApp();
  } catch (e) {
    toast.error(errText(e));
  }
}

const resetOpen = ref(false);
async function doReset() {
  await settings.resetToDefaults();
  toast.success(t("settings.saved"));
}

// ---------------------------------------------------------------------------
// 显示
// ---------------------------------------------------------------------------

function onThemeChange(v: unknown) {
  s.appearance.theme = String(v);
  settings.applyThemeClass();
  saveNow();
}

function setBg(color: string) {
  s.appearance.bg_color = color;
  saveNow();
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

const cameras = ref<MediaDeviceInfo[]>([]);
const probing = ref(false);
const cameraItems = computed(() =>
  cameras.value.map((c, i) => ({ title: c.label || `Camera ${i + 1}`, value: i })),
);

async function probeCameras() {
  probing.value = true;
  try {
    cameras.value = await listCameras();
  } finally {
    probing.value = false;
  }
}

function onSensitivityChange(v: unknown) {
  s.ai_control.sensitivity = String(v);
  saveNow();
}

// ---------------------------------------------------------------------------
// 快捷键
// ---------------------------------------------------------------------------

function setShortcut(key: keyof typeof s.shortcuts, combo: string) {
  s.shortcuts[key] = combo;
  saveNow();
}

// ---------------------------------------------------------------------------
// 关于
// ---------------------------------------------------------------------------

const version = ref("0.3.0");
onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch {
    version.value = "0.3.0";
  }
  void probeCameras();
});

const UPDATE_URL = "https://github.com/QinCongH/fingertip-tab/tags";
const HELP_URL = "https://github.com/QinCongH/fingertip-tab/blob/main/README.md";

function openReleases() {
  void openUrl(UPDATE_URL);
}
function openHelp() {
  void openUrl(HELP_URL);
}
</script>

<style scoped>
.settings-root {
  height: 100vh;
}
.settings-titlebar {
  height: 42px;
  background: var(--st-deep);
  border-bottom: 2px solid var(--st-border);
}
.side {
  width: 300px;
  flex-shrink: 0;
  background: var(--st-deep);
  border-right: 2px solid var(--st-border);
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  padding: 18px 0;
  min-height: 72px;
}
.shortcut-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0;
  border-bottom: 1px solid var(--st-border);
}
.swatch {
  width: 30px;
  height: 30px;
  border: 2px solid var(--st-border);
  cursor: pointer;
  transition: all 0.3s ease;
  position: relative;
}
.swatch:hover,
.swatch.active {
  border-color: var(--st-coral);
  transform: scale(1.08);
}
.swatch.custom {
  background: conic-gradient(#ff6b6b, #f5a623, #4caf7d, #5b8def, #ff6b6b);
  overflow: hidden;
}
.color-input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}
.brand-big {
  width: 64px;
  height: 64px;
  background: var(--st-gray-900);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  padding: 8px;
  box-sizing: border-box;
}
.brand-logo-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
}
</style>
