<template>
  <v-navigation-drawer permanent :width="252" class="st-panel">
    <!-- 品牌区 -->
    <div class="pa-5 d-flex align-center ga-3">
      <div class="brand-block">
        <img src="/logo.svg" alt="Logo" class="brand-logo-img" />
      </div>
      <div class="min-width-0">
        <div class="st-display" style="font-size: 19px">{{ t("app.name") }}</div>
        <div class="st-mono">FINGERTIP TABS</div>
      </div>
    </div>

    <div class="px-3 flex-grow-1" style="overflow-y: auto">
      <div
        v-for="item in fixedItems"
        :key="item.value"
        class="st-nav-item pa-3 d-flex align-center ga-3 st-clickable"
        :class="{ 'st-nav-item--active': modelValue === item.value }"
        @click="select(item.value)"
      >
        <v-icon size="20">{{ item.icon }}</v-icon>
        <span class="flex-grow-1 text-body-2 font-weight-bold">{{ item.label }}</span>
        <span class="st-mono">{{ item.count }}</span>
      </div>

      <div class="st-divider my-3 mx-2"></div>

      <div
        v-for="col in library.collections"
        :key="col.id"
        class="st-nav-item pa-3 d-flex align-center ga-3 st-clickable group-item"
        :class="{ 'st-nav-item--active': modelValue === col.id }"
        @click="select(col.id)"
      >
        <v-icon size="20">mdi-playlist-music</v-icon>
        <span class="flex-grow-1 text-body-2 font-weight-bold text-truncate">{{ col.name }}</span>
        <span class="st-mono">{{ col.songCount }}</span>
        <div class="item-actions d-flex ga-1">
          <v-icon
            size="16"
            icon="mdi-pencil-outline"
            class="st-clickable"
            @click.stop="startRename(col)"
          />
          <v-icon
            size="16"
            icon="mdi-trash-can-outline"
            class="st-clickable"
            @click.stop="askDelete(col)"
          />
        </div>
      </div>
    </div>

    <!-- 底部操作 -->
    <div class="pa-3">
      <v-btn block variant="tonal" prepend-icon="mdi-plus" @click="newGroupOpen = true">
        {{ t("library.new_group") }}
      </v-btn>
    </div>

    <!-- 新建分组 -->
    <v-dialog v-model="newGroupOpen" max-width="420">
      <v-card class="pa-6">
        <div class="st-display text-h6 mb-4">{{ t("library.new_group") }}</div>
        <v-text-field
          v-model="newGroupName"
          :label="t('library.group_name')"
          variant="underlined"
          hide-details
          autofocus
          @keyup.enter="createGroup"
        />
        <div class="d-flex justify-end ga-2 mt-6">
          <v-btn variant="text" @click="newGroupOpen = false">{{ t("common.cancel") }}</v-btn>
          <v-btn color="primary" @click="createGroup">{{ t("common.confirm") }}</v-btn>
        </div>
      </v-card>
    </v-dialog>

    <!-- 重命名分组 -->
    <v-dialog v-model="renameOpen" max-width="420">
      <v-card class="pa-6">
        <div class="st-display text-h6 mb-4">{{ t("library.group_rename") }}</div>
        <v-text-field
          v-model="renameValue"
          :label="t('library.group_name')"
          variant="underlined"
          hide-details
          autofocus
          @keyup.enter="doRename"
        />
        <div class="d-flex justify-end ga-2 mt-6">
          <v-btn variant="text" @click="renameOpen = false">{{ t("common.cancel") }}</v-btn>
          <v-btn color="primary" @click="doRename">{{ t("common.confirm") }}</v-btn>
        </div>
      </v-card>
    </v-dialog>

    <ConfirmDialog
      v-model="deleteOpen"
      :title="t('library.delete')"
      :text="t('library.group_delete_text', { name: deleteTarget?.name ?? '' })"
      danger
      @confirm="doDelete"
    />
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { api, type CollectionDto } from "@/lib/tauri";
import { t } from "@/i18n";
import { useLibraryStore } from "@/stores/library";
import { useToastStore, errText } from "@/stores/toast";
import ConfirmDialog from "@/components/ConfirmDialog.vue";

type ActiveValue = number | "all" | "uncategorized";

const props = defineProps<{ modelValue: ActiveValue }>();
const emit = defineEmits<{ (e: "update:modelValue", v: ActiveValue): void }>();

const library = useLibraryStore();
const toast = useToastStore();

const uncategorizedCount = computed(
  () => library.songs.filter((s) => s.collectionIds.length === 0).length,
);

const fixedItems = computed(() => [
  { value: "all" as const, icon: "mdi-library-music", label: t("library.title"), count: library.songs.length },
  { value: "uncategorized" as const, icon: "mdi-tag-outline", label: t("library.uncategorized"), count: uncategorizedCount.value },
]);

function select(v: ActiveValue) {
  emit("update:modelValue", v);
}

// 新建
const newGroupOpen = ref(false);
const newGroupName = ref("");
async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  try {
    const col = await api.createCollection(name);
    await library.loadCollections();
    toast.success(t("library.group_created"));
    newGroupOpen.value = false;
    newGroupName.value = "";
    select(col.id);
  } catch (e) {
    toast.error(errText(e));
  }
}

// 重命名
const renameOpen = ref(false);
const renameValue = ref("");
let renameTarget: CollectionDto | null = null;
function startRename(col: CollectionDto) {
  renameTarget = col;
  renameValue.value = col.name;
  renameOpen.value = true;
}
async function doRename() {
  if (!renameTarget) return;
  try {
    await api.renameCollection(renameTarget.id, renameValue.value.trim());
    await library.loadCollections();
    renameOpen.value = false;
  } catch (e) {
    toast.error(errText(e));
  }
}

// 删除
const deleteOpen = ref(false);
const deleteTarget = ref<CollectionDto | null>(null);
function askDelete(col: CollectionDto) {
  deleteTarget.value = col;
  deleteOpen.value = true;
}
async function doDelete() {
  const target = deleteTarget.value;
  if (!target) return;
  try {
    await api.deleteCollection(target.id);
    await library.loadCollections();
    if (props.modelValue === target.id) select("all");
  } catch (e) {
    toast.error(errText(e));
  }
}
</script>

<style scoped>
.brand-block {
  width: 45px;
  height: 45px;
  background: var(--st-gray-900);
  /* border: 1px solid rgba(255, 255, 255, 0.15); */
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  padding: 6px;
  box-sizing: border-box;
}
.brand-logo-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
}
.group-item .item-actions {
  display: none;
}
.group-item:hover .item-actions {
  display: flex;
}
.min-width-0 {
  min-width: 0;
}
</style>
