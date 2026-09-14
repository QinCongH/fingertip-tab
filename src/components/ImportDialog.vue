<template>
  <v-dialog v-model="visible" max-width="920" scrollable>
    <v-card>
      <v-card-title class="d-flex align-center pa-5 pb-3">
        <div class="st-display text-h5">{{ t("import.title") }}</div>
        <v-spacer />
        <v-btn icon="mdi-close" variant="text" size="small" @click="close" />
      </v-card-title>

      <div class="st-divider mx-5" />

      <v-card-text class="pa-5">
        <v-row no-gutters>
          <!-- 左：页面列表 -->
          <v-col cols="12" md="5" class="pr-md-6">
            <div class="d-flex align-center ga-2 mb-2">
              <span class="text-body-2 font-weight-black">{{ t("import.pages") }}</span>
              <span class="st-mono">{{ pages.length }}</span>
              <v-spacer />
              <v-btn size="small" variant="tonal" prepend-icon="mdi-image-multiple-outline" @click="pickFiles">
                {{ pages.length ? t("import.add_more") : t("import.select_files") }}
              </v-btn>
            </div>
            <div class="text-body-2 mb-1" style="opacity: 0.5">{{ t("import.pages_hint") }}</div>
            <div class="text-body-2 mb-3 d-flex align-center ga-1" style="opacity: 0.5">
              <v-icon icon="mdi-content-paste" size="13" />{{ t("import.paste_hint") }}
            </div>

            <div class="page-list">
              <div v-for="(img, i) in pages" :key="img.key" class="page-row d-flex align-center ga-2 pa-2">
                <span class="st-mono" style="width: 18px">{{ i + 1 }}</span>
                <div class="page-thumb">
                  <img :src="pageThumbSrc(img)" draggable="false" />
                </div>
                <span class="text-caption text-truncate flex-grow-1" style="opacity: 0.6">{{ img.name }}</span>
                <v-icon
                  size="18"
                  icon="mdi-arrow-up"
                  class="st-clickable"
                  :style="{ visibility: i === 0 ? 'hidden' : 'visible' }"
                  @click="movePage(i, -1)"
                />
                <v-icon
                  size="18"
                  icon="mdi-arrow-down"
                  class="st-clickable"
                  :style="{ visibility: i === pages.length - 1 ? 'hidden' : 'visible' }"
                  @click="movePage(i, 1)"
                />
                <v-icon size="18" icon="mdi-close" class="st-clickable" @click="removePage(i)" />
              </div>
              <div v-if="!pages.length" class="empty-pages d-flex flex-column align-center justify-center ga-2">
                <v-icon icon="mdi-image-plus-outline" size="34" style="opacity: 0.35" />
                <span class="text-body-2" style="opacity: 0.45">{{ t("import.select_files") }}</span>
              </div>
            </div>
          </v-col>

          <!-- 右：元数据 -->
          <v-col cols="12" md="7">
            <div class="d-flex align-center ga-2">
              <v-text-field
                v-model="meta.title"
                :label="t('import.meta_title') + ' *'"
                :placeholder="t('import.title_placeholder')"
                variant="underlined"
                density="comfortable"
                class="flex-grow-1"
                hide-details
              />
              <v-tooltip location="top" :text="t('import.ocr_tip')">
                <template #activator="{ props: tipProps }">
                  <v-btn
                    v-bind="tipProps"
                    icon="mdi-text-recognition"
                    variant="tonal"
                    :loading="ocrRunning"
                    :disabled="!pages.length"
                    @click="runOcr(false)"
                  />
                </template>
              </v-tooltip>
            </div>
            <div v-if="ocrStatusText" class="text-caption mb-1 d-flex align-center ga-1" style="opacity: 0.55">
              <v-icon icon="mdi-text-recognition" size="13" />{{ ocrStatusText }}
            </div>
            <v-text-field
              v-model="meta.artist"
              :label="t('import.meta_artist')"
              variant="underlined"
              density="comfortable"
              hide-details
            />
            <v-row no-gutters class="mt-3">
              <v-col cols="6" class="pr-3">
                <v-combobox
                  v-model="meta.tuning"
                  :items="TUNINGS"
                  :label="t('import.meta_tuning')"
                  variant="underlined"
                  density="comfortable"
                  hide-details
                />
              </v-col>
              <v-col cols="6">
                <v-text-field
                  v-model.number="meta.bpm"
                  :label="t('import.meta_bpm')"
                  type="number"
                  variant="underlined"
                  density="comfortable"
                  hide-details
                />
              </v-col>
            </v-row>
            <v-text-field
              v-model="meta.album"
              :label="t('import.meta_album')"
              variant="underlined"
              density="comfortable"
              hide-details
              class="mt-3"
            />
            <v-text-field
              v-model="meta.tags"
              :label="t('import.meta_tags')"
              variant="underlined"
              density="comfortable"
              hide-details
              class="mt-3"
            />
            <v-combobox
              v-model="collectionNames"
              :items="collectionOptions"
              :label="t('import.collections')"
              :placeholder="t('import.collections_hint')"
              multiple
              chips
              closable-chips
              variant="underlined"
              density="comfortable"
              hide-details
              class="mt-4"
            />
          </v-col>
        </v-row>
      </v-card-text>

      <div class="st-divider mx-5" />
      <v-card-actions class="pa-5">
        <span v-if="errorHint" class="text-error text-body-2">{{ errorHint }}</span>
        <v-spacer />
        <v-btn variant="text" @click="close">{{ t("import.cancel") }}</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">
          {{ t("import.save") }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, type SongDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";
import { useToastStore, errText } from "@/stores/toast";
import { recognizeText } from "@/lib/ocr";

const props = defineProps<{
  modelValue: boolean;
  /** 拖拽/按钮预选的文件路径 */
  presetPaths?: string[];
  /** 截图裁剪后的 dataURL（作为第一页，可与粘贴/文件混用） */
  imageData?: string | null;
}>();
const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
  (e: "imported", song: SongDto): void;
}>();

const library = useLibraryStore();
const toast = useToastStore();

const TUNINGS = ["Standard", "Drop D", "Open G", "Open D", "DADGAD", "Half Step Down"];

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit("update:modelValue", v),
});

// ---------------------------------------------------------------------------
// 页面列表（文件路径 + 内存图片 dataURL 统一管理）
// ---------------------------------------------------------------------------

interface PageItem {
  key: string;
  kind: "path" | "data";
  value: string;
  name: string;
  ext: string; // data 类型的格式（png/jpg）
}

const pages = ref<PageItem[]>([]);
let dataSeq = 0;

const meta = ref({ title: "", artist: "", album: "", tuning: "Standard", bpm: null as number | null, tags: "" });
const collectionNames = ref<string[]>([]);
const saving = ref(false);
const errorHint = ref("");

watch(
  () => props.modelValue,
  (v) => {
    if (!v) return;
    pages.value = (props.presetPaths ?? []).map((p) => ({
      key: `path-${p}`,
      kind: "path" as const,
      value: p,
      name: p.split(/[\\/]/).pop() ?? p,
      ext: "",
    }));
    if (props.imageData) {
      pages.value.unshift({
        key: `data-shot`,
        kind: "data",
        value: props.imageData,
        name: t("screenshot.title"),
        ext: dataUrlExt(props.imageData),
      });
    }
    meta.value = { title: "", artist: "", album: "", tuning: "Standard", bpm: null, tags: "" };
    collectionNames.value = [];
    errorHint.value = "";
    resetOcr();
    // 有图且歌名为空时自动尝试 OCR
    if (pages.value.length) void runOcr(true);
  },
);

function pageThumbSrc(item: PageItem): string {
  return item.kind === "path" ? convertFileSrc(item.value) : item.value;
}

function dataUrlExt(dataUrl: string): string {
  const m = /^data:image\/(png|jpeg|jpg|webp|bmp|gif)/i.exec(dataUrl);
  if (!m) return "png";
  const raw = m[1].toLowerCase();
  return raw === "jpeg" ? "jpg" : raw;
}

function movePage(index: number, dir: -1 | 1) {
  const j = index + dir;
  if (j < 0 || j >= pages.value.length) return;
  const arr = [...pages.value];
  const [item] = arr.splice(index, 1);
  arr.splice(j, 0, item);
  pages.value = arr;
}

function removePage(index: number) {
  const arr = [...pages.value];
  arr.splice(index, 1);
  pages.value = arr;
}

async function pickFiles() {
  const picked = await open({
    multiple: true,
    filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }],
  });
  if (!picked) return;
  const added = Array.isArray(picked) ? picked : [picked];
  for (const p of added) {
    if (pages.value.some((it) => it.kind === "path" && it.value === p)) continue;
    pages.value.push({
      key: `path-${p}`,
      kind: "path",
      value: p,
      name: p.split(/[\\/]/).pop() ?? p,
      ext: "",
    });
  }
}

// ---------------------------------------------------------------------------
// 剪贴板粘贴图片
// ---------------------------------------------------------------------------

async function onPaste(e: ClipboardEvent) {
  if (!visible.value) return;
  const items = Array.from(e.clipboardData?.items ?? []);
  const imageItems = items.filter((it) => it.type.startsWith("image/"));
  if (!imageItems.length) return;
  e.preventDefault();

  let added = 0;
  for (const item of imageItems) {
    const file = item.getAsFile();
    if (!file) continue;
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error("read clipboard image failed"));
      reader.readAsDataURL(file);
    });
    dataSeq += 1;
    pages.value.push({
      key: `data-paste-${dataSeq}`,
      kind: "data",
      value: dataUrl,
      name: `${t("import.pasted")} ${dataSeq}`,
      ext: dataUrlExt(dataUrl),
    });
    added += 1;
  }
  if (added) {
    toast.success(t("import.pasted_added", { n: added }));
    if (!meta.value.title.trim()) void runOcr(true);
  }
}

watch(visible, (v) => {
  if (v) window.addEventListener("paste", onPaste);
  else window.removeEventListener("paste", onPaste);
});
onUnmounted(() => window.removeEventListener("paste", onPaste));

// ---------------------------------------------------------------------------
// OCR 自动识别（Tesseract WASM，本地推理）
// ---------------------------------------------------------------------------

const ocrRunning = ref(false);
const ocrFailed = ref(false);
const ocrStatusText = ref("");
let ocrAttempted = false;

function resetOcr() {
  ocrRunning.value = false;
  ocrFailed.value = false;
  ocrStatusText.value = "";
  ocrAttempted = false;
}

function ocrStatusKey(status: string): string | null {
  if (status.includes("loading tesseract core")) return "import.ocr_prepare";
  if (status.includes("initializing tesseract")) return "import.ocr_prepare";
  if (status.includes("loading language")) return "import.ocr_model";
  if (status.includes("initializing api")) return "import.ocr_prepare";
  if (status.includes("recognizing")) return "import.ocr_recognize";
  return null;
}

async function runOcr(auto: boolean) {
  const first = pages.value[0];
  if (!first || ocrRunning.value) return;
  if (auto && (ocrAttempted || meta.value.title.trim())) return;
  ocrAttempted = true;
  ocrFailed.value = false;
  ocrRunning.value = true;
  try {
    const image =
      first.kind === "data" ? first.value : await api.readImageBase64(first.value);
    const text = await recognizeText(image, (status, progress) => {
      const key = ocrStatusKey(status);
      ocrStatusText.value = key
        ? t(key, { p: Math.round(progress * 100) })
        : status;
    });
    const lines = text
      .split(/\r?\n/)
      .map((l) => l.replace(/\s+/g, " ").trim())
      .filter((l) => l.length > 1);

    let filled = false;
    if (lines.length && !meta.value.title.trim()) {
      meta.value.title = lines[0].slice(0, 40);
      filled = true;
    }
    if (lines.length > 1 && !meta.value.tags.trim()) {
      meta.value.tags = lines.slice(1, 3).join(",").slice(0, 60);
      filled = true;
    }
    ocrStatusText.value = "";
    if (filled) toast.success(t("import.ocr_done"));
    else if (!auto) toast.push(t("import.ocr_empty"), "info");
  } catch (e) {
    ocrFailed.value = true;
    ocrStatusText.value = "";
    toast.error(`${t("import.ocr_failed")} — ${errText(e)}`);
  } finally {
    ocrRunning.value = false;
  }
}

// ---------------------------------------------------------------------------
// 保存
// ---------------------------------------------------------------------------

const collectionOptions = computed(() => library.collections.map((c) => c.name));

async function resolveCollectionIds(): Promise<number[]> {
  const ids: number[] = [];
  for (const name of collectionNames.value) {
    const existing = library.collections.find((c) => c.name === name);
    if (existing) {
      ids.push(existing.id);
    } else {
      const created = await api.createCollection(name.trim());
      ids.push(created.id);
    }
  }
  await library.loadCollections();
  return ids;
}

async function save() {
  errorHint.value = "";
  const title = meta.value.title.trim();
  if (!title) {
    errorHint.value = t("import.need_title");
    return;
  }
  if (!pages.value.length) {
    errorHint.value = t("import.need_image");
    return;
  }
  saving.value = true;
  try {
    const ids = await resolveCollectionIds();
    const metaInput = {
      title,
      artist: meta.value.artist.trim(),
      album: meta.value.album.trim(),
      tuning: meta.value.tuning?.trim() || "Standard",
      bpm: meta.value.bpm && meta.value.bpm > 0 ? Math.round(meta.value.bpm) : null,
      tags: meta.value.tags.trim(),
    };
    const pathItems = pages.value.filter((p) => p.kind === "path").map((p) => p.value);
    const extraImages = pages.value
      .filter((p) => p.kind === "data")
      .map((p) => ({ data: p.value, ext: p.ext }));
    const song = await api.importSongs(pathItems, extraImages, metaInput, ids);
    toast.success(t("library.imported"));
    emit("imported", song);
    close();
  } catch (e) {
    errorHint.value = errText(e);
  } finally {
    saving.value = false;
  }
}

function close() {
  visible.value = false;
}
</script>

<style scoped>
.page-list {
  border: 2px solid var(--st-border);
  min-height: 220px;
  max-height: 380px;
  overflow-y: auto;
  background: rgba(13, 13, 13, 0.5);
}
.page-row {
  border-bottom: 1px solid var(--st-border);
}
.page-row:hover {
  background: rgba(255, 107, 107, 0.06);
}
.page-thumb {
  width: 44px;
  height: 56px;
  background: #e8e8e0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;
}
.page-thumb img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
.empty-pages {
  height: 220px;
}
</style>
