<template>
  <div
    class="shortcut-box"
    :class="{ recording }"
    tabindex="0"
    @click="startRecording"
  >
    <template v-if="recording">
      <v-icon icon="mdi-keyboard" size="16" class="mr-2" />
      <span class="font-weight-bold">{{ t("settings.shortcut_recording") }}</span>
    </template>
    <template v-else>
      <v-icon icon="mdi-key-outline" size="16" class="mr-2" style="opacity: 0.5" />
      <span class="font-weight-black st-mono" style="font-size: 13px; opacity: 1">
        {{ display || "?" }}
      </span>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { t } from "@/i18n";
import { eventToCombo, formatCombo } from "@/lib/shortcuts";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

const recording = ref(false);
const display = computed(() => formatCombo(props.modelValue));

function startRecording() {
  if (recording.value) return;
  recording.value = true;
  window.addEventListener("keydown", onKey, true);
}

function onKey(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    stop();
    return;
  }
  const combo = eventToCombo(e);
  if (!combo) return; // 只按修饰键时继续等待
  emit("update:modelValue", combo);
  stop();
}

function stop() {
  recording.value = false;
  window.removeEventListener("keydown", onKey, true);
}

onUnmounted(stop);
</script>

<style scoped>
.shortcut-box {
  display: inline-flex;
  align-items: center;
  min-width: 200px;
  padding: 8px 14px;
  border: 2px solid var(--st-border);
  cursor: pointer;
  transition: all 0.3s ease;
  background: var(--st-grey);
}
.shortcut-box:hover {
  border-color: rgba(255, 107, 107, 0.6);
}
.shortcut-box.recording {
  border-color: var(--st-coral);
  background: rgba(255, 107, 107, 0.1);
}
</style>
