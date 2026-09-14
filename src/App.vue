<template>
  <v-app>
    <router-view />
    <v-snackbar
      v-model="toast.show"
      :timeout="-1"
      :color="toastColor"
      location="top right"
      variant="flat"
      class="st-snackbar"
    >
      <div class="d-flex align-center ga-3">
        <v-icon :icon="toastIcon" size="20" />
        <span class="font-weight-medium">{{ toast.text }}</span>
      </div>
      <template #actions>
        <v-btn icon="mdi-close" variant="text" size="small" @click="toast.show = false" />
      </template>
    </v-snackbar>
  </v-app>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { useTheme } from "vuetify";
import { api } from "@/lib/tauri";
import { useToastStore } from "@/stores/toast";
import { useSettingsStore } from "@/stores/settings";

const toast = useToastStore();
const settings = useSettingsStore();
const router = useRouter();
const theme = useTheme();

const toastColor = computed(() =>
  toast.type === "error" ? "#E55A5A" : toast.type === "success" ? "#333333" : "#333333",
);
const toastIcon = computed(() =>
  toast.type === "error" ? "mdi-alert-circle" : toast.type === "success" ? "mdi-check-circle" : "mdi-information",
);

// 主题即时生效
watch(
  () => [settings.settings.appearance.theme, settings.loaded] as const,
  () => {
    if (!settings.loaded) return;
    theme.global.name.value = settings.resolvedThemeName();
    settings.applyThemeClass();
  },
  { immediate: true },
);

onMounted(async () => {
  await settings.load();
  theme.global.name.value = settings.resolvedThemeName();
  settings.applyThemeClass();

  // 启动时恢复上次打开的曲谱
  try {
    const cfg = await api.getAppConfig();
    if (settings.settings.general.restore_window && cfg.lastSongId) {
      const exists = await api.getSong(cfg.lastSongId).catch(() => null);
      if (exists) void router.push(`/viewer/${cfg.lastSongId}`);
    }
  } catch {
    // 忽略恢复失败
  }
});
</script>

<style>
.v-snackbar__wrapper {
  border-radius: 0 !important;
}
</style>
