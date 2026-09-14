import { ref } from "vue";
import { defineStore } from "pinia";

export type ToastType = "success" | "error" | "info";

export const useToastStore = defineStore("toast", () => {
  const show = ref(false);
  const text = ref("");
  const type = ref<ToastType>("info");
  let timer: number | null = null;

  function push(message: string, toastType: ToastType = "info", timeout = 3200) {
    text.value = message;
    type.value = toastType;
    show.value = true;
    if (timer !== null) window.clearTimeout(timer);
    timer = window.setTimeout(() => (show.value = false), timeout);
  }

  function success(message: string) {
    push(message, "success");
  }
  function error(message: string) {
    push(message, "error", 4200);
  }

  return { show, text, type, push, success, error };
});

/** 把任意异常转为可读字符串 */
export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}
