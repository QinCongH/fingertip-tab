// OCR 文字识别（Tesseract WASM 版，纯本地推理）
// 语言包（chi_sim/eng）首次使用时从 CDN 下载并缓存在 IndexedDB，之后离线可用
import { createWorker, PSM, type Worker } from "tesseract.js";

let workerPromise: Promise<Worker> | null = null;
let progressSink: ((status: string, progress: number) => void) | null = null;

function getWorker(): Promise<Worker> {
  if (!workerPromise) {
    workerPromise = createWorker(["chi_sim", "eng"], 1, {
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
