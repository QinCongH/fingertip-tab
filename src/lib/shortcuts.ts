// 快捷键组合的捕获与匹配
// 组合格式与 KeyboardEvent.key 对齐："ArrowRight"、"F11"、"Space"、"Ctrl+K"

const NAMED_ALIASES: Record<string, string> = {
  " ": "Space",
  Spacebar: "Space",
  Esc: "Escape",
  Del: "Delete",
  Up: "ArrowUp",
  Down: "ArrowDown",
  Left: "ArrowLeft",
  Right: "ArrowRight",
};

/** 把 KeyboardEvent 转换为标准组合字符串（无实际按键时返回 null） */
export function eventToCombo(e: KeyboardEvent): string | null {
  const raw = e.key;
  if (!raw) return null;
  let key = NAMED_ALIASES[raw] ?? raw;
  if (key.length === 1) key = key.toUpperCase();
  if (key === "Control" || key === "Shift" || key === "Alt" || key === "Meta") {
    return null; // 仅按下修饰键时不记录
  }

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(key);
  return parts.join("+");
}

/** 展示用格式 */
export function formatCombo(combo: string): string {
  return combo
    .split("+")
    .map((p) => {
      if (p === "ArrowLeft") return "←";
      if (p === "ArrowRight") return "→";
      if (p === "ArrowUp") return "↑";
      if (p === "ArrowDown") return "↓";
      if (p === "Escape") return "Esc";
      return p;
    })
    .join(" + ");
}

/** 判断键盘事件是否命中组合 */
export function matchCombo(e: KeyboardEvent, combo: string | undefined | null): boolean {
  if (!combo) return false;
  return eventToCombo(e) === combo;
}

/** 是否在文本输入控件中（此时不响应看谱快捷键） */
export function isTypingTarget(e: KeyboardEvent): boolean {
  const el = e.target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}
