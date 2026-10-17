// OCR 文字识别（Tesseract WASM 版，纯本地推理）
// worker / WASM 核心 / 语言包（chi_sim、eng）全部随应用打包，
// 从本地加载而非 CDN，避免网络不可达或离线时 OCR 整体失效。
import { createWorker, PSM, type Worker } from "tesseract.js";

// 打包后的静态资源基址（与 tauri.conf.json 的 frontendDist 相对应）
const base = import.meta.env.BASE_URL || "/";

function assetUrl(p: string): string {
  return new URL(`${base.replace(/\/?$/, "/")}${p}`, window.location.href).toString();
}

let workerPromise: Promise<Worker> | null = null;
let progressSink: ((status: string, progress: number) => void) | null = null;

function getWorker(): Promise<Worker> {
  if (!workerPromise) {
    workerPromise = createWorker(["chi_sim", "eng"], 1, {
      workerPath: assetUrl("ocr/worker.min.js"),
      corePath: assetUrl("ocr/"),
      langPath: assetUrl("ocr-lang/"),
      // 本地已提供 gz 版语言包，无需重复下载
      gzip: true,
      logger: (m) => {
        progressSink?.(m.status ?? "", m.progress ?? 0);
      },
    }).then(async (worker) => {
      // 单块文本更接近谱头歌名的排版
      await worker.setParameters({ tessedit_pageseg_mode: PSM.SINGLE_BLOCK });
      return worker;
    });
    // 失败时允许下次重试
    workerPromise.catch(() => {
      workerPromise = null;
    });
  }
  return workerPromise;
}

export interface OcrProgress {
  (status: string, progress: number): void;
}

/** 识别图片中的文字（image 支持 dataURL / Blob / URL） */
export async function recognizeText(
  image: string | Blob,
  onProgress?: OcrProgress,
): Promise<string> {
  const worker = await getWorker();
  progressSink = onProgress ?? null;
  try {
    const { data } = await worker.recognize(image);
    return (data.text ?? "").trim();
  } finally {
    progressSink = null;
  }
}

/** OCR 是否可用（无法初始化 worker 时视为不可用） */
export function isOcrPending(): boolean {
  return workerPromise === null;
}
