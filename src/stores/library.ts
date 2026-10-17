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

  /** 分页状态 */
  const page = ref(1);
  const pageSize = ref(20);
  const total = ref(0);
  const allCount = ref(0);
  const uncategorizedCount = ref(0);
  const totalPages = ref(1);

  /** 'all' | 'uncategorized' | collectionId */
  const activeCollection = ref<number | "all" | "uncategorized">("all");
  const search = ref("");

  async function loadSongs() {
    loading.value = true;
    try {
      const isUncategorized = activeCollection.value === "uncategorized";
      const collectionId = typeof activeCollection.value === "number" ? activeCollection.value : null;
      const res = await api.getSongsPaged(
        page.value,
        pageSize.value,
        collectionId,
        isUncategorized,
        search.value ? search.value.trim() : null,
      );
      songs.value = res.items;
      total.value = res.total;
      allCount.value = res.allCount;
      uncategorizedCount.value = res.uncategorizedCount;
      totalPages.value = res.totalPages;
      page.value = res.page;
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

  const filteredSongs = computed(() => songs.value);

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
    page,
    pageSize,
    total,
    allCount,
    uncategorizedCount,
    totalPages,
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
