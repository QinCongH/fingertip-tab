// AI 动作捕获：摄像头画面运动质心追踪，识别「大幅度从左往右摇头」翻页
// 纯浏览器实现（canvas 帧差 + 水平运动质心），无需外部依赖、完全本地离线

export type Sensitivity = "low" | "medium" | "high";

/** 灵敏度 → 触发阈值（160px 宽画布内的水平位移像素） */
export const SENSITIVITY_THRESHOLD: Record<Sensitivity, number> = {
  low: 55,
  medium: 34,
  high: 20,
};

const CANVAS_W = 160;
const CANVAS_H = 120;
const SAMPLE_INTERVAL_MS = 66; // ~15fps
const REST_RESET_MS = 1200; // 运动静止多久后重置基准点
const MIN_MOTION_PIXELS = 10; // 有效运动的最少像素数

export interface HeadShakeOptions {
  sensitivity: Sensitivity;
  cooldownMs: number;
  onTrigger: () => void;
  onDebug?: (info: { dx: number; threshold: number }) => void;
}

export class HeadShakeDetector {
  private video: HTMLVideoElement | null = null;
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private stream: MediaStream | null = null;
  private timer: number | null = null;
  private prevGray: Uint8Array | null = null;
  private baselineX: number | null = null;
  private lastMotionAt = 0;
  private lastTriggerAt = 0;
  private running = false;
  private opts: HeadShakeOptions;

  constructor(opts: HeadShakeOptions) {
    this.opts = opts;
    this.canvas = document.createElement("canvas");
    this.canvas.width = CANVAS_W;
    this.canvas.height = CANVAS_H;
    this.ctx = this.canvas.getContext("2d", { willReadFrequently: true })!;
  }

  get isRunning() {
    return this.running;
  }

  updateOptions(opts: Partial<HeadShakeOptions>) {
    this.opts = { ...this.opts, ...opts };
  }

  /** 打开摄像头并开始检测 */
  async start(deviceId?: string): Promise<void> {
    if (this.running) return;
    const constraints: MediaStreamConstraints = {
      audio: false,
      video: deviceId
        ? { deviceId: { exact: deviceId }, width: { ideal: 320 }, height: { ideal: 240 } }
        : { width: { ideal: 320 }, height: { ideal: 240 } },
    };
    this.stream = await navigator.mediaDevices.getUserMedia(constraints);
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = this.stream;
    await video.play();
    this.video = video;
    this.prevGray = null;
    this.baselineX = null;
    this.running = true;
    this.timer = window.setInterval(() => this.sample(), SAMPLE_INTERVAL_MS);
  }

  stop() {
    if (this.timer !== null) {
      window.clearInterval(this.timer);
      this.timer = null;
    }
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    if (this.video) {
      this.video.srcObject = null;
      this.video = null;
    }
    this.prevGray = null;
    this.baselineX = null;
    this.running = false;
  }

  /** 给预览小窗用的视频元素 */
  attachPreview(el: HTMLVideoElement) {
    if (this.video && this.stream) {
      el.srcObject = this.stream;
      el.muted = true;
      el.playsInline = true;
      el.play().catch(() => undefined);
    }
  }

  /** 单帧采样：帧差 → 水平运动质心 → 位移判度 */
  private sample() {
    const video = this.video;
    if (!video || video.readyState < 2) return;

    this.ctx.drawImage(video, 0, 0, CANVAS_W, CANVAS_H);
    const frame = this.ctx.getImageData(0, 0, CANVAS_W, CANVAS_H).data;

    // 转灰度
    const gray = new Uint8Array(CANVAS_W * CANVAS_H);
    for (let i = 0, p = 0; i < gray.length; i++, p += 4) {
      gray[i] = (frame[p] * 299 + frame[p + 1] * 587 + frame[p + 2] * 114) / 1000;
    }

    const prev = this.prevGray;
    this.prevGray = gray;
    if (!prev) return; // 第一帧只做基准

    // 帧差 + 运动质心
    let weightSum = 0;
    let weightedX = 0;
    for (let i = 0; i < gray.length; i++) {
      const d = Math.abs(gray[i] - prev[i]);
      if (d > 28) {
        const x = i % CANVAS_W;
        weightSum += d;
        weightedX += d * x;
      }
    }
    const now = performance.now();
    if (weightSum < MIN_MOTION_PIXELS * 28) {
      // 无明显运动：静止超过阈值时间则重置基准
      if (this.baselineX !== null && now - this.lastMotionAt > REST_RESET_MS) {
        this.baselineX = null;
      }
      return;
    }
    const centroid = weightedX / weightSum;
    this.lastMotionAt = now;

    if (this.baselineX === null) {
      this.baselineX = centroid;
      return;
    }

    const dx = centroid - this.baselineX; // 从左往右为正
    const threshold = SENSITIVITY_THRESHOLD[this.opts.sensitivity] ?? 34;
    this.opts.onDebug?.({ dx, threshold });

    if (dx >= threshold) {
      const sinceTrigger = now - this.lastTriggerAt;
      if (sinceTrigger >= this.opts.cooldownMs) {
        this.lastTriggerAt = now;
        this.baselineX = null; // 触发后等待运动重新落定
        this.opts.onTrigger();
      }
    }
  }
}

/** 枚举摄像头设备（首次调用会请求权限以获得设备名） */
export async function listCameras(): Promise<MediaDeviceInfo[]> {
  try {
    let devices = await navigator.mediaDevices.enumerateDevices();
    const labeled = devices.some((d) => d.kind === "videoinput" && d.label);
    if (!labeled) {
      const stream = await navigator.mediaDevices.getUserMedia({ video: true });
      stream.getTracks().forEach((t) => t.stop());
      devices = await navigator.mediaDevices.enumerateDevices();
    }
    return devices.filter((d) => d.kind === "videoinput");
  } catch {
    return [];
  }
}
