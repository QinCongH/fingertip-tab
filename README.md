# 指尖乐谱 (Fingertip Tabs)

本地吉他谱桌面工具：曲谱管理、四种看谱模式、AI 摇头翻页。基于 **Tauri v2 + Vue 3 + TypeScript + Vuetify 3/4**，所有数据完全保存在本地。

## 功能总览

| 模块 | 功能 |
| :--- | :--- |
| 曲谱库 | 分组侧栏（多对多）、卡片网格、搜索、批量导入、拖拽导入、截图录入、元数据编辑、分页管理 |
| 看谱器 | ① 固定翻页 ② 垂直滚动 ③ 双页水平对齐（滚轮转横向）④ AI 摄像头摇头翻页 |
| 看谱增强 | 自动滚动（像素/秒 或 BPM 换算，空格启停）、全屏（F11/Esc，悬浮工具栏）、缩放（适应宽度/高度/实际）、快捷键自定义、翻页边缘闪光反馈 |
| 数据存储 | 自定义库目录（默认 `文档/FingertipTabs`）、UUID 重命名 + 缩略图、SQLite（sqlx）、存储迁移、一键备份/恢复 ZIP |
| 设置 | 通用（自启/恢复窗口/语言 zh-CN・en）、显示（主题/默认模式/背景色/缩放）、AI 控制（设备/灵敏度/冷却/预览/特效）、快捷键、关于 |

## 开发

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 启动开发环境（Vite + Rust 同时）
```

前置要求：Node 18+、Rust（含 VS Build Tools）、Tauri v2 依赖。

## 构建

```bash
npm run tauri build  # 产出安装包（src-tauri/target/release/bundle）
```

## 数据目录结构

```text
{库目录}/
├── FingertipTabs.db       # SQLite（songs / collections / song_collections / pages）
├── settings.json          # 应用设置（随库迁移）
├── Resources/
│   ├── Images/            # {uuid}.jpg/png/...（UUID 重命名存储）
│   └── Thumbnails/        # {uuid}_thumb.jpg 列表缩略图
└── Backups/
```

应用指针配置位于系统应用数据目录 `config.json`（记录当前库路径、窗口状态、最后打开的曲谱）。

## AI 摇头翻页实现说明

摄像头画面经 canvas 降采样后做帧差，计算水平运动质心；当质心在短时间内从左向右位移超过阈值（灵敏度：低≈55px / 中≈34px / 高≈20px，160px 宽画布）并经过冷却时间（0.5–3.0s 可调）即触发下一页。纯本地计算，不依赖 OpenCV 或联网模型，详见 `src/lib/headshake.ts`；算法行为由 `md/headshake-sim.cjs` 合成帧仿真验证（`node md/headshake-sim.cjs`）。

## 测试素材

`node md/gen-fixtures.cjs` 可生成 4 页测试曲谱图片（`md/fixtures/`），用于导入/翻页/双页对齐等功能验证。
