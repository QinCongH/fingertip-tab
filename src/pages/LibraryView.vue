<template>
  <div
    class="library-root"
    @dragover.prevent="dragOver = true"
    @dragleave="dragOver = false"
    @drop.prevent="onDrop"
  >
    <GroupSidebar v-model="library.activeCollection" />

    <!-- 顶栏 -->
    <v-app-bar flat density="comfortable" :height="56" class="topbar" style="background: var(--st-black)" data-tauri-drag-region>
      <div class="d-flex align-center flex-grow-1 pl-6 pr-2 ga-3 min-width-0" data-tauri-drag-region>
        <div style="flex: 0 1 380px; min-width: 140px; -webkit-app-region: no-drag">
          <v-text-field
            v-model="library.search"
            :placeholder="t('library.search')"
            prepend-inner-icon="mdi-magnify"
            variant="underlined"
            density="comfortable"
            hide-details
            clearable
          />
        </div>
        <v-spacer data-tauri-drag-region />
        <div class="d-flex align-center ga-2 flex-shrink-0" style="-webkit-app-region: no-drag">
          <v-btn variant="tonal" prepend-icon="mdi-monitor-screenshot" @click="screenshotOpen = true">
            {{ t("library.screenshot") }}
          </v-btn>
          <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openImport([])">
            {{ t("library.import") }}
          </v-btn>
          <!-- 局域网共享 -->
          <v-btn
            icon="mdi-qrcode"
            :variant="shareActive ? 'flat' : 'text'"
            :color="shareActive ? 'primary' : undefined"
            :title="t('share.title')"
            @click="onShareBtnClick"
          />
          <v-btn icon="mdi-cog-outline" variant="text" @click="router.push('/settings')" />
        </div>
        <div class="ml-1 flex-shrink-0">
          <WindowControls />
        </div>
      </div>
    </v-app-bar>

    <v-main>
      <div class="pa-8 pt-10" style="height: calc(100vh - 56px); overflow-y: auto">
        <!-- 标题 -->
        <div class="d-flex align-end ga-4 mb-8">
          <div class="st-display" style="font-size: 44px">
            {{ activeTitle }}
          </div>
          <div class="st-mono pb-2">{{ t("library.count_songs", { n: library.filteredSongs.length }) }}</div>
        </div>

        <!-- 空状态 -->
        <div
          v-if="!library.filteredSongs.length && !library.loading"
          class="empty-state d-flex flex-column align-center justify-center ga-4"
        >
          <v-icon icon="mdi-guitar-acoustic" size="72" style="opacity: 0.3" />
          <div class="st-display text-h4">{{ emptyTitle }}</div>
          <div class="text-body-1" style="opacity: 0.55; max-width: 420px; text-align: center">
            {{ t("library.empty_desc") }}
          </div>
          <v-btn color="primary" variant="flat" size="large" prepend-icon="mdi-plus" @click="openImport([])">
            {{ t("library.import") }}
          </v-btn>
        </div>

        <!-- 卡片网格 -->
        <div v-else class="card-grid">
          <SongCard
            v-for="song in library.filteredSongs"
            :key="song.id"
            :song="song"
            @open="openViewer(song)"
            @edit="editSong = song"
            @delete="askDelete(song)"
          />
        </div>
      </div>
    </v-main>

    <!-- 拖拽遮罩 -->
    <div v-if="dragOver" class="st-dropzone">
      <v-icon icon="mdi-tray-arrow-down" size="64" />
      <div class="st-display text-h4">{{ t("library.drag_hint") }}</div>
    </div>

    <!-- 弹窗 -->
    <ImportDialog
      v-model="importOpen"
      :preset-paths="presetPaths"
      :preset-format="presetFormat"
      :image-data="croppedImage"
      @imported="onImported"
    />
    <ScreenCaptureDialog v-model="screenshotOpen" @cropped="onCropped" />
    <EditSongDialog v-model="editOpen" :song="editSong" @saved="onEdited" />
    <ConfirmDialog
      v-model="deleteOpen"
      :title="t('library.delete_confirm_title')"
      :text="t('library.delete_confirm_text', { name: deleteSong?.title ?? '' })"
      danger
      @confirm="doDelete"
    />

    <!-- 局域网共享弹窗 -->
    <v-dialog v-model="shareOpen" max-width="380">
      <div class="share-dialog">
        <div class="d-flex align-center mb-3">
          <span class="text-body-1 font-weight-black">{{ t("share.title") }}</span>
          <v-spacer />
          <v-btn icon="mdi-close" variant="text" size="small" @click="shareOpen = false" />
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
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import QRCode from "qrcode";
import type { SongDto, ShareInfo } from "@/lib/tauri";
import { api } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";
import { useToastStore, errText } from "@/stores/toast";
import GroupSidebar from "@/components/GroupSidebar.vue";
import SongCard from "@/components/SongCard.vue";
import ImportDialog from "@/components/ImportDialog.vue";
import ScreenCaptureDialog from "@/components/ScreenCaptureDialog.vue";
import EditSongDialog from "@/components/EditSongDialog.vue";
import ConfirmDialog from "@/components/ConfirmDialog.vue";
import WindowControls from "@/components/WindowControls.vue";

const router = useRouter();
const library = useLibraryStore();
const toast = useToastStore();

const activeTitle = computed(() => {
  if (library.activeCollection === "all") return t("library.title");
  if (library.activeCollection === "uncategorized") return t("library.uncategorized");
  return library.collections.find((c) => c.id === library.activeCollection)?.name ?? t("library.title");
});

const emptyTitle = computed(() =>
  (library.search ?? "").toString().trim() ? t("library.no_match") : t("library.empty_title"),
);

onMounted(() => {
  void library.refresh();
});

// ------- 打开 / 删除 -------

function openViewer(song: SongDto) {
  void router.push(`/viewer/${song.id}`);
}

const deleteOpen = ref(false);
const deleteSong = ref<SongDto | null>(null);
function askDelete(song: SongDto) {
  deleteSong.value = song;
  deleteOpen.value = true;
}
async function doDelete() {
  if (!deleteSong.value) return;
  try {
    await api.deleteSong(deleteSong.value.id);
    await library.refresh();
    toast.success(t("library.deleted"));
  } catch (e) {
    toast.error(errText(e));
  }
}

// ------- 导入 -------

const importOpen = ref(false);
const presetPaths = ref<string[]>([]);
const presetFormat = ref<"image" | "gp">("image");
const croppedImage = ref<string | null>(null);
const screenshotOpen = ref(false);

function openImport(paths: string[]) {
  presetPaths.value = paths;
  if (!paths.length) presetFormat.value = "image";
  croppedImage.value = null;
  importOpen.value = true;
}

function onCropped(dataUrl: string) {
  screenshotOpen.value = false;
  croppedImage.value = dataUrl;
  presetPaths.value = [];
  importOpen.value = true;
}

async function onImported() {
  await library.refresh();
}

function onEdited() {
  void library.refresh();
}

const editOpen = ref(false);
const editSong = ref<SongDto | null>(null);
watch(editSong, (s) => (editOpen.value = !!s));

// ------- 拖拽导入 -------

const dragOver = ref(false);
let unlisten: (() => void) | null = null;

function onDrop(e: DragEvent) {
  dragOver.value = false;
  // 浏览器 drop 事件拿不到完整路径，交给 Tauri 的 drag-drop 处理
  e.preventDefault();
}

onMounted(() => {
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        dragOver.value = true;
      } else if (event.payload.type === "drop") {
        dragOver.value = false;
        const paths = event.payload.paths ?? [];
        const gp = paths.filter((p) => /\.(gp3|gp4|gp5|gpx|gp)$/i.test(p));
        const images = paths.filter((p) =>
          /\.(jpe?g|png|webp|bmp|gif)$/i.test(p),
        );
        if (gp.length && !images.length) {
          presetFormat.value = "gp";
          openImport(gp.slice(0, 1));
        } else if (images.length) {
          presetFormat.value = "image";
          openImport(images);
        }
      } else {
        dragOver.value = false;
      }
    })
    .then((fn) => (unlisten = fn));
});

onUnmounted(() => {
  unlisten?.();
});

// ------- 局域网整库共享 -------

const shareOpen = ref(false);
const shareActive = ref(false);
const shareInfo = ref<ShareInfo | null>(null);
const qrDataUrl = ref("");

async function checkCurrentShare() {
  try {
    const info = await api.getShareInfo();
    if (info) {
      shareActive.value = true;
      shareInfo.value = info;
      qrDataUrl.value = await QRCode.toDataURL(info.url, {
        width: 240,
        margin: 1,
        color: { dark: "#1A1A1A", light: "#F5F5F0" },
      });
    } else {
      shareActive.value = false;
      shareInfo.value = null;
      qrDataUrl.value = "";
    }
  } catch {
    // 忽略检查失败
  }
}

async function onShareBtnClick() {
  // 点击二维码按钮打开弹窗并确保共享开启
  shareOpen.value = true;
  if (!shareActive.value || !shareInfo.value) {
    await startLibrarySharing();
  }
}

async function startLibrarySharing() {
  shareInfo.value = null;
  qrDataUrl.value = "";
  try {
    const info = await api.startLibraryShare();
    shareInfo.value = info;
    shareActive.value = true;
    qrDataUrl.value = await QRCode.toDataURL(info.url, {
      width: 240,
      margin: 1,
      color: { dark: "#1A1A1A", light: "#F5F5F0" },
    });
  } catch (e) {
    toast.error(t("share.failed", { msg: errText(e) }));
    shareActive.value = false;
    shareOpen.value = false;
  }
}

async function closeShare() {
  shareOpen.value = false;
  shareActive.value = false;
  shareInfo.value = null;
  qrDataUrl.value = "";
  try {
    await api.stopShare();
  } catch {
    // 忽略停止失败
  }
}

async function copyShareUrl() {
  if (!shareInfo.value) return;
  try {
    await navigator.clipboard.writeText(shareInfo.value.url);
    toast.success(t("share.copied"));
  } catch {
    // 忽略复制失败
  }
}

onMounted(() => {
  void checkCurrentShare();
});
</script>

<style scoped>
.library-root {
  height: 100vh;
  background: var(--st-black);
  overflow: hidden;
}
.topbar {
  border-bottom: 2px solid var(--st-border);
}
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 22px;
}
.empty-state {
  height: 60vh;
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
</style>
