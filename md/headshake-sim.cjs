// HeadShakeDetector 核心算法的合成帧验证（与 src/lib/headshake.ts 逻辑一致的移植）
// 场景：画面中一个亮斑从左向右大幅移动（模拟摇头），应触发一次；随后冷却期内不再触发

const W = 160, H = 120;
const THRESHOLD = 34; // medium
const COOLDOWN_MS = 1500;
const REST_RESET_MS = 1200;
const MIN_MOTION_PIXELS = 10;

function makeFrame(spotX) {
  const g = new Uint8Array(W * H);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) g[y * W + x] = 30; // 暗背景
  // 亮斑 20x40
  for (let y = 40; y < 80; y++)
    for (let x = Math.round(spotX) - 10; x <= Math.round(spotX) + 10; x++)
      if (x >= 0 && x < W) g[y * W + x] = 220;
  return g;
}

function centroid(prev, cur) {
  let ws = 0, wx = 0;
  for (let i = 0; i < cur.length; i++) {
    const d = Math.abs(cur[i] - prev[i]);
    if (d > 28) { ws += d; wx += d * (i % W); }
  }
  if (ws < MIN_MOTION_PIXELS * 28) return null;
  return wx / ws;
}

// --- 模拟检测循环 ---
let prev = makeFrame(60); // 静止基准帧
let baseline = null, lastMotion = 0, lastTrigger = -1e9;
let triggers = 0;
let t = 0;
const step = 66; // ms，~15fps

function sample(gray, now) {
  const c = centroid(prev, gray);
  prev = gray;
  if (c === null) {
    if (baseline !== null && now - lastMotion > REST_RESET_MS) baseline = null;
    return;
  }
  lastMotion = now;
  if (baseline === null) { baseline = c; return; }
  const dx = c - baseline;
  if (dx >= THRESHOLD && now - lastTrigger >= COOLDOWN_MS) {
    lastTrigger = now;
    baseline = null;
    triggers++;
  }
}

// 阶段1：静止 1s（20 帧）→ 不触发
for (let i = 0; i < 20; i++) { t += step; sample(makeFrame(60), t); }
// 阶段2：亮斑 60→110 快速右移（摇头向右），6 帧
for (let i = 1; i <= 6; i++) { t += step; sample(makeFrame(60 + i * 9), t); }
console.log("after sweep right: triggers =", triggers, "(expect 1)");
// 阶段3：继续抖动 ±8px 1s → 冷却期内不应触发
for (let i = 0; i < 15; i++) { t += step; sample(makeFrame(100 + (i % 2 ? 8 : -8)), t); }
console.log("during cooldown: triggers =", triggers, "(expect 1)");
// 阶段4：静止 1.3s 后再次大幅右移 → 冷却已过，应再次触发
for (let i = 0; i < 20; i++) { t += step; sample(makeFrame(100), t); }
for (let i = 1; i <= 6; i++) { t += step; sample(makeFrame(100 + i * 8), t); }
console.log("after second sweep: triggers =", triggers, "(expect 2)");
// 阶段5：向左大幅移动（反方向）→ 不触发（只识别从左往右）
for (let i = 0; i < 20; i++) { t += step; sample(makeFrame(120), t); }
for (let i = 1; i <= 6; i++) { t += step; sample(makeFrame(120 - i * 9), t); }
console.log("after sweep left: triggers =", triggers, "(expect 2)");

const pass = triggers === 2;
console.log(pass ? "ALGORITHM PASS" : "ALGORITHM FAIL");
process.exit(pass ? 0 : 1);
