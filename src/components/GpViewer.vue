<template>
  <div class="gp-root" :style="{ background: bg }">
    <!-- 错误态 -->
    <div v-if="phase === 'error'" class="gp-state d-flex flex-column align-center justify-center ga-4">
      <v-icon icon="mdi-file-music-outline" size="56" style="opacity: 0.3" />
      <div class="st-display text-h5">{{ t("gp.load_failed") }}</div>
      <div class="text-body-2" style="opacity: 0.55; max-width: 440px; text-align: center">{{ errorMsg }}</div>
      <div class="d-flex flex-column ga-1 text-body-2" style="opacity: 0.6">
        <span>· {{ t("gp.tip_format") }}</span>
        <span>· {{ t("gp.tip_resave") }}</span>
      </div>
    </div>

    <!-- 加载态遮罩：覆盖在谱面区上，不 unmount 容器 -->
    <div v-if="phase === 'loading'" class="gp-state gp-state--overlay d-flex flex-column align-center justify-center ga-3">
      <v-progress-circular indeterminate color="primary" size="44" />
      <div class="st-mono">{{ t("gp.loading") }}</div>
      <div v-if="soundFontProgress > 0" class="st-mono text-caption" style="opacity: 0.45">
        {{ t("gp.soundfont") }} {{ Math.round(soundFontProgress * 100) }}%
      </div>
    </div>

    <!-- 谱面与控制条始终渲染（loading 时由遮罩盖住），保证 atEl 始终存在 -->
    <!-- 谱面区 -->
    <div ref="scrollEl" class="gp-surface">
      <div ref="atEl" class="gp-alphatab"></div>
    </div>

      <!-- 播放位置高亮条（进度条上的循环区段通过 slider track 无法表达，用 overlay） -->
      <div class="gp-bottom st-viewer-bar st-viewer-bar--bottom" :class="{ 'st-viewer-bar--hidden': barsHidden }">
        <div class="d-flex align-center px-4 ga-3" style="height: 72px">
          <v-btn
            icon="mdi-stop"
            variant="tonal"
            :title="t('gp.stop')"
            :disabled="!playerReady"
            @click="stop"
          />
          <v-btn
            :icon="playing ? 'mdi-pause' : 'mdi-play'"
            color="primary"
            variant="flat"
            :title="playing ? t('gp.pause') : t('gp.play')"
            :disabled="!playerReady"
            @click="togglePlay"
          />

          <!-- 进度条 -->
          <div class="gp-progress flex-grow-1 d-flex align-center ga-2">
            <span class="st-mono text-caption flex-shrink-0">{{ fmtTime(currentTime) }}</span>
            <div class="gp-slider-wrap flex-grow-1">
              <v-slider
                :model-value="currentTime"
                :max="Math.max(1, endTime)"
                :step="50"
                hide-details
                density="compact"
                thumb-size="12"
                track-size="4"
                thumb-color="primary"
                @update:model-value="seek"
              />
              <div
                v-if="loopEnabled && endTime > 0"
                class="gp-loop-range"
                :style="loopRangeStyle"
              />
            </div>
            <span class="st-mono text-caption flex-shrink-0">{{ fmtTime(endTime) }}</span>
          </div>

          <!-- 速度 -->
          <v-menu :close-on-content-click="false" location="top" :offset="8">
            <template #activator="{ props: menuProps }">
              <v-btn v-bind="menuProps" variant="tonal" class="gp-speed-btn" prepend-icon="mdi-metronome">
                {{ speed.toFixed(2) }}x
              </v-btn>
            </template>
            <div class="st-bg-menu pa-4" style="min-width: 280px">
              <div class="text-caption font-weight-bold mb-2">{{ t("gp.speed") }}</div>
              <v-slider
                v-model="speed"
                :min="0.25"
                :max="2"
                :step="0.05"
                hide-details
                density="compact"
                thumb-color="primary"
                @update:model-value="applySpeed"
              >
                <template #thumb-label>{{ speed.toFixed(2) }}x</template>
              </v-slider>
              <div class="d-flex flex-wrap ga-1 mt-3">
                <v-btn
                  v-for="v in SPEED_PRESETS"
                  :key="v"
                  size="small"
                  :variant="Math.abs(speed - v) < 0.001 ? 'flat' : 'outlined'"
                  :color="Math.abs(speed - v) < 0.001 ? 'primary' : undefined"
                  @click="setSpeed(v)"
                >
                  {{ v }}x
                </v-btn>
              </div>
            </div>
          </v-menu>

          <!-- 循环 -->
          <v-menu :close-on-content-click="false" location="top" :offset="8">
            <template #activator="{ props: menuProps }">
              <v-btn
                v-bind="menuProps"
                :icon="loopEnabled ? 'mdi-repeat-variant' : 'mdi-repeat-off'"
                :variant="loopEnabled ? 'flat' : 'tonal'"
                :color="loopEnabled ? 'primary' : undefined"
                :title="t('gp.loop')"
              />
            </template>
            <div class="st-bg-menu pa-4" style="width: 340px">
              <div class="text-caption font-weight-bold mb-3">{{ t("gp.loop") }}</div>
              <div class="d-flex align-center ga-2 mb-3">
                <v-text-field
                  v-model.number="loopStart"
                  :label="t('gp.loop_start')"
                  type="number"
                  :min="1"
                  :max="barCount"
                  density="compact"
                  variant="underlined"
                  hide-details
                  style="min-width: 0"
                />
                <span class="st-mono flex-shrink-0">—</span>
                <v-text-field
                  v-model.number="loopEnd"
                  :label="t('gp.loop_end')"
                  type="number"
                  :min="1"
                  :max="barCount"
                  density="compact"
                  variant="underlined"
                  hide-details
                  style="min-width: 0"
                />
                <v-btn
                  size="small"
                  icon="mdi-check"
                  color="primary"
                  variant="flat"
                  :disabled="!loopValid"
                  :title="t('gp.loop_apply')"
                  class="flex-shrink-0"
                  @click="applyLoop"
                />
              </div>
              <div class="text-caption mb-3" style="opacity: 0.5">
                {{ t("gp.loop_hint", { n: barCount }) }}
              </div>
              <div class="d-flex ga-2">
                <v-btn size="small" color="primary" variant="flat" :disabled="!loopValid" @click="applyLoop">
                  {{ t("gp.loop_apply") }}
                </v-btn>
                <v-btn size="small" variant="tonal" :disabled="!loopEnabled" @click="clearLoop">
                  {{ t("gp.loop_clear") }}
                </v-btn>
              </div>
            </div>
          </v-menu>

          <!-- 谱面设置 -->
          <v-menu
            v-model="sheetMenuOpen"
            :close-on-content-click="false"
            location="top end"
            offset="8"
          >
            <template #activator="{ props: activatorProps }">
              <v-btn
                v-bind="activatorProps"
                icon="mdi-cog-outline"
                :variant="sheetMenuOpen ? 'flat' : 'tonal'"
                :color="sheetMenuOpen ? 'primary' : undefined"
                :title="t('gp.sheet_settings')"
              />
            </template>
            <div class="gp-sheet-menu">
              <div class="text-body-2 font-weight-black mb-3">{{ t("gp.sheet_settings") }}</div>
              <div class="d-flex align-center justify-space-between ga-4 mb-3">
                <span class="text-body-2">{{ t("gp.show_score") }}</span>
                <v-switch
                  v-model="showScore"
                  color="primary" hide-details density="compact"
                  class="gp-sheet-switch"
                  @update:model-value="applyStaves"
                />
              </div>
              <div class="d-flex align-center justify-space-between ga-4 mb-3">
                <span class="text-body-2">{{ t("gp.show_tab") }}</span>
                <v-switch
                  v-model="showTab"
                  color="primary" hide-details density="compact"
                  class="gp-sheet-switch"
                  @update:model-value="applyStaves"
                />
              </div>
              <div class="d-flex align-center justify-space-between ga-4 mb-3">
                <span class="text-body-2">{{ t("gp.show_chords") }}</span>
                <v-switch
                  v-model="showChords"
                  color="primary" hide-details density="compact"
                  class="gp-sheet-switch"
                  @update:model-value="applyChords"
                />
              </div>
              <div class="d-flex align-center ga-3 mb-1">
                <span class="text-body-2 flex-shrink-0">{{ t("gp.zoom") }}</span>
                <v-slider
                  v-model="zoom"
                  :min="0.6" :max="2" :step="0.05"
                  hide-details density="compact"
                  @update:model-value="applyZoom"
                />
                <span class="st-mono text-caption flex-shrink-0 gp-zoom-label">{{ Math.round(zoom * 100) }}%</span>
              </div>
              <div class="d-flex align-center ga-3">
                <span class="text-body-2 flex-shrink-0">{{ t("gp.count_in") }}</span>
                <v-select
                  v-model="countInBeats"
                  :items="countInOptions"
                  item-title="title" item-value="value"
                  hide-details density="compact" variant="outlined"
                  class="gp-countin-select"
                  @update:model-value="applyCountIn"
                />
              </div>
            </div>
          </v-menu>

          <!-- 节拍器 -->
          <v-btn
            :icon="metronomeOn ? 'mdi-metronome-tick' : 'mdi-metronome'"
            :variant="metronomeOn ? 'flat' : 'tonal'"
            :color="metronomeOn ? 'primary' : undefined"
            :title="t('gp.metronome')"
            @click="toggleMetronome"
          />

          <!-- 音轨 -->
          <v-btn
            icon="mdi-tune-vertical"
            variant="tonal"
            :title="t('gp.tracks')"
            @click="tracksOpen = !tracksOpen"
          />
        </div>
      </div>

      <!-- 音轨抽屉：悬浮在控制条上方，高度自适应内容 -->
      <Transition name="gp-slide">
        <div v-if="tracksOpen" class="gp-tracks" :class="{ 'gp-tracks--bar-hidden': barsHidden }">
          <div class="d-flex align-center px-4 py-3 flex-shrink-0">
            <span class="text-body-2 font-weight-black">{{ t("gp.tracks") }}</span>
            <span class="st-mono ml-2" style="opacity: 0.5">{{ trackStates.length }}</span>
            <v-spacer />
            <v-btn icon="mdi-close" variant="text" size="small" @click="tracksOpen = false" />
          </div>
          <div class="st-divider" />
          <div class="gp-track-list">
            <div v-for="tr in trackStates" :key="tr.index" class="gp-track-row pa-3">
              <div class="d-flex align-center ga-2 mb-2">
                <span class="st-mono" style="opacity: 0.5">{{ tr.index + 1 }}</span>
                <span class="text-body-2 font-weight-bold text-truncate flex-grow-1">{{ tr.name }}</span>
                <v-btn
                  size="x-small"
                  :variant="tr.mute ? 'flat' : 'outlined'"
                  :color="tr.mute ? 'primary' : undefined"
                  @click="toggleMute(tr)"
                >M</v-btn>
                <v-btn
                  size="x-small"
                  :variant="tr.solo ? 'flat' : 'outlined'"
                  :color="tr.solo ? 'primary' : undefined"
                  @click="toggleSolo(tr)"
                >S</v-btn>
              </div>
              <div class="d-flex align-center ga-2">
                <v-icon icon="mdi-volume-high" size="16" style="opacity: 0.5" />
                <v-slider
                  v-model="tr.volume"
                  :min="0" :max="1" :step="0.01"
                  hide-details density="compact"
                  @update:model-value="applyTrack(tr)"
                />
                <v-icon icon="mdi-pan-horizontal" size="16" style="opacity: 0.5" />
                <v-slider
                  v-model="tr.pan"
                  :min="-1" :max="1" :step="0.05"
                  hide-details density="compact" style="max-width: 90px"
                  @update:model-value="applyTrack(tr)"
                />
              </div>
            </div>
          </div>
        </div>
      </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import * as alphaTab from "@coderline/alphatab";
import { api, type SongDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { useToastStore, errText } from "@/stores/toast";

const props = defineProps<{ song: SongDto; bg: string; barsHidden?: boolean }>();

const settings = useSettingsStore();
const toast = useToastStore();

// ---------------------------------------------------------------------------
// 状态
// ---------------------------------------------------------------------------

const phase = ref<"loading" | "ready" | "error">("loading");
const errorMsg = ref("");
const soundFontProgress = ref(0);
const playerReady = ref(false);
const playing = ref(false);

const atEl = ref<HTMLElement | null>(null);
const scrollEl = ref<HTMLElement | null>(null);
let at: alphaTab.AlphaTabApi | null = null;

const currentTime = ref(0);
const endTime = ref(0);
const endTick = ref(0);
const barCount = ref(0);

const speed = ref(1);
const SPEED_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2];

const loopEnabled = ref(false);
const loopStart = ref(1);
const loopEnd = ref(1);

const metronomeOn = ref(false);
const tracksOpen = ref(false);
const sheetMenuOpen = ref(false);

// 谱面显示项（五线谱/六线谱/和弦名/缩放/预备拍），看谱时可即时调整
const showScore = ref(true);
const showTab = ref(true);
const showChords = ref(true);
const zoom = ref(1);
const countInBeats = ref(0);
const countInOptions = computed(() => [
  { title: t("gp.countin_off"), value: 0 },
  { title: t("gp.countin_one"), value: 1 },
  { title: t("gp.countin_two"), value: 2 },
  { title: t("gp.countin_four"), value: 4 },
]);

interface TrackState {
  index: number;
  name: string;
  mute: boolean;
  solo: boolean;
  volume: number; // 0–1
  pan: number; // -1..1
  // 用弱化类型持有 alphaTab 模型，避免 vue-tsc 深类型展开差异
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  track: any;
}
const trackStates = ref<TrackState[]>([]);

// ---------------------------------------------------------------------------
// AlphaTab 初始化
// ---------------------------------------------------------------------------

interface PersistedState {
  speed?: number;
  loop?: { enabled: boolean; start: number; end: number };
  tracks?: { index: number; mute: boolean; solo: boolean; volume: number; pan: number }[];
  scrollMode?: string;
}

function persisted(): PersistedState {
  try {
    return props.song.playerState ? (JSON.parse(props.song.playerState) as PersistedState) : {};
  } catch {
    return {};
  }
}

onMounted(async () => {
  try {
    console.log("[GpViewer] mounted, song:", props.song.id, props.song.uuid, props.song.fileType);
    const b64 = await api.readGpFile(props.song.uuid, props.song.fileType);
    console.log("[GpViewer] file read, base64 len:", b64.length);
    const bytes = base64ToBytes(b64);
    console.log("[GpViewer] bytes:", bytes.length, "magic:", Array.from(bytes.slice(0, 8)).map(b => b.toString(16)).join(" "));
    // 等下一帧确保容器已挂载且有尺寸，AlphaTab 初始化需要可测量的 DOM
    await nextTick();
    console.log("[GpViewer] atEl size:", atEl.value?.clientWidth, atEl.value?.clientHeight, "scrollEl:", scrollEl.value?.clientWidth, scrollEl.value?.clientHeight);
    initAlphaTab(bytes);
    console.log("[GpViewer] AlphaTabApi created, load called");
  } catch (e) {
    console.error("[GpViewer] mount error:", e);
    phase.value = "error";
    errorMsg.value = errText(e);
  }
});

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * 猜测 GP 文件字符串编码。
 * GP3-5 二进制中字符串以「长度前缀 + 定长字节」存储，老中文谱站导出普遍用 GBK。
 * 探测策略：扫描文件找高位字节连续段（>=0x80 视为多字节字符），
 * 按 UTF-8 校验失败（含替换字符）则判 GBK；纯 ASCII 或 UTF-8 合法则判 utf-8。
 */
function detectGpEncoding(data: Uint8Array): string {
  // 逐段找"看起来像文本"的高位字节区：跳过版本头（前 64 字节），
  // 在后续区域内找 >=0x80 的字节聚簇，取最长一段做编码校验
  const limit = Math.min(data.length, 8192);
  let bestStart = -1, bestLen = 0, curStart = -1;
  for (let i = 64; i < limit; i++) {
    if (data[i] >= 0x80) {
      if (curStart < 0) curStart = i;
    } else {
      const len = i - curStart;
      if (curStart >= 0 && len > bestLen) { bestLen = len; bestStart = curStart; }
      curStart = -1;
    }
  }
  if (curStart >= 0 && limit - curStart > bestLen) { bestLen = limit - curStart; bestStart = curStart; }
  if (bestStart < 0) return "utf-8"; // 全 ASCII，无所谓编码

  const probe = data.subarray(bestStart, Math.min(data.length, bestStart + bestLen + 64));
  try {
    const utf8 = new TextDecoder("utf-8", { fatal: false }).decode(probe);
    return utf8.includes("") ? "gbk" : "utf-8";
  } catch {
    return "gbk";
  }
}

function initAlphaTab(data: Uint8Array) {
  const el = atEl.value;
  if (!el) {
    console.error("[GpViewer] atEl is null!");
    return;
  }
  const gp = settings.settings.gp_player;
  const saved = persisted();
  speed.value = clamp(saved.speed ?? 1, 0.25, 2);
  showScore.value = gp.show_standard_notation;
  showTab.value = true;
  showChords.value = gp.show_chord_names;
  zoom.value = gp.zoom;
  countInBeats.value = gp.count_in_beats;

  const staveProfile = alphaTab.StaveProfile.ScoreTab;

  try {
    at = new alphaTab.AlphaTabApi(el, {
    core: {
      fontDirectory: "/font/",
      logLevel: alphaTab.LogLevel.Debug,
      includeNoteBounds: true,
    },
    importer: {
      // 老 GP 谱（尤其国内吉他谱站）常用 GBK/GB2312 存中文标题与音轨名
      encoding: detectGpEncoding(data),
    },
    display: {
      layoutMode: alphaTab.LayoutMode.Page,
      staveProfile,
      scale: zoom.value,
    },
    notation: {
      // 和弦名：关闭时隐藏谱面上方的和弦文字（EffectChordNames）
      elements: showChords.value
        ? undefined
        : new Map([[alphaTab.NotationElement.EffectChordNames, false]]),
    },
    player: {
      enablePlayer: true,
      enableCursor: true,
      enableAnimatedBeatCursor: true,
      soundFont: "/soundfont/sonivox.sf2",
      scrollElement: scrollEl.value ?? undefined,
      scrollMode: scrollModeOf(gp.scroll_mode),
      scrollOffsetY: -30,
      enableUserInteraction: true,
    },
  });
    console.log("[GpViewer] api instance created");
  } catch (e) {
    console.error("[GpViewer] AlphaTabApi ctor failed:", e);
    phase.value = "error";
    errorMsg.value = errText(e);
    return;
  }

  wireEvents();
  // -1 = 渲染全部音轨（多轨并列），与图片谱"默认全展开"一致
  const ok = at.load(data, [-1]);
  console.log("[GpViewer] load() returned", ok);
}

function scrollModeOf(mode: string): alphaTab.ScrollMode {
  switch (mode) {
    case "continuous":
      return alphaTab.ScrollMode.Continuous;
    case "off_screen":
      return alphaTab.ScrollMode.OffScreen;
    case "off":
      return alphaTab.ScrollMode.Off;
    default:
      return alphaTab.ScrollMode.Smooth;
  }
}

function wireEvents() {
  if (!at) return;
  at.error.on((e) => {
    console.error("[GpViewer] alphaTab error:", e);
    phase.value = "error";
    errorMsg.value = errText(e);
  });
  at.scoreLoaded.on((score) => {
    console.log("[GpViewer] scoreLoaded, bars:", score.masterBars.length);
    barCount.value = score.masterBars.length;
    buildTracks(score);
    const saved = persisted();
    if (saved.loop?.enabled) {
      loopStart.value = saved.loop.start;
      loopEnd.value = saved.loop.end;
      applyLoop();
    }
  });
  at.renderStarted.on((resized) => {
    console.log("[GpViewer] renderStarted resized=", resized);
  });
  at.renderFinished.on(() => {
    console.log("[GpViewer] renderFinished");
    if (phase.value === "loading") phase.value = "ready";
  });
  at.postRenderFinished.on(() => {
    console.log("[GpViewer] postRenderFinished");
    if (phase.value === "loading") phase.value = "ready";
  });
  at.soundFontLoad.on((e) => {
    soundFontProgress.value = e.loaded / Math.max(1, e.total);
  });
  at.soundFontLoaded.on(() => {
    soundFontProgress.value = 1;
  });
  at.playerReady.on(() => {
    playerReady.value = true;
    applySpeed(speed.value);
    at!.metronomeVolume = metronomeOn.value ? settings.settings.gp_player.metronome_volume / 100 : 0;
    applyCountIn();
    restoreTrackStates();
  });  at.playerStateChanged.on((e) => {
    playing.value = e.state === alphaTab.synth.PlayerState.Playing;
  });
  at.playerPositionChanged.on((e) => {
    currentTime.value = e.currentTime;
    endTime.value = e.endTime;
    endTick.value = e.endTick;
  });
  at.playerFinished.on(() => persistState());
}

function buildTracks(score: alphaTab.model.Score) {
  trackStates.value = score.tracks.map((tr) => ({
    index: tr.index,
    name: tr.name || `${t("gp.track")} ${tr.index + 1}`,
    mute: tr.playbackInfo.isMute,
    solo: tr.playbackInfo.isSolo,
    volume: tr.playbackInfo.volume / 16,
    pan: (tr.playbackInfo.balance - 8) / 8,
    track: tr,
  }));
}

function restoreTrackStates() {
  if (!at?.score) return;
  const saved = persisted();
  if (!saved.tracks) return;
  for (const s of saved.tracks) {
    const ts = trackStates.value.find((x) => x.index === s.index);
    if (!ts) continue;
    ts.mute = s.mute;
    ts.solo = s.solo;
    ts.volume = s.volume;
    ts.pan = s.pan;
    applyTrack(ts);
  }
}

// ---------------------------------------------------------------------------
// 播放控制
// ---------------------------------------------------------------------------

function togglePlay() {
  at?.playPause();
}

function stop() {
  at?.stop();
  persistState();
}

function seek(ms: number) {
  if (!at) return;
  at.timePosition = ms;
}

function fmtTime(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

// ---------------------------------------------------------------------------
// 速度
// ---------------------------------------------------------------------------

function applySpeed(v: number) {
  if (at) at.playbackSpeed = v;
}

function setSpeed(v: number) {
  speed.value = v;
  applySpeed(v);
  persistState();
}

// ---------------------------------------------------------------------------
// 循环
// ---------------------------------------------------------------------------

const loopValid = computed(
  () =>
    Number.isInteger(loopStart.value) &&
    Number.isInteger(loopEnd.value) &&
    loopStart.value >= 1 &&
    loopEnd.value >= loopStart.value &&
    barCount.value > 0,
);

const loopRangeStyle = computed(() => {
  if (!endTime.value) return {};
  // tick → 时间比例近似：直接用 tick 区间比例更准
  const ticks = loopTickRange();
  if (!ticks) return {};
  const l = (ticks.start / Math.max(1, endTick.value)) * 100;
  const w = ((ticks.end - ticks.start) / Math.max(1, endTick.value)) * 100;
  return { left: `${l}%`, width: `${Math.max(0.5, w)}%` };
});

function loopTickRange(): { start: number; end: number } | null {
  if (!at?.tickCache || !at.score) return null;
  const bars = at.score.masterBars;
  const s = Math.min(Math.max(1, loopStart.value), bars.length) - 1;
  const e = Math.min(Math.max(loopStart.value, loopEnd.value), bars.length) - 1;
  const start = at.tickCache.getMasterBarStart(bars[s]);
  const endLookup = at.tickCache.getMasterBar(bars[e]);
  return { start, end: endLookup.end };
}

function applyLoop() {
  if (!at || !loopValid.value) return;
  const ticks = loopTickRange();
  if (!ticks) return;
  if (loopEnd.value > barCount.value) {
    loopEnd.value = barCount.value;
    toast.push(t("gp.loop_clamped", { n: barCount.value }), "info");
  }
  at.playbackRange = { startTick: ticks.start, endTick: ticks.end };
  at.isLooping = true;
  loopEnabled.value = true;
  persistState();
}

function clearLoop() {
  if (!at) return;
  at.isLooping = false;
  at.playbackRange = null;
  loopEnabled.value = false;
  persistState();
}

// ---------------------------------------------------------------------------
// 节拍器 / 预备拍
// ---------------------------------------------------------------------------

function toggleMetronome() {
  metronomeOn.value = !metronomeOn.value;
  if (at) {
    at.metronomeVolume = metronomeOn.value ? settings.settings.gp_player.metronome_volume / 100 : 0;
  }
}

function applyCountIn() {
  if (!at) return;
  at.countInVolume = countInBeats.value > 0 ? 1 : 0;
  persistSheetPrefs();
}

// ---------------------------------------------------------------------------
// 谱面显示项（运行时切换，updateSettings + render 重排）
// ---------------------------------------------------------------------------

function applyStaves() {
  if (!at || !at.score) return;
  // 两者全关没有意义，强制保留其一
  if (!showScore.value && !showTab.value) {
    showScore.value = true;
  }
  for (const staff of at.score.tracks.flatMap((tr) => tr.staves)) {
    staff.showStandardNotation = showScore.value;
    staff.showTablature = showTab.value;
  }
  at.updateSettings();
  at.render();
  persistSheetPrefs();
}

function applyChords() {
  if (!at || !at.score) return;
  const st = at.score.stylesheet;
  // 谱内和弦名文字 + 和弦图统一开关
  for (const tr of at.score.tracks) {
    for (const staff of tr.staves) {
      for (const bar of staff.bars) {
        for (const voice of bar.voices) {
          for (const beat of voice.beats) {
            if (beat.chord) {
              beat.chord.showName = showChords.value;
              beat.chord.showDiagram = showChords.value;
            }
          }
        }
      }
    }
  }
  st.globalDisplayChordDiagramsInScore = showChords.value;
  at.updateSettings();
  at.render();
  persistSheetPrefs();
}

function applyZoom() {
  if (!at) return;
  at.settings.display.scale = zoom.value;
  at.updateSettings();
  at.render();
  persistSheetPrefs();
}

function persistSheetPrefs() {
  const gp = { ...settings.settings.gp_player };
  gp.show_standard_notation = showScore.value;
  gp.show_chord_names = showChords.value;
  gp.zoom = zoom.value;
  gp.count_in_beats = countInBeats.value;
  settings.settings.gp_player = gp;
  void settings.save();
}

// ---------------------------------------------------------------------------
// 音轨
// ---------------------------------------------------------------------------

function toggleMute(tr: TrackState) {
  tr.mute = !tr.mute;
  if (tr.mute) tr.solo = false;
  applyTrack(tr);
  persistState();
}

function toggleSolo(tr: TrackState) {
  tr.solo = !tr.solo;
  if (tr.solo) tr.mute = false;
  applyTrack(tr);
  persistState();
}

function applyTrack(tr: TrackState) {
  if (!at) return;
  at.changeTrackMute([tr.track], tr.mute);
  at.changeTrackSolo([tr.track], tr.solo);
  at.changeTrackVolume([tr.track], tr.volume);
  // 声像：alphaTab 暂无 track 级 pan API，写入 playbackInfo（0–16，8 居中）供合成器使用
  tr.track.playbackInfo.balance = Math.round(clamp(tr.pan, -1, 1) * 8 + 8);
}

// ---------------------------------------------------------------------------
// 持久化
// ---------------------------------------------------------------------------

let persistTimer: number | null = null;
function persistState() {
  if (persistTimer !== null) window.clearTimeout(persistTimer);
  persistTimer = window.setTimeout(() => {
    const state: PersistedState = {
      speed: speed.value,
      loop: { enabled: loopEnabled.value, start: loopStart.value, end: loopEnd.value },
      tracks: trackStates.value.map((x) => ({
        index: x.index,
        mute: x.mute,
        solo: x.solo,
        volume: x.volume,
        pan: x.pan,
      })),
      scrollMode: settings.settings.gp_player.scroll_mode,
    };
    void api.updatePlayerState(props.song.id, JSON.stringify(state)).catch(() => undefined);
  }, 300);
}

function clamp(v: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, v));
}

onBeforeUnmount(() => {
  persistState();
  at?.destroy();
  at = null;
});
</script>

<style scoped>
.gp-root {
  position: relative;
  height: 100%;
  overflow: hidden;
}
.gp-state {
  height: 100%;
}
.gp-state--overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  background: var(--st-black);
}
.gp-surface {
  position: absolute;
  inset: 0;
  overflow: auto;
  background: var(--st-light);
  /* 给底部悬浮控制条留出滚动余量，避免最后一行被遮住 */
  padding-bottom: 72px;
}
.gp-alphatab {
  min-height: 100%;
}
.gp-bottom {
  /* 复用 .st-viewer-bar--bottom 的 fixed 定位与显隐动画，这里只补尺寸与内边距 */
  height: 72px;
  border-top: 2px solid var(--st-border);
}
.gp-progress {
  min-width: 0;
}
.gp-slider-wrap {
  position: relative;
}
.gp-loop-range {
  position: absolute;
  top: 50%;
  height: 6px;
  transform: translateY(-50%);
  background: var(--st-coral);
  opacity: 0.85;
  pointer-events: none;
}
.gp-speed-btn {
  min-width: 86px;
}
.gp-tracks {
  position: fixed;
  top: 56px;
  right: 12px;
  bottom: 84px;
  width: 300px;
  background: var(--st-deep);
  border: 2px solid var(--st-border);
  display: flex;
  flex-direction: column;
  z-index: 2400;
  transition: bottom 0.3s ease;
  overflow: hidden;
}
/* 控制条隐藏时音轨抽屉沉底 */
.gp-tracks--bar-hidden {
  bottom: 12px;
}
.gp-track-list {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.gp-track-row {
  border-bottom: 1px solid var(--st-border);
}
.gp-slide-enter-active,
.gp-slide-leave-active {
  transition: transform 0.3s ease;
}
.gp-slide-enter-from,
.gp-slide-leave-to {
  transform: translateX(100%);
}
/* 速度/循环弹层用的 st-bg-menu：v-menu 内容被传送出组件树，需自带不透明背景 */
.st-bg-menu {
  background: #1a1a1a;
  border: 2px solid rgba(255, 255, 255, 0.2);
}
.gp-sheet-menu {
  width: 300px;
  padding: 14px 16px;
  background: var(--st-deep);
  border: 2px solid var(--st-border);
}
.gp-sheet-switch {
  flex: none;
}
.gp-zoom-label {
  width: 42px;
  text-align: right;
}
.gp-countin-select {
  max-width: 140px;
}
</style>

<style>
/* AlphaTab 运行时创建的光标 DOM 没有 data-v 属性，样式必须放在非 scoped 块 */
.gp-alphatab.at-surface .at-cursor-bar {
  background: rgba(255, 107, 107, 0.14) !important;
  border-radius: 0;
}
.gp-alphatab.at-surface .at-cursor-beat {
  background: #ff6b6b !important;
  /* 贯穿谱面的长竖线：alphaTab 内联设置的 height 用 !important 覆盖为超长值 */
  width: 5px !important;
  height: 800px !important;
  box-shadow: 0 0 6px rgba(255, 107, 107, 0.6);
  z-index: 3;
}
.gp-alphatab .at-highlight {
  fill: #ff6b6b;
}
</style>
