// 生成测试用吉他谱图片（BMP 24bit），模拟 4 页曲谱
const fs = require("fs");
const path = require("path");

const W = 1240;
const H = 1600;
const OUT_DIR = path.join(__dirname, "fixtures");

function bmpHeader() {
  const rowSize = W * 3; // 1240*3=3720, 已是 4 的倍数
  const pixelBytes = rowSize * H;
  const fileSize = 54 + pixelBytes;
  const b = Buffer.alloc(54);
  b.write("BM", 0);
  b.writeUInt32LE(fileSize, 2);
  b.writeUInt32LE(54, 10);
  b.writeUInt32LE(40, 14); // info header size
  b.writeInt32LE(W, 18);
  b.writeInt32LE(H, 22);
  b.writeUInt16LE(1, 26);
  b.writeUInt16LE(24, 28);
  b.writeUInt32LE(0, 30); // BI_RGB
  b.writeUInt32LE(pixelBytes, 34);
  return b;
}

/** 在顶部画页码色带 + 画 6 根弦线与若干竖线 */
function renderPage(page, total) {
  const rowSize = W * 3;
  const buf = Buffer.alloc(rowSize * H);
  const set = (x, y, r, g, bl) => {
    if (x < 0 || x >= W || y < 0 || y >= H) return;
    const off = (H - 1 - y) * rowSize + x * 3; // BMP 自底向上
    buf[off] = bl;
    buf[off + 1] = g;
    buf[off + 2] = r;
  };
  const fillRect = (x0, y0, w, h, r, g, bl) => {
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) set(x, y, r, g, bl);
  };

  // 白底
  fillRect(0, 0, W, H, 250, 250, 245);
  // 页码色带（每页不同颜色，肉眼可辨页序）
  const colors = [
    [255, 107, 107],
    [91, 141, 239],
    [76, 175, 125],
    [245, 166, 35],
  ];
  const [r, g, b2] = colors[(page - 1) % colors.length];
  fillRect(0, 0, W, 120, r, g, b2);
  // 大页码块（数字用条形数量表示）
  for (let i = 0; i < page; i++) {
    fillRect(60 + i * 90, 20, 60, 80, 255, 255, 255);
  }
  // 弦线 ×6
  for (let s = 0; s < 6; s++) {
    const y = 320 + s * 90;
    fillRect(80, y, W - 160, 5, 40, 40, 40);
  }
  // 小节竖线
  for (let x = 80; x <= W - 80; x += 180) {
    fillRect(x, 320, 4, 450, 40, 40, 40);
  }
  // 第二组谱行
  for (let s = 0; s < 6; s++) {
    const y = 900 + s * 90;
    fillRect(80, y, W - 160, 5, 40, 40, 40);
  }
  for (let x = 80; x <= W - 80; x += 180) {
    fillRect(x, 900, 4, 450, 40, 40, 40);
  }
  // 底部装饰
  fillRect(80, 1480, W - 160, 10, 200, 200, 195);
  return buf;
}

fs.mkdirSync(OUT_DIR, { recursive: true });
const header = bmpHeader();
for (let page = 1; page <= 4; page++) {
  const file = Buffer.concat([header, renderPage(page, 4)]);
  const name = path.join(OUT_DIR, `test-tab-page${page}.bmp`);
  fs.writeFileSync(name, file);
  console.log("written", name, file.length, "bytes");
}
console.log("DONE");
