<template>
  <div
    class="library-root"
    @dragover.prevent="dragOver = true"
    @dragleave="dragOver = false"
    @drop.prevent="onDrop"
  >
    <GroupSidebar v-model="library.activeCollection" />

    <!-- 顶栏 -->
    <v-app-bar flat density="comfortable" :height="56" class="topbar" style="background: var(--st-black)">
      <div class="d-flex align-center flex-grow-1 px-6 ga-3">
        <v-text-field
          v-model="library.search"
          :placeholder="t('library.search')"
          prepend-inner-icon="mdi-magnify"
          variant="underlined"
          density="comfortable"
          hide-details
          max-width="380"
          clearable
        />
        <v-spacer />
        <v-btn variant="tonal" prepend-icon="mdi-monitor-screenshot" @click="screenshotOpen = true">
          {{ t("library.screenshot") }}
        </v-btn>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openImport([])">
          {{ t("library.import") }}
        </v-btn>
        <v-btn icon="mdi-cog-outline" variant="text" @click="router.push('/settings')" />
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
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { SongDto } from "@/lib/tauri";
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
const croppedImage = ref<string | null>(null);
const screenshotOpen = ref(false);

function openImport(paths: string[]) {
  presetPaths.value = paths;
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
        const images = paths.filter((p) =>
          /\.(jpe?g|png|webp|bmp|gif)$/i.test(p),
        );
        if (images.length) openImport(images);
      } else {
        dragOver.value = false;
      }
    })
    .then((fn) => (unlisten = fn));
});

onUnmounted(() => {
  unlisten?.();
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
</style>
