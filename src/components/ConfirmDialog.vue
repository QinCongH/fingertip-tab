<template>
  <v-dialog v-model="visible" max-width="460" persistent>
    <v-card class="pa-6">
      <div class="st-display text-h6 mb-2" :class="{ 'text-error': danger }">
        <v-icon v-if="danger" icon="mdi-alert" class="mr-2" />
        {{ title }}
      </div>
      <div class="text-body-2 opacity-80" style="white-space: pre-line">{{ text }}</div>
      <div class="d-flex justify-end ga-2 mt-6">
        <v-btn variant="text" @click="visible = false">{{ t("common.cancel") }}</v-btn>
        <v-btn :color="danger ? 'error' : 'primary'" @click="confirm">{{ t("common.confirm") }}</v-btn>
      </div>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { t } from "@/i18n";

const props = defineProps<{
  modelValue: boolean;
  title: string;
  text: string;
  danger?: boolean;
}>();
const emit = defineEmits<{ (e: "update:modelValue", v: boolean): void; (e: "confirm"): void }>();

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit("update:modelValue", v),
});

function confirm() {
  visible.value = false;
  emit("confirm");
}
</script>
