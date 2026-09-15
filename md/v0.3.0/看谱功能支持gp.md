根据你的最新指示，我重新梳理了需求文档。这份文档移除了练习统计相关的复杂逻辑，专注于核心的**双模式（图片/GP）导入与查看**体验，并最大程度保留现有的存储逻辑。
---
# 📄 指尖乐谱 - 曲谱管理与查看功能需求文档 (V2.0)
## 1. 概述
本项目旨在升级“指尖乐谱”的核心功能，使其不仅能管理图片曲谱，还能原生支持 Guitar Pro (`.gp`) 格式曲谱。系统将根据曲谱类型自动切换查看器和交互模式，同时保持现有文件存储逻辑的稳定性。
**核心变更点**：
1.  **简化统计**：移除练习统计表及相关功能。
2.  **混合导入**：支持在导入时选择曲谱类型（图片/GP）。
3.  **分屏查看**：根据曲谱类型加载不同的渲染逻辑（图片渲染器 vs AlphaTab 引擎）。
4.  **稳定存储**：沿用现有本地文件存储 + SQLite 元数据记录的方式。
---
## 2. 数据库设计变更
为支持查看逻辑的分流，需在现有的 `Songs` 表中增加最小化字段，不改变现有的文件路径存储逻辑。
### 2.1 表结构变更
```sql
-- 现有表: songs
-- 新增字段: format (用于标识曲谱类型，决定前端渲染逻辑)
ALTER TABLE songs ADD COLUMN format TEXT DEFAULT 'image';
-- 字段说明:
-- format: 'image' (图片格式, jpg/png等) | 'gp' (Guitar Pro 格式)
```
### 2.2 数据存储逻辑（保持不变）
*   **图片格式**：
    *   文件物理存储于 `Resources/Images/` 目录。
    *   `local_path` 字段存储图片的绝对路径。
*   **GP 格式**：
    *   文件物理存储于 `Resources/GPFiles/` 目录（建议新建此子目录以示区分）。
    *   `local_path` 字段存储 `.gp` 文件的绝对路径。
---
## 3. 导入功能需求
导入流程需具备类型识别或手动选择能力，确保文件被正确归类。
### 3.1 导入入口
*   位置：主界面“曲谱库”页面的“导入”按钮。
### 3.2 导入交互流程
1.  **点击导入**：弹出“添加曲谱”对话框。
2.  **类型选择（UI关键点）**：
    *   提供 **单选框** 或 **下拉菜单**：`[ 图片曲谱 ]` / `[ Guitar Pro 曲谱 (.gp) ]`。
    *   *默认值*：根据用户最后选择的类型记忆，或默认为“图片曲谱”。
3.  **文件选择**：
    *   根据所选类型，调用 Tauri 文件对话框，自动过滤文件后缀名。
    *   *图片模式*：过滤 `.jpg`, `.png`, `.jpeg`。
    *   *GP模式*：过滤 `.gp`, `.gpx`, `.gp5`, `.gp4`, `.gp3`。
4.  **元数据输入**：
    *   输入曲谱标题（默认填充文件名）。
    *   选择所属分组（从现有的分组列表选择）。
5.  **保存**：
    *   **Rust 后端**：
        *   接收文件流。
        *   根据 `format` 类型，将文件移动/复制到对应的 `Resources/Images/` 或 `Resources/GPFiles/` 目录。
        *   向数据库插入一条记录，设置 `format` 字段。
---
## 4. 查看功能需求 (核心逻辑)
查看页面的核心逻辑是**多态渲染**。主视图容器根据当前曲谱的 `format` 属性动态切换组件。
### 4.1 视图组件划分
1.  **ImageViewer.vue (现有/微调)**
    *   **适用对象**：`format = 'image'` 的曲谱。
    *   **功能**：图片加载、缩放、鼠标拖拽移动、翻页逻辑。
2.  **GpViewer.vue (全新)**
    *   **适用对象**：`format = 'gp'` 的曲谱。
    *   **功能**：基于 AlphaTab 的乐谱渲染、播放控制、轨道管理。
### 4.2 查看流程
1.  用户在左侧曲谱列表点击某首曲谱。
2.  **判断逻辑**：
    *   读取当前曲谱对象的 `format` 字段。
3.  **动态路由/组件切换**：
    *   **如果是 'image'**：
        *   显示 `<ImageViewer :src="song.local_path" />`。
        *   禁用播放控制栏（或隐藏播放相关按钮）。
        *   启用图片缩放/全屏工具。
    *   **如果是 'gp'**：
        *   显示 `<GpViewer :path="song.local_path" />`。
        *   显示播放控制栏（播放、暂停、进度条）。
        *   调用 Rust API 读取 `.gp` 文件的二进制数据喂给 AlphaTab。
### 4.3 GP 格式查看详细功能
参考 TabPilot 的核心体验，集中在 `GpViewer.vue` 中实现：
*   **谱面显示**：
    *   渲染六线谱与五线谱。
    *   支持单页/连续滚动模式切换。
*   **播放控制**：
    *   播放/暂停/停止。
    *   进度条拖拽跳转。
*   **速度控制**：
    *   提供倍速播放（0.5x - 2.0x），实时生效。
*   **轨道混音**：
    *   左侧显示轨道列表（吉他1、吉他2、贝斯等）。
    *   提供 **Mute (静音)** 和 **Solo (独奏)** 开关。
    *   提供简单的音量平衡（可选）。
*   **循环练习**：
    *   支持用户在谱面上选择起止小节。
    *   开启“循环播放”开关，在该区间内重复播放。
---
## 5. 设置功能需求
设置功能分为“通用设置”和“播放设置”。由于移除了统计，设置主要集中在界面和引擎参数上。
### 5.1 通用设置 (现有 + 微调)
*   **默认视图**：设置打开图片谱时的默认缩放比例。
*   **存储路径**：管理 `Resources` 目录的位置（复用现有逻辑）。
### 5.2 GP 播放/显示设置 (新增)
这些设置保存在本地配置文件（如 `settings.json`）中，仅对 `format='gp'` 的查看器生效。
*   **显示选项**：
    *   `[ ] 显示五线谱` (默认开启)。
    *   `[ ] 显示和弦名称` (默认开启)。
    *   `乐谱缩放` (滑块：0.8 - 1.5)。
*   **播放选项**：
    *   `Count-in (预拍)`：播放前倒数几拍 (1, 2, 4 或 关闭)。
    *   `音色库路径`：允许用户自定义 `.sf2` 音色库文件（默认使用内置）。
---
## 6. 技术实现关键点
### 6.1 前端动态渲染逻辑 (Vue 伪代码)
在 `src/views/Home.vue` (主查看页) 中：
```vue
<template>
  <div class="viewer-container">
    <!-- 顶部通用工具栏 -->
    <v-app-bar>
      <v-btn @click="goBack">返回</v-btn>
      <v-spacer></v-spacer>
      <!-- GP 专用控制 (仅在 GP 模式显示) -->
      <template v-if="currentSong?.format === 'gp'">
        <PlaybackControls />
      </template>
    </v-app-bar>
    <!-- 主内容区：根据类型切换组件 -->
    <component 
      :is="viewerComponent" 
      :src="currentSong?.local_path" 
      :format="currentSong?.format"
    />
  </div>
</template>
<script setup lang="ts">
import { computed, defineAsyncComponent } from 'vue'
import ImageViewer from '@/components/ImageViewer.vue'
import GpViewer from '@/components/GpViewer.vue'
const currentSong = ref(null) // 从路由或 store 获取当前曲谱对象
// 核心逻辑：根据 format 决定渲染哪个组件
const viewerComponent = computed(() => {
  if (!currentSong.value) return null
  if (currentSong.value.format === 'gp') {
    return GpViewer
  } else {
    return ImageViewer // 默认为图片查看器
  }
})
</script>
```
### 6.2 后端接口调整
Rust 端只需保证 `import_song` 接口接收 `format` 参数，并将文件存放到对应的子目录即可。
```rust
#[tauri::command]
async fn import_song(
    file_base64: String,
    title: String,
    format: String, // "image" or "gp"
    // ...
) -> Result<i64> {
    let file_data = decode(file_base64)?;
    let dest_path = if format == "gp" {
        storage_dir.join("Resources/GPFiles").join(uuid)
    } else {
        storage_dir.join("Resources/Images").join(uuid)
    };
    // 写入文件...
    // 写入数据库 (INSERT INTO songs ... format=?)
}
```
---
## 7. 总结
通过这份需求文档，我们在现有项目基础上进行了**非侵入式**的功能扩展：
1.  **数据库**：仅增加一个 `format` 字段。
2.  **存储**：延续文件拷贝模式，仅增加分类目录。
3.  **交互**：实现了导入时的类型分流和查看时的组件分流。
这使得“指尖乐谱”既能保持原有的稳定看图功能，又能无缝接入专业的 GP 播放能力。
