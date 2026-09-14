<template>
  <div class="window-controls d-flex align-center">
    <button
      type="button"
      class="win-btn"
      title="最小化"
      @click.stop="minimize"
    >
      <v-icon size="16">mdi-minus</v-icon>
    </button>
    <button
      type="button"
      class="win-btn"
      :title="isMaximized ? '还原' : '最大化'"
      @click.stop="toggleMaximize"
    >
      <v-icon size="14">{{ isMaximized ? 'mdi-window-restore' : 'mdi-window-maximize' }}</v-icon>
    </button>
    <button
      type="button"
      class="win-btn win-btn--close"
      title="关闭"
      @click.stop="close"
    >
      <v-icon size="16">mdi-close</v-icon>
    </button>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

const win = getCurrentWindow();
const isMaximized = ref(false);
let unlisten: (() => void) | null = null;

async function checkMaximized() {
  try {
    isMaximized.value = await win.isMaximized();
  } catch {
    // 忽略异常
  }
}

async function minimize() {
  await win.minimize();
}

async function toggleMaximize() {
  await win.toggleMaximize();
  await checkMaximized();
}

async function close() {
  await win.close();
}

onMounted(async () => {
  await checkMaximized();
  unlisten = await win.onResized(async () => {
    await checkMaximized();
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
});
</script>

<style scoped>
.window-controls {
  height: 100%;
  -webkit-app-region: no-drag;
  flex-shrink: 0;
  z-index: 100;
}
.win-btn {
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: #F5F5F0;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
  outline: none;
  padding: 0;
  margin: 0;
  border-radius: 0;
  flex-shrink: 0;
}
.win-btn:hover {
  background: rgba(255, 255, 255, 0.12);
}
.win-btn--close:hover {
  background: #E55A5A !important;
  color: #ffffff !important;
}
</style>
