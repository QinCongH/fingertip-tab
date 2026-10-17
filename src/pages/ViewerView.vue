<template>
  <div
    class="viewer-root"
    :style="{ background: bg }"
    @mousemove="onMouseMove"
  >
    <!-- 顶部悬浮工具栏 -->
    <div
      ref="topBarEl"
      class="st-viewer-bar st-viewer-bar--top"
      :class="{ 'st-viewer-bar--hidden': barsHidden }"
      data-tauri-drag-region
    >
      <div class="d-flex align-center px-3 ga-1" style="height: 56px" data-tauri-drag-region>
        <div class="d-flex align-center flex-grow-1 min-width-0" data-tauri-drag-region>
          <v-btn icon="mdi-arrow-left" variant="text" :title="t('viewer.back')" style="-webkit-app-region: no-drag" @click="goBack" />
          <div class="min-width-0 ml-1" data-tauri-drag-region>
            <div class="font-weight-black text-truncate" style="font-size: 15px" data-tauri-drag-region>
              {{ song?.title || "…" }}
            </div>
            <div class="st-mono text-truncate" data-tauri-drag-region>
              {{ song?.artist }} · {{ modeTitle }}
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-1" style="-webkit-app-region: no-drag">
          <template v-if="!isGp">
          <v-btn-toggle v-model="mode" mandatory density="comfortable" class="mr-2" variant="outlined">
            <v-btn v-for="m in modeItems" :key="m.value" :value="m.value">
              <v-icon size="20">{{ m.icon }}</v-icon>
              <v-tooltip activator="parent" location="bottom">{{ m.title }}</v-tooltip>
            </v-btn>
          </v-btn-toggle>

          <v-select
            v-model="zoomMode"
            :items="zoomItems"
            hide-details
            density="compact"
            variant="underlined"
            max-width="150"
            class="mr-1"
          />
          </template>
          <v-chip v-else size="small" variant="tonal" color="primary" class="mr-2" prepend-icon="mdi-music-note-sixteenth">
            GP
          </v-chip>

          <!-- 背景颜色设置 -->
          <v-menu :close-on-content-click="false" location="bottom end">
            <template #activator="{ props: menuProps }">
              <v-btn
                v-bind="menuProps"
                icon="mdi-palette"
                variant="text"
                :title="t('viewer.bg_color')"
                class="mr-1"
              >
                <v-icon :style="{ color: bg }">mdi-circle</v-icon>
              </v-btn>
            </template>
            <div class="st-bg-menu pa-3">
              <div class="text-caption font-weight-bold mb-2">{{ t("viewer.bg_color") }}</div>
              <div class="d-flex align-center ga-2 mb-2">
                <div
                  v-for="c in BG_PRESETS"
                  :key="c"
                  class="viewer-swatch"
                  :class="{ active: bg.toLowerCase() === c.toLowerCase() }"
                  :style="{ background: c }"
                  @click="setBgColor(c)"
                />
              </div>
              <div class="d-flex align-center ga-2 pt-1 border-t-2" style="border-color: rgba(255,255,255,0.1)">
                <span class="st-mono text-caption">{{ bg }}</span>
                <v-spacer />
                <label class="viewer-swatch custom d-flex align-center justify-center">
                  <v-icon icon="mdi-eyedropper-variant" size="14" />
                  <input
                    type="color"
                    class="color-input"
                    :value="bg"
                    @input="setBgColor(($event.target as HTMLInputElement).value)"
                  />
                </label>
              </div>
            </div>
          </v-menu>

          <v-btn
            v-if="!isGp"
            :icon="aiEnabled ? 'mdi-motion-sensor' : 'mdi-motion-sensor-off'"
            :variant="aiEnabled ? 'flat' : 'text'"
            :color="aiEnabled ? 'primary' : undefined"
            :title="t('viewer.ai')"
            class="ml-1"
            @click="toggleAI"
          />
          <!-- 局域网共享 -->
          <v-btn
            icon="mdi-qrcode"
            :variant="sharing ? 'flat' : 'text'"
            :color="sharing ? 'primary' : undefined"
            :title="t('share.title')"
            class="ml-1"
            @click="shareOpen = !shareOpen"
          />
          <v-btn
            :icon="fullscreen ? 'mdi-fullscreen-exit' : 'mdi-fullscreen'"
            variant="text"
            :title="t('viewer.fullscreen')"
            @click="toggleFullscreen"
          />
        </div>

        <div class="ml-2 flex-shrink-0" style="-webkit-app-region: no-drag">
          <WindowControls />
        </div>
      </div>
    </div>

    <!-- 局域网共享弹窗 -->
    <v-dialog v-model="shareOpen" max-width="380">
      <div class="share-dialog">
        <div class="d-flex align-center mb-3">
          <span class="text-body-1 font-weight-black">{{ t("share.title") }}</span>
          <v-spacer />
          <v-btn icon="mdi-close" variant="text" size="small" @click="closeShare" />
        </div>
        <template v-if="shareInfo">
          <div class="share-qr-wrap">
            <img v-if="qrDataUrl" :src="qrDataUrl" alt="QR" class="share-qr" />
            <div v-else class="share-qr-empty">{{ t("share.generating") }}</div>
          </div>
          <div class="st-mono text-caption mt-3 text-center" style="word-break: break-all; opacity: 0.7">
            {{ shareInfo.url }}
          </div>
          <div class="text-caption mt-3 text-center" style="opacity: 0.6">
            {{ t("share.hint") }}
          </div>
          <div class="d-flex justify-center ga-2 mt-4">
            <v-btn variant="tonal" size="small" @click="copyShareUrl">
              {{ t("share.copy") }}
            </v-btn>
            <v-btn color="primary" variant="flat" size="small" @click="closeShare">
              {{ t("share.stop") }}
            </v-btn>
          </div>
        </template>
        <div v-else class="text-center py-6">
          <v-progress-circular indeterminate color="primary" size="36" />
          <div class="text-caption mt-3" style="opacity: 0.6">{{ t("share.starting") }}</div>
        </div>
      </div>
    </v-dialog>

    <!-- 内容区 -->
    <div
      v-if="song"
      class="viewer-content"
      :style="contentBoxStyle"
    >
      <!-- GP 曲谱：AlphaTab 查看器 -->
      <GpViewer v-if="isGp" :song="song" :bg="bg" :bars-hidden="barsHidden" />

      <template v-else>
      <!-- 模式 1：固定翻页 -->
      <div
        v-if="mode === 'fixed'"
        ref="fixedContainerEl"
        class="mode-fixed"
        @click="onFixedClick"
        @wheel="onFixedWheel"
      >
        <Transition name="page-flip" mode="out-in">
          <img
            ref="fixedImgEl"
            :key="currentUuid"
            :src="currentSrc"
            class="page-img"
            :style="zoomStyle"
            draggable="false"
          />
        </Transition>
        <div class="click-zone" style="left: 0; width: 38%"></div>
        <div class="click-zone" style="right: 0; width: 38%"></div>

        <!-- 屏幕左右两侧悬浮拖拽拉伸手柄（hover 时显示，支持双击恢复自适应） -->
        <div
          class="fixed-screen-handle fixed-screen-handle--left"
          :class="{ 'fixed-screen-handle--active': isResizingFixed }"
          :title="t('viewer.width_drag_hint')"
          @mousedown.stop.prevent="startFixedDragResize($event, 'left')"
          @dblclick.stop.prevent="resetFixedSheetWidth"
        >
          <div class="fixed-screen-handle-bar">
            <v-icon icon="mdi-drag-vertical" size="18" />
          </div>
        </div>

        <div
          class="fixed-screen-handle fixed-screen-handle--right"
          :class="{ 'fixed-screen-handle--active': isResizingFixed }"
          :title="t('viewer.width_drag_hint')"
          @mousedown.stop.prevent="startFixedDragResize($event, 'right')"
          @dblclick.stop.prevent="resetFixedSheetWidth"
        >
          <div class="fixed-screen-handle-bar">
            <v-icon icon="mdi-drag-vertical" size="18" />
          </div>
        </div>
      </div>

      <!-- 模式 2：垂直滚动 -->
      <div
        v-else-if="mode === 'scroll'"
        ref="scrollEl"
        class="mode-scroll"
        @scroll="onScroll"
      >
        <img
          v-for="p in pages"
          :key="p.uuid"
          :src="pageSrcOf(p)"
          class="page-img"
          :style="scrollImgStyle"
          draggable="false"
        />
      </div>

      <!-- 模式 3：多页横向对齐（自适应满高居中横向排布） -->
      <div v-else ref="dualEl" class="mode-dual" @wheel.prevent="onDualWheel" @scroll="onScroll">
        <div v-for="(p, pi) in pages" :key="p.uuid" class="dual-item">
          <img
            :src="pageSrcOf(p)"
            class="dual-img"
            :alt="'page ' + (pi + 1)"
            draggable="false"
          />
        </div>
      </div>
      </template>
    </div>

    <div v-else class="not-found" :style="contentBoxStyle">
      <div class="st-display text-h4">{{ loaded ? t("viewer.not_found") : t("viewer.loading") }}</div>
    </div>

    <!-- 底部悬浮工具栏（图片谱专用，GP 由 GpViewer 自带控制条） -->
    <div
      v-if="song && !isGp"
      ref="bottomBarEl"
      class="st-viewer-bar st-viewer-bar--bottom"
      :class="{ 'st-viewer-bar--hidden': barsHidden }"
    >
      <div class="d-flex align-center px-4 ga-3" style="height: 64px">
        <!-- 翻页模式 -->
        <template v-if="mode === 'fixed'">
          <v-btn
            icon="mdi-chevron-left"
            variant="tonal"
            :title="t('viewer.prev')"
            :disabled="pageIndex === 0"
            @click.stop="flip(-1)"
          />
          <v-btn
            icon="mdi-chevron-right"
            variant="tonal"
            :title="t('viewer.next')"
            :disabled="pageIndex >= pages.length - 1"
            @click.stop="flip(1)"
          />
        </template>

        <!-- 自动滚动（滚动/多页模式） -->
        <template v-else>
          <v-btn
            :icon="playing ? 'mdi-pause' : 'mdi-play'"
            color="primary"
            :variant="playing ? 'flat' : 'tonal'"
            :title="playing ? t('viewer.pause') : t('viewer.play')"
            @click="togglePlay"
          />
          <template v-if="speedMode === 'px'">
            <span class="st-mono" style="width: 50px">{{ t("viewer.speed") }}</span>
            <v-slider
              v-model="speedPx"
              :min="1"
              :max="300"
              :step="0.5"
              hide-details
              density="compact"
              max-width="200"
              thumb-color="primary"
              @update:model-value="onSpeedChange"
            >
              <template #thumb-label="{ modelValue }">{{ modelValue }} {{ t("viewer.speed_unit") }}</template>
            </v-slider>
            <v-text-field
              v-model.number="speedPx"
              type="number"
              :min="1"
              :max="300"
              :step="0.5"
              suffix="px/s"
              density="compact"
              variant="underlined"
              hide-details
              max-width="110"
              @update:model-value="onSpeedChange"
            />
          </template>
          <template v-else>
            <v-text-field
              v-model.number="bpm"
              :label="t('viewer.bpm_mode')"
              type="number"
              :min="1"
              :max="300"
              :step="1"
              hide-details
              density="compact"
              variant="underlined"
              max-width="90"
              @update:model-value="onSpeedChange"
            />
            <span class="st-mono" style="max-width: 240px">{{ t("viewer.bpm_hint") }}</span>
          </template>
          <v-btn-toggle v-model="speedMode" mandatory density="compact" variant="outlined">
            <v-btn value="px">{{ t("viewer.speed_unit") }}</v-btn>
            <v-btn value="bpm">BPM</v-btn>
          </v-btn-toggle>
        </template>

        <v-spacer />

        <!-- 页码指示 -->
        <div class="page-indicator st-mono">
          {{ t("viewer.page_indicator", { current: displayPage + 1, total: pages.length }) }}
        </div>
      </div>
    </div>

    <!-- 摄像头预览小窗 -->
    <div v-if="aiEnabled && showPreview" class="st-camera-pip">
      <video ref="pipEl" autoplay muted playsinline class="pip-video"></video>
      <div class="d-flex align-center justify-space-between px-2 py-1">
        <span class="st-mono">AI · {{ t("viewer.ai") }}</span>
        <span class="live-dot"></span>
      </div>
    </div>

    <!-- AI 翻页成功视觉反馈 -->
    <div v-if="flash" class="st-page-flash"></div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import QRCode from "qrcode";
import { useRoute, useRouter } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type PageDto, type SongDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";
import { useSettingsStore } from "@/stores/settings";
import { useToastStore, errText } from "@/stores/toast";
import {
  HeadShakeDetector,
  listCameras,
  type Sensitivity,
} from "@/lib/headshake";
import { isTypingTarget, matchCombo } from "@/lib/shortcuts";
import WindowControls from "@/components/WindowControls.vue";
import GpViewer from "@/components/GpViewer.vue";

const route = useRoute();
const router = useRouter();
const settings = useSettingsStore();
const library = useLibraryStore();
const toast = useToastStore();
const appWindow = getCurrentWindow();

// ---------------------------------------------------------------------------
// 基础状态
// ---------------------------------------------------------------------------

const songId = Number(route.params.id);
const song = ref<SongDto | null>(null);
const loaded = ref(false);
const isGp = computed(() => song.value?.format === "gp");

// ---------------------------------------------------------------------------
// 局域网共享（手机扫码查看）
// ---------------------------------------------------------------------------

const shareOpen = ref(false);
const sharing = ref(false);
const shareInfo = ref<{ url: string } | null>(null);
const qrDataUrl = ref("");

async function openShare() {
  if (!song.value || shareInfo.value) return;
  sharing.value = true;
  shareInfo.value = null;
  qrDataUrl.value = "";
  try {
    const info = await api.startShare(song.value.id);
    shareInfo.value = info;
    qrDataUrl.value = await QRCode.toDataURL(info.url, {
      width: 240,
      margin: 1,
      color: { dark: "#1A1A1A", light: "#F5F5F0" },
    });
  } catch (e) {
    toast.push(t("share.failed", { msg: errText(e) }), "error");
    shareOpen.value = false;
  } finally {
    sharing.value = false;
  }
}

async function closeShare() {
  shareOpen.value = false;
  if (shareInfo.value) {
    shareInfo.value = null;
    qrDataUrl.value = "";
    try {
      await api.stopShare();
    } catch {
      // 忽略停止失败
    }
  }
}

async function copyShareUrl() {
  if (!shareInfo.value) return;
  try {
    await navigator.clipboard.writeText(shareInfo.value.url);
    toast.push(t("share.copied"), "success");
  } catch {
    // 忽略复制失败
  }
}

watch(shareOpen, (open) => {
  if (open) void openShare();
  else if (shareInfo.value) void closeShare();
});


const mode = ref<"fixed" | "scroll" | "dual_horizontal">(
  (["fixed", "scroll", "dual_horizontal"].includes(settings.settings.appearance.default_view_mode)
    ? settings.settings.appearance.default_view_mode
    : "dual_horizontal") as "fixed" | "scroll" | "dual_horizontal",
);
const zoomMode = ref(settings.settings.appearance.zoom_mode || "fit_height");
const fixedSheetWidth = ref<number>(0); // 0 为按 zoomMode 自适应；>0 为手动拖拽设定的像素宽度
const isResizingFixed = ref(false);
const fixedContainerEl = ref<HTMLElement | null>(null);
const fixedImgEl = ref<HTMLImageElement | null>(null);
const pageIndex = ref(0);
const scrollPageIndex = ref(0);

const pages = computed(() => song.value?.pages ?? []);
const currentUuid = computed(() => pages.value[pageIndex.value]?.uuid ?? "");
const currentSrc = computed(() => {
  const p = pages.value[pageIndex.value];
  return p ? pageSrcOf(p) : "";
});
const displayPage = computed(() => (mode.value === "fixed" ? pageIndex.value : scrollPageIndex.value));

function pageSrcOf(p: PageDto): string {
  return library.pageSrc(p.uuid, p.fileType);
}

const bg = computed(() => settings.settings.appearance.bg_color || "#1A1A1A");
const BG_PRESETS = ["#000000", "#1A1A1A", "#333333", "#F5F5F0", "#FDF6E3"];

function setBgColor(color: string) {
  settings.settings.appearance.bg_color = color;
  settings.save();
}

const showPreview = computed(() => settings.settings.ai_control.show_preview);

const modeItems = computed(() => [
  { value: "fixed" as const, icon: "mdi-book-open-page-variant", title: t("viewer.mode_fixed") },
  { value: "scroll" as const, icon: "mdi-arrow-expand-vertical", title: t("viewer.mode_scroll") },
  { value: "dual_horizontal" as const, icon: "mdi-book-open-blank-variant", title: t("viewer.mode_dual") },
]);

const modeTitle = computed(() =>
  isGp.value ? t("gp.title") : (modeItems.value.find((m) => m.value === mode.value)?.title ?? ""),
);

const zoomItems = computed(() => [
  { title: t("viewer.zoom_fit_width"), value: "fit_width" },
  { title: t("viewer.zoom_fit_height"), value: "fit_height" },
  { title: t("viewer.zoom_actual"), value: "actual" },
]);

// ---------------------------------------------------------------------------
// 顶部 / 底部栏高度与内容区自适应
// ---------------------------------------------------------------------------

const topBarEl = ref<HTMLElement | null>(null);
const bottomBarEl = ref<HTMLElement | null>(null);
const topBarHeight = ref(56);
const bottomBarHeight = ref(64);

let resizeObserver: ResizeObserver | null = null;

function updateBarHeights() {
  if (topBarEl.value) {
    topBarHeight.value = topBarEl.value.offsetHeight;
  }
  if (bottomBarEl.value) {
    bottomBarHeight.value = bottomBarEl.value.offsetHeight;
  }
}

// 实际占位的高度：当全屏且隐藏工具栏时为 0，否则为实际测量高度
const effectiveTopBarHeight = computed(() => (barsHidden.value ? 0 : topBarHeight.value));
const effectiveBottomBarHeight = computed(() => (barsHidden.value ? 0 : bottomBarHeight.value));

const contentHeight = computed(() => {
  return `calc(100vh - ${effectiveTopBarHeight.value + effectiveBottomBarHeight.value}px)`;
});

const contentBoxStyle = computed(() => ({
  height: contentHeight.value,
  marginTop: `${effectiveTopBarHeight.value}px`,
  marginBottom: `${effectiveBottomBarHeight.value}px`,
  "--top-bar-h": `${effectiveTopBarHeight.value}px`,
  "--bottom-bar-h": `${effectiveBottomBarHeight.value}px`,
}));

const zoomStyle = computed(() => {
  if (fixedSheetWidth.value > 0) {
    return {
      width: `${fixedSheetWidth.value}px`,
      maxWidth: "100%",
      height: "auto",
      maxHeight: "none",
    };
  }
  switch (zoomMode.value) {
    case "fit_height":
      return { height: contentHeight.value, width: "auto", maxWidth: "none", objectFit: "contain" as const };
    case "actual":
      return { maxWidth: "none", maxHeight: "none" };
    default:
      return { width: "100%", height: "auto", maxWidth: "none", maxHeight: "none" };
  }
});

/** 滚动模式：缩放样式作用于每张图片（容器本身必须保持 contentHeight 可滚动） */
const scrollImgStyle = computed(() => {
  switch (zoomMode.value) {
    case "fit_height":
      return { height: contentHeight.value, width: "auto", maxWidth: "none", objectFit: "contain" as const };
    case "actual":
      return { maxWidth: "none", maxHeight: "none" };
    default:
      return { width: "100%", height: "auto", maxWidth: "none" };
  }
});

// ---------------------------------------------------------------------------
// 数据加载
// ---------------------------------------------------------------------------

onMounted(async () => {
  await library.loadCollections();
  song.value = await api.getSong(songId).catch(() => null);
  loaded.value = true;
  try {
    const cfg = await api.getAppConfig();
    await api.saveAppConfig({ ...cfg, lastSongId: song.value ? songId : null });
  } catch {
    // 忽略
  }
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", updateBarHeights);

  if (typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => {
      updateBarHeights();
    });
    if (topBarEl.value) resizeObserver.observe(topBarEl.value);
    if (bottomBarEl.value) resizeObserver.observe(bottomBarEl.value);
  }
  updateBarHeights();
});

onUnmounted(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
  window.removeEventListener("resize", updateBarHeights);
  stopDetector();
  pauseScroll();
  window.removeEventListener("keydown", onKeydown);
  if (fullscreen.value) void appWindow.setFullscreen(false);
  // 离开看谱页时停止局域网共享
  if (shareInfo.value) {
    shareInfo.value = null;
    void api.stopShare().catch(() => undefined);
  }
  // 离开看谱页时清除"最后打开"记录
  void api
    .getAppConfig()
    .then((cfg) => api.saveAppConfig({ ...cfg, lastSongId: null }))
    .catch(() => undefined);
});

// ---------------------------------------------------------------------------
// 翻页
// ---------------------------------------------------------------------------

function flip(dir: 1 | -1) {
  const next = pageIndex.value + dir;
  if (next < 0 || next >= pages.value.length) return;
  pageIndex.value = next;
}

function onFixedClick(e: MouseEvent) {
  if (isResizingFixed.value) return;
  const half = window.innerWidth / 2;
  flip(e.clientX < half ? -1 : 1);
}

// ---------------------------------------------------------------------------
// 翻页模式边缘拖拽拉伸 & 滚轮交互
// ---------------------------------------------------------------------------

function startFixedDragResize(e: MouseEvent, side: "left" | "right") {
  isResizingFixed.value = true;
  document.body.style.cursor = "ew-resize";
  document.body.style.userSelect = "none";

  const startX = e.clientX;
  const initialWidth = fixedImgEl.value?.clientWidth || fixedContainerEl.value?.clientWidth || 900;
  const maxW = (fixedContainerEl.value?.clientWidth || window.innerWidth) - 20;

  const onMouseMove = (ev: MouseEvent) => {
    const delta = (ev.clientX - startX) * (side === "right" ? 2 : -2);
    const newWidth = Math.min(maxW, Math.max(400, Math.round(initialWidth + delta)));
    if (newWidth >= maxW - 40) {
      fixedSheetWidth.value = 0; // 贴近视口边界时吸附恢复铺满/自适应
    } else {
      fixedSheetWidth.value = newWidth;
    }
  };

  const onMouseUp = () => {
    setTimeout(() => {
      isResizingFixed.value = false;
    }, 50);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  };

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

function resetFixedSheetWidth() {
  fixedSheetWidth.value = 0;
}

function onFixedWheel(e: WheelEvent) {
  if (e.ctrlKey) {
    // 按住 Ctrl 时滚轮缩放谱面宽度
    e.preventDefault();
    const currentW = fixedSheetWidth.value || fixedImgEl.value?.clientWidth || 900;
    const step = e.deltaY < 0 ? 40 : -40;
    const maxW = (fixedContainerEl.value?.clientWidth || window.innerWidth) - 20;
    const nextW = Math.min(maxW, Math.max(400, currentW + step));
    fixedSheetWidth.value = nextW;
    return;
  }
  // 未按住 Ctrl 时：如果图片内容产生了垂直或水平滚动条，允许原生滚动查看谱面细节
}

const scrollEl = ref<HTMLElement | null>(null);
const dualEl = ref<HTMLElement | null>(null);

function scrollByPage(dir: 1 | -1) {
  const el = mode.value === "scroll" ? scrollEl.value : dualEl.value;
  if (!el) return;
  if (mode.value === "scroll") {
    el.scrollBy({ top: dir * el.clientHeight * 0.88, behavior: "smooth" });
  } else {
    el.scrollBy({ left: dir * el.clientWidth, behavior: "smooth" });
  }
}

/** 当前模式下"下一页/上一页"的统一入口（键盘、AI、点击共用） */
function pageTurn(dir: 1 | -1) {
  if (mode.value === "fixed") {
    flip(dir);
  } else {
    scrollByPage(dir);
  }
}

function onDualWheel(e: WheelEvent) {
  const el = dualEl.value;
  if (!el) return;
  // 垂直滚轮转换为水平滚动；按住 Ctrl 时交给缩放
  if (e.deltaY !== 0 && !e.ctrlKey) el.scrollLeft += e.deltaY;
  else if (e.deltaX !== 0) el.scrollLeft += e.deltaX;
}

function onScroll() {
  const el = mode.value === "scroll" ? scrollEl.value : dualEl.value;
  if (!el || pages.value.length < 2) {
    scrollPageIndex.value = 0;
    return;
  }
  const total = pages.value.length;
  const denom =
    mode.value === "scroll" ? el.scrollHeight - el.clientHeight : el.scrollWidth - el.clientWidth;
  const ratio = denom > 0 ? (mode.value === "scroll" ? el.scrollTop : el.scrollLeft) / denom : 0;
  scrollPageIndex.value = Math.min(total - 1, Math.max(0, Math.round(ratio * (total - 1))));
}

// 切模式时复位
watch(mode, async () => {
  pauseScroll();
  pageIndex.value = 0;
  scrollPageIndex.value = 0;
  await nextTick();
  scrollEl.value?.scrollTo({ top: 0 });
  dualEl.value?.scrollTo({ left: 0 });
});

// ---------------------------------------------------------------------------
// 自动滚动
// ---------------------------------------------------------------------------

const playing = ref(false);
const speedMode = ref<"px" | "bpm">("px");
const speedPx = ref(60);
const bpm = ref(80);

function effectiveSpeed(): number {
  if (speedMode.value === "px") return Math.max(0.5, speedPx.value);
  const bpmValue = bpm.value > 0 ? bpm.value : 80;
  // 每分钟 bpm 拍 → 比如 80 BPM 时，每 4 拍对应 3 秒，滚过一屏高度
  return Math.max(0.5, ((bpmValue / 60) * window.innerHeight) / 4);
}

let raf = 0;
let lastTs = 0;
let subpixelPos = 0; // 高精度浮点累积位置，避免小数值被取整丢弃导致低速卡顿

function loop(ts: number) {
  if (!playing.value) return;
  const dt = Math.min(0.1, (ts - lastTs) / 1000);
  lastTs = ts;
  const el = mode.value === "scroll" ? scrollEl.value : dualEl.value;
  if (el) {
    const d = effectiveSpeed() * dt;
    subpixelPos += d;
    if (subpixelPos >= 1) {
      const step = Math.floor(subpixelPos);
      subpixelPos -= step;
      if (mode.value === "scroll") el.scrollTop += step;
      else el.scrollLeft += step;
    }
  }
  raf = requestAnimationFrame(loop);
}

function togglePlay() {
  playing.value ? pauseScroll() : playScroll();
}

function playScroll() {
  if (mode.value === "fixed" || isGp.value) return;
  playing.value = true;
  lastTs = performance.now();
  subpixelPos = 0;
  raf = requestAnimationFrame(loop);
}

function pauseScroll() {
  playing.value = false;
  cancelAnimationFrame(raf);
}

function onSpeedChange() {
  settings.save();
}

// ---------------------------------------------------------------------------
// 全屏
// ---------------------------------------------------------------------------

const fullscreen = ref(false);
const barsHidden = ref(false);
let hideTimer: number | null = null;

function onMouseMove(e: MouseEvent) {
  if (!fullscreen.value) {
    barsHidden.value = false;
    return;
  }
  const nearEdge = e.clientY < 80 || e.clientY > window.innerHeight - 100;
  barsHidden.value = !nearEdge;
  if (hideTimer !== null) window.clearTimeout(hideTimer);
  if (!nearEdge) {
    hideTimer = window.setTimeout(() => (barsHidden.value = true), 2500);
  }
}

async function toggleFullscreen() {
  try {
    const current = await appWindow.isFullscreen();
    fullscreen.value = !current;
    await appWindow.setFullscreen(!current);
    if (hideTimer !== null) window.clearTimeout(hideTimer);
    if (!current) {
      barsHidden.value = false;
      hideTimer = window.setTimeout(() => (barsHidden.value = true), 2500);
    } else {
      barsHidden.value = false;
    }
  } catch (e) {
    toast.error(errText(e));
  }
}

// ---------------------------------------------------------------------------
// AI 摇头翻页
// ---------------------------------------------------------------------------

const aiEnabled = ref(false);
const flash = ref(false);
const pipEl = ref<HTMLVideoElement | null>(null);
let detector: HeadShakeDetector | null = null;

async function toggleAI() {
  if (aiEnabled.value) {
    stopDetector();
    aiEnabled.value = false;
    return;
  }
  const ai = settings.settings.ai_control;
  const devices = await listCameras();
  const device = devices[ai.camera_id] ?? devices[0];
  detector = new HeadShakeDetector({
    sensitivity: (ai.sensitivity as Sensitivity) || "medium",
    cooldownMs: ai.cooldown_ms,
    onTrigger: onAiTrigger,
  });
  try {
    await detector.start(device?.deviceId);
    aiEnabled.value = true;
    toast.success(t("viewer.ai_on"));
    await nextTick();
    if (pipEl.value) detector?.attachPreview(pipEl.value);
  } catch (e) {
    detector.stop();
    detector = null;
    toast.error(`${t("viewer.camera_failed")} — ${errText(e)}`);
  }
}

function stopDetector() {
  detector?.stop();
  detector = null;
}

function onAiTrigger() {
  pageTurn(1);
  if (settings.settings.ai_control.page_effect) {
    flash.value = true;
    window.setTimeout(() => (flash.value = false), 750);
  }
}

// AI 设置即时生效
watch(
  () => settings.settings.ai_control,
  () => {
    if (!detector) return;
    const ai = settings.settings.ai_control;
    detector.updateOptions({
      sensitivity: (ai.sensitivity as Sensitivity) || "medium",
      cooldownMs: ai.cooldown_ms,
    });
  },
  { deep: true },
);

// ---------------------------------------------------------------------------
// 快捷键
// ---------------------------------------------------------------------------

function onKeydown(e: KeyboardEvent) {
  if (isTypingTarget(e)) return;
  const sc = settings.settings.shortcuts;

  if (matchCombo(e, sc.toggle_fullscreen) || (fullscreen.value && e.key === "Escape")) {
    e.preventDefault();
    void toggleFullscreen();
    return;
  }
  if (isGp.value) return; // GP 模式下翻页/自动滚动快捷键由播放器控制
  if (matchCombo(e, sc.next_page)) {
    e.preventDefault();
    pageTurn(1);
    return;
  }
  if (matchCombo(e, sc.prev_page)) {
    e.preventDefault();
    pageTurn(-1);
    return;
  }
  if (matchCombo(e, sc.toggle_autoscroll)) {
    e.preventDefault();
    if (mode.value !== "fixed") togglePlay();
    return;
  }
}

// ---------------------------------------------------------------------------
// 返回
// ---------------------------------------------------------------------------

function goBack() {
  void router.push("/");
}
</script>

<style scoped>
.viewer-root {
  height: 100vh;
  overflow: hidden;
  user-select: none;
}
.min-width-0 {
  min-width: 0;
}
.viewer-content {
  height: 100%;
  box-sizing: border-box;
}
.mode-fixed {
  position: relative;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: auto;
}
.mode-fixed .page-img {
  display: block;
}
/* 图片翻页模式边缘拖拽手柄：默认完全透明隐藏，仅当鼠标悬浮靠近边缘热区或正在拖拽时显示 */
.fixed-screen-handle {
  position: fixed;
  top: 100px;
  bottom: 100px;
  width: 32px;
  z-index: 100;
  cursor: ew-resize;
  user-select: none;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.25s ease;
}
.fixed-screen-handle:hover,
.fixed-screen-handle--active {
  opacity: 1;
}
.fixed-screen-handle--left {
  left: 0;
}
.fixed-screen-handle--right {
  right: 0;
}
.fixed-screen-handle-bar {
  width: 14px;
  height: 56px;
  background: rgba(30, 30, 30, 0.88);
  border: 1px solid rgba(255, 255, 255, 0.35);
  border-radius: 7px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  transition: background 0.2s ease, transform 0.15s ease, width 0.15s ease, border-color 0.2s ease;
}
.fixed-screen-handle:hover .fixed-screen-handle-bar,
.fixed-screen-handle--active .fixed-screen-handle-bar {
  background: #ff6b6b;
  border-color: #ff6b6b;
  width: 18px;
  transform: scale(1.12);
}
.click-zone {
  position: fixed;
  top: var(--top-bar-h, 56px);
  bottom: var(--bottom-bar-h, 64px);
  cursor: pointer;
}
.mode-scroll {
  height: 100%;
  width: 100%;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 0;
  box-sizing: border-box;
}
.mode-scroll .page-img {
  flex-shrink: 0;
  display: block;
  margin: 0 auto;
}
.mode-dual {
  height: 100%;
  width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  justify-content: safe center;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  box-sizing: border-box;
}
.dual-item {
  height: 100%;
  max-height: 100%;
  width: auto;
  max-width: none;
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  position: relative;
}
.dual-img {
  height: 100%;
  max-height: 100%;
  width: auto;
  max-width: none;
  object-fit: contain;
  display: block;
  border-radius: 4px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
}
.not-found {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.6;
}
.page-indicator {
  font-size: 14px;
  opacity: 0.75;
  min-width: 88px;
  text-align: center;
}
.pip-video {
  width: 100%;
  aspect-ratio: 4 / 3;
  object-fit: cover;
  transform: scaleX(-1);
  display: block;
}
.live-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #4caf7d;
  animation: live-blink 1.6s infinite;
}
@keyframes live-blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}
.st-bg-menu {
  background: #1A1A1A;
  border: 2px solid rgba(255, 255, 255, 0.2);
  min-width: 180px;
}
.share-dialog {
  background: #1A1A1A;
  border: 2px solid rgba(255, 255, 255, 0.2);
  padding: 16px 20px 20px;
}
.share-qr-wrap {
  display: flex;
  justify-content: center;
}
.share-qr {
  width: 240px;
  height: 240px;
  display: block;
}
.share-qr-empty {
  width: 240px;
  height: 240px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #333333;
  font-size: 12px;
  opacity: 0.7;
}
.viewer-swatch {
  width: 24px;
  height: 24px;
  cursor: pointer;
  border: 2px solid transparent;
  transition: all 0.2s;
  box-sizing: border-box;
}
.viewer-swatch:hover {
  transform: scale(1.1);
}
.viewer-swatch.active {
  border-color: #FF6B6B;
}
.viewer-swatch.custom {
  background: #333333;
  border: 1px dashed rgba(255, 255, 255, 0.3);
  position: relative;
  overflow: hidden;
}
.color-input {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  cursor: pointer;
}
</style>
