import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  api,
  pageImageUrl,
  thumbImageUrl,
  type CollectionDto,
  type SongDto,
} from "@/lib/tauri";
import { useSettingsStore } from "./settings";

/** 曲谱库状态：全部数据客户端过滤（本地 SQLite，量级小） */
export const useLibraryStore = defineStore("library", () => {
  const settings = useSettingsStore();

  const songs = ref<SongDto[]>([]);
  const collections = ref<CollectionDto[]>([]);
  const loading = ref(false);

  /** 'all' | 'uncategorized' | collectionId */
  const activeCollection = ref<number | "all" | "uncategorized">("all");
  const search = ref("");

  async function loadSongs() {
    loading.value = true;
    try {
      songs.value = await api.getSongs(null, null);
    } finally {
      loading.value = false;
    }
  }

  async function loadCollections() {
    collections.value = await api.getCollections();
  }

  async function refresh() {
    await Promise.all([loadSongs(), loadCollections()]);
  }

  const filteredSongs = computed(() => {
    const q = (search.value ?? "").toString().trim().toLowerCase();
    return songs.value.filter((s) => {
      if (activeCollection.value === "uncategorized" && s.collectionIds.length > 0) return false;
      if (typeof activeCollection.value === "number" && !s.collectionIds.includes(activeCollection.value)) {
        return false;
      }
      if (q) {
        const hit =
          s.title.toLowerCase().includes(q) ||
          s.artist.toLowerCase().includes(q) ||
          s.tags.toLowerCase().includes(q);
        if (!hit) return false;
      }
      return true;
    });
  });

  // ------- 图片 URL 助手 -------

  function pageSrc(uuid: string, fileType: string): string {
    return pageImageUrl(settings.settings.general.library_path, uuid, fileType);
  }

  function thumbSrc(uuid: string): string {
    return thumbImageUrl(settings.settings.general.library_path, uuid);
  }

  return {
    songs,
    collections,
    loading,
    activeCollection,
    search,
    filteredSongs,
    loadSongs,
    loadCollections,
    refresh,
    pageSrc,
    thumbSrc,
  };
});
