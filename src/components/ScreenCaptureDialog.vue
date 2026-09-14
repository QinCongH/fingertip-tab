<template>
  <v-dialog v-model="visible" fullscreen>
    <v-card class="capture-card d-flex flex-column">
      <!-- 顶栏 -->
      <div class="d-flex align-center px-5 py-3 bg-deep">
        <div class="st-display text-h6">{{ t("screenshot.title") }}</div>
        <v-spacer />
        <span class="text-body-2 mr-4" style="opacity: 0.55">{{ t("screenshot.tip") }}</span>
        <v-btn
          variant="tonal"
          size="small"
          prepend-icon="mdi-monitor-screenshot"
          :loading="capturing"
          class="mr-2"
          @click="capture"
        >
          {{ t("screenshot.capture") }}
        </v-btn>
        <v-btn icon="mdi-close" variant="text" size="small" @click="close" />
      </div>

      <!-- 截图区 -->
      <div class="flex-grow-1 d-flex align-center justify-center overflow-hidden pa-4" style="position: relative">
        <div v-if="!shot" class="text-center">
          <v-icon icon="mdi-monitor-screenshot" size="64" style="opacity: 0.3" />
          <div class="text-body-2 mt-3" style="opacity: 0.5">{{ t("viewer.loading") }}</div>
        </div>
        <div
          v-else
          ref="stage"
          class="shot-stage"
          @pointerdown="onDown"
          @pointermove="onMove"
          @pointerup="onUp"
        >
          <img :src="shot.data" class="shot-img" draggable="false" />
          <div class="dim-mask" :style="maskStyle"></div>
          <div v-if="sel" class="sel-box" :style="selStyle"></div>
        </div>
      </div>

      <!-- 底栏 -->
      <div class="d-flex align-center px-5 py-3 bg-deep">
        <span class="st-mono" v-if="shot">{{ shot.width }} × {{ shot.height }}</span>
        <v-spacer />
        <v-btn variant="text" class="mr-2" @click="close">{{ t("common.cancel") }}</v-btn>
        <v-btn color="primary" variant="flat" :disabled="!sel" @click="crop">
          {{ t("screenshot.ok") }}
        </v-btn>
      </div>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type CaptureResult } from "@/lib/tauri";
import { t } from "@/i18n";
import { useToastStore, errText } from "@/stores/toast";

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
  (e: "cropped", dataUrl: string): void;
}>();

const toast = useToastStore();
const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit("update:modelValue", v),
});

const shot = ref<CaptureResult | null>(null);
const capturing = ref(false);
const stage = ref<HTMLElement | null>(null);

// 选区（显示坐标）
const sel = ref<{ x: number; y: number; w: number; h: number } | null>(null);
let dragging = false;
let startX = 0;
let startY = 0;

async function capture() {
  capturing.value = true;
  sel.value = null;
  const win = getCurrentWindow();
  let hidden = false;
  try {
    // 隐藏本窗口再截屏，避免把应用自身截进图里
    await win.hide();
    hidden = true;
    await new Promise((r) => setTimeout(r, 420));
    shot.value = await api.captureScreen();
  } catch (e) {
    toast.error(`${t("screenshot.failed")} — ${errText(e)}`);
  } finally {
    if (hidden) {
      await win.show();
      await win.setFocus().catch(() => undefined);
    }
    capturing.value = false;
  }
}

watch(
  () => props.modelValue,
  (v) => {
    if (v) void capture();
  },
);

function stageRect(): DOMRect | null {
  return stage.value?.getBoundingClientRect() ?? null;
}

function localPoint(e: PointerEvent) {
  const rect = stageRect();
  if (!rect) return { x: 0, y: 0 };
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function onDown(e: PointerEvent) {
  if (!shot.value) return;
  const p = localPoint(e);
  dragging = true;
  startX = p.x;
  startY = p.y;
  sel.value = { x: p.x, y: p.y, w: 0, h: 0 };
}

function onMove(e: PointerEvent) {
  if (!dragging) return;
  const p = localPoint(e);
  sel.value = {
    x: Math.min(startX, p.x),
    y: Math.min(startY, p.y),
    w: Math.abs(p.x - startX),
    h: Math.abs(p.y - startY),
  };
}

function onUp() {
  dragging = false;
  // 过滤误触的极小选区
  if (sel.value && (sel.value.w < 8 || sel.value.h < 8)) sel.value = null;
}

const maskStyle = computed(() => {
  if (!sel.value) return { opacity: 0.45 };
  return {
    clipPath: `polygon(0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${sel.value.x}px ${sel.value.y}px, ${sel.value.x}px ${sel.value.y + sel.value.h}px, ${sel.value.x + sel.value.w}px ${sel.value.y + sel.value.h}px, ${sel.value.x + sel.value.w}px ${sel.value.y}px, ${sel.value.x}px ${sel.value.y}px)`,
    opacity: 0.5,
  };
});

const selStyle = computed(() => {
  if (!sel.value) return {};
  const s = sel.value;
  return {
    left: `${s.x}px`,
    top: `${s.y}px`,
    width: `${s.w}px`,
    height: `${s.h}px`,
  };
});

function crop() {
  if (!shot.value || !sel.value || !stage.value) return;
  const img = stage.value.querySelector("img") as HTMLImageElement | null;
  if (!img?.naturalWidth) return;

  const displayW = img.clientWidth;
  const displayH = img.clientHeight;
  const scaleX = img.naturalWidth / displayW;
  const scaleY = img.naturalHeight / displayH;

  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(sel.value.w * scaleX));
  canvas.height = Math.max(1, Math.round(sel.value.h * scaleY));
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(
    img,
    sel.value.x * scaleX,
    sel.value.y * scaleY,
    sel.value.w * scaleX,
    sel.value.h * scaleY,
    0,
    0,
    canvas.width,
    canvas.height,
  );
  const dataUrl = canvas.toDataURL("image/jpeg", 0.92);
  emit("cropped", dataUrl);
  close();
}

function close() {
  visible.value = false;
  shot.value = null;
  sel.value = null;
}
</script>

<style scoped>
.capture-card {
  background: var(--st-black) !important;
}
.shot-stage {
  position: relative;
  max-width: 100%;
  max-height: 100%;
  cursor: crosshair;
  user-select: none;
  border: 2px solid var(--st-border);
}
.shot-img {
  display: block;
  max-width: 100%;
  max-height: calc(100vh - 140px);
}
.dim-mask {
  position: absolute;
  inset: 0;
  background: #0d0d0d;
  pointer-events: none;
}
.sel-box {
  position: absolute;
  border: 2px solid var(--st-coral);
  box-shadow: 0 0 0 9999px rgba(13, 13, 13, 0.001);
  pointer-events: none;
}
</style>
