<template>
  <v-dialog v-model="visible" max-width="920" scrollable>
    <v-card v-if="song">
      <v-card-title class="d-flex align-center pa-5 pb-3">
        <div class="st-display text-h5">{{ t("edit.title") }}</div>
        <v-spacer />
        <v-btn icon="mdi-close" variant="text" size="small" @click="close" />
      </v-card-title>

      <div class="st-divider mx-5" />

      <v-card-text class="pa-5">
        <v-row no-gutters>
          <!-- 左：页面管理（GP 曲谱无分页概念，显示文件信息） -->
          <v-col cols="12" md="5" class="pr-md-6">
            <template v-if="isGp">
              <div class="d-flex align-center ga-2 mb-2">
                <span class="text-body-2 font-weight-black">{{ t("edit.gp_file") }}</span>
              </div>
              <div class="gp-file-info d-flex align-center ga-3 pa-4">
                <v-icon icon="mdi-music-note-sixteenth" size="30" style="opacity: 0.5" />
                <div class="min-width-0">
                  <div class="text-body-2 font-weight-bold">{{ song.uuid }}.{{ song.fileType }}</div>
                  <div class="text-caption" style="opacity: 0.5">{{ t("edit.gp_hint") }}</div>
                </div>
              </div>
            </template>
            <template v-else>
            <div class="d-flex align-center ga-2 mb-2">
              <span class="text-body-2 font-weight-black">{{ t("edit.pages_manage") }}</span>
              <span class="st-mono">{{ pages.length }}</span>
              <v-spacer />
              <v-btn size="small" variant="tonal" prepend-icon="mdi-image-plus-outline" :loading="adding" @click="addPages">
                {{ t("edit.add_pages") }}
              </v-btn>
            </div>

            <div class="page-list">
              <div v-for="(p, i) in pages" :key="p.uuid" class="page-row d-flex align-center ga-2 pa-2">
                <span class="st-mono" style="width: 18px">{{ i + 1 }}</span>
                <div class="page-thumb">
                  <img :src="library.thumbSrc(p.uuid)" draggable="false" />
                </div>
                <span class="text-caption flex-grow-1 st-mono">{{ p.uuid.slice(0, 8) }}…</span>
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
                <v-icon
                  size="18"
                  icon="mdi-close"
                  class="st-clickable"
                  :style="{ visibility: pages.length <= 1 ? 'hidden' : 'visible' }"
                  @click="removePage(i)"
                />
              </div>
            </div>
            </template>
          </v-col>

          <!-- 右：元数据 -->
          <v-col cols="12" md="7">
            <v-text-field
              v-model="meta.title"
              :label="t('import.meta_title') + ' *'"
              variant="underlined"
              density="comfortable"
            />
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
        <v-btn variant="text" @click="close">{{ t("common.cancel") }}</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">
          {{ t("edit.save") }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type PageDto, type SongDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";
import { useToastStore, errText } from "@/stores/toast";

const props = defineProps<{ modelValue: boolean; song: SongDto | null }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
  (e: "saved", song: SongDto): void;
}>();

const library = useLibraryStore();
const toast = useToastStore();

const TUNINGS = ["Standard", "Drop D", "Open G", "Open D", "DADGAD", "Half Step Down"];

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit("update:modelValue", v),
});

const meta = ref({ title: "", artist: "", album: "", tuning: "Standard", bpm: null as number | null, tags: "" });
const collectionNames = ref<string[]>([]);
const pages = ref<PageDto[]>([]);
const saving = ref(false);
const adding = ref(false);
const errorHint = ref("");
const isGp = computed(() => props.song?.format === "gp");

watch(
  () => props.modelValue,
  (v) => {
    if (!v || !props.song) return;
    const s = props.song;
    meta.value = {
      title: s.title,
      artist: s.artist,
      album: s.album,
      tuning: s.tuning || "Standard",
      bpm: s.bpm,
      tags: s.tags,
    };
    pages.value = [...s.pages];
    collectionNames.value = s.collectionIds
      .map((id) => library.collections.find((c) => c.id === id)?.name)
      .filter((n): n is string => Boolean(n));
    errorHint.value = "";
  },
);

const collectionOptions = computed(() => library.collections.map((c) => c.name));

// ------- 页面管理（即时生效） -------

async function applyPages() {
  if (!props.song) return;
  try {
    const updated = await api.setSongPages(props.song.id, pages.value.map((p) => p.uuid));
    pages.value = [...updated.pages];
    emit("saved", updated);
  } catch (e) {
    toast.error(errText(e));
  }
}

function movePage(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= pages.value.length) return;
  const arr = [...pages.value];
  const [item] = arr.splice(i, 1);
  arr.splice(j, 0, item);
  pages.value = arr;
  void applyPages();
}

function removePage(i: number) {
  if (pages.value.length <= 1) return;
  const arr = [...pages.value];
  arr.splice(i, 1);
  pages.value = arr;
  void applyPages();
}

async function addPages() {
  if (!props.song) return;
  const picked = await open({
    multiple: true,
    filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }],
  });
  if (!picked) return;
  adding.value = true;
  try {
    const paths = Array.isArray(picked) ? picked : [picked];
    const updated = await api.addPagesToSong(props.song.id, paths);
    pages.value = [...updated.pages];
    emit("saved", updated);
  } catch (e) {
    toast.error(errText(e));
  } finally {
    adding.value = false;
  }
}

// ------- 元数据保存 -------

async function save() {
  if (!props.song) return;
  errorHint.value = "";
  const title = meta.value.title.trim();
  if (!title) {
    errorHint.value = t("import.need_title");
    return;
  }
  saving.value = true;
  try {
    const ids: number[] = [];
    for (const name of collectionNames.value) {
      const existing = library.collections.find((c) => c.name === name);
      ids.push(existing ? existing.id : (await api.createCollection(name.trim())).id);
    }
    await library.loadCollections();
    await api.updateSong(props.song.id, {
      title,
      artist: meta.value.artist.trim(),
      album: meta.value.album.trim(),
      tuning: meta.value.tuning?.trim() || "Standard",
      bpm: meta.value.bpm && meta.value.bpm > 0 ? Math.round(meta.value.bpm) : null,
      tags: meta.value.tags.trim(),
    }, ids);
    const fresh = await api.getSong(props.song.id);
    toast.success(t("edit.saved"));
    emit("saved", fresh);
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
.gp-file-info {
  border: 2px solid var(--st-border);
  background: rgba(13, 13, 13, 0.5);
  min-height: 88px;
}
.min-width-0 {
  min-width: 0;
}
</style>
