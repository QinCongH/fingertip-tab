<template>
  <div class="st-card" @click="emit('open')">
    <!-- 封面 -->
    <div class="thumb-wrap">
      <template v-if="isGp">
        <div class="thumb-gp d-flex flex-column align-center justify-center ga-2">
          <v-icon icon="mdi-music-note-sixteenth" size="44" style="opacity: 0.55" />
          <span class="st-mono gp-ext">.{{ song.fileType }}</span>
        </div>
        <div class="pages-badge st-chip-coral">GP</div>
      </template>
      <template v-else>
      <img
        v-if="coverSrc"
        :src="coverSrc"
        class="thumb-img"
        loading="lazy"
        draggable="false"
        @error="imgFailed = true"
      />
      <div v-else class="thumb-fallback st-display">♪</div>
      <div class="pages-badge st-chip-coral">{{ t("library.pages_count", { n: song.pages.length }) }}</div>
      </template>
      <div class="hover-open">
        <v-btn color="primary" prepend-icon="mdi-book-open-variant" variant="flat">
          {{ t("library.open") }}
        </v-btn>
      </div>
    </div>

    <!-- 信息 -->
    <div class="pa-3">
      <div class="d-flex align-start">
        <div class="min-width-0 flex-grow-1">
          <div class="font-weight-black text-truncate" style="font-size: 15px">{{ song.title || "—" }}</div>
          <div class="text-truncate" style="font-size: 12px; opacity: 0.55">
            {{ song.artist || "Unknown" }}<template v-if="song.tuning"> · {{ song.tuning }}</template>
          </div>
        </div>
        <v-menu location="bottom end">
          <template #activator="{ props: menuProps }">
            <v-btn
              icon="mdi-dots-vertical"
              variant="text"
              size="small"
              v-bind="menuProps"
              class="card-menu"
              @click.stop
            />
          </template>
          <v-list density="compact" class="py-0">
            <v-list-item prepend-icon="mdi-pencil-outline" :title="t('library.edit')" @click.stop="emit('edit')" />
            <v-list-item
              prepend-icon="mdi-trash-can-outline"
              :title="t('library.delete')"
              class="text-error"
              @click.stop="emit('delete')"
            />
          </v-list>
        </v-menu>
      </div>

      <div class="d-flex ga-1 mt-2 flex-wrap">
        <span v-if="song.bpm" class="st-chip-line">{{ song.bpm }} BPM</span>
        <span v-for="tag in tagList" :key="tag" class="st-chip-line">{{ tag }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { SongDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";

const props = defineProps<{ song: SongDto }>();
const emit = defineEmits<{ (e: "open"): void; (e: "edit"): void; (e: "delete"): void }>();

const library = useLibraryStore();
const imgFailed = ref(false);
const isGp = computed(() => props.song.format === "gp");

const coverSrc = computed(() => {
  if (imgFailed.value || isGp.value) return null;
  const cover = props.song.pages[0];
  if (!cover) return null;
  return library.thumbSrc(cover.uuid);
});

const tagList = computed(() =>
  props.song.tags
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 3),
);
</script>

<style scoped>
.thumb-wrap {
  position: relative;
  aspect-ratio: 4 / 3;
  background: #e8e8e0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.thumb-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.thumb-fallback {
  font-size: 48px;
  color: #1a1a1a;
  opacity: 0.25;
}
.thumb-gp {
  width: 100%;
  height: 100%;
  color: #1a1a1a;
}
.gp-ext {
  font-size: 13px;
  opacity: 0.4;
  letter-spacing: 1px;
}
.pages-badge {
  position: absolute;
  left: 8px;
  bottom: 8px;
}
.hover-open {
  position: absolute;
  inset: 0;
  background: rgba(13, 13, 13, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.3s ease;
}
.st-card:hover .hover-open {
  opacity: 1;
}
.card-menu {
  opacity: 0.45;
}
.st-card:hover .card-menu {
  opacity: 1;
}
.min-width-0 {
  min-width: 0;
}
</style>
