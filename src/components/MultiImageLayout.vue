<template>
  <div class="layout-outer-wrap">
    <div
      ref="containerRef"
      class="multi-image-layout"
      :class="computedLayoutClass"
      :style="dynamicGridStyle"
    >
      <div
        v-for="(img, index) in images"
        :key="img.uuid || index"
        class="grid-cell"
        :class="`cell-${index + 1}`"
        @click="$emit('select', img, index)"
      >
        <img
          :src="pageSrc(img)"
          :style="{ objectFit: fitMode }"
          draggable="false"
          @load="onImageLoaded($event, index)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { PageDto } from "@/lib/tauri";

const props = withDefaults(
  defineProps<{
    images: PageDto[];
    pageSrc: (page: PageDto) => string;
    gap?: number;
    background?: string;
    fitMode?: "contain" | "cover";
  }>(),
  {
    gap: 8,
    background: "transparent",
    fitMode: "contain",
  },
);

defineEmits<{
  (e: "select", img: PageDto, index: number): void;
}>();

const containerRef = ref<HTMLElement | null>(null);
const containerWidth = ref(0);
const containerHeight = ref(0);

// 记录各张图片的原始宽高比：ratio = width / height
const imageRatios = ref<{ [index: number]: number }>({});

let resizeObserver: ResizeObserver | null = null;

function updateContainerSize() {
  if (containerRef.value) {
    const parent = containerRef.value.parentElement;
    if (parent) {
      containerWidth.value = parent.clientWidth;
      containerHeight.value = parent.clientHeight;
    } else {
      containerWidth.value = containerRef.value.clientWidth;
      containerHeight.value = containerRef.value.clientHeight;
    }
  }
}

function onImageLoaded(e: Event, index: number) {
  const img = e.target as HTMLImageElement;
  if (img && img.naturalHeight > 0) {
    imageRatios.value = {
      ...imageRatios.value,
      [index]: img.naturalWidth / img.naturalHeight,
    };
  }
}

onMounted(() => {
  updateContainerSize();
  if (typeof ResizeObserver !== "undefined" && containerRef.value?.parentElement) {
    resizeObserver = new ResizeObserver(() => {
      updateContainerSize();
    });
    resizeObserver.observe(containerRef.value.parentElement);
  }
  window.addEventListener("resize", updateContainerSize);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
  window.removeEventListener("resize", updateContainerSize);
});

// 当切换页/图片列表变化时清空缓存比率
watch(
  () => props.images,
  () => {
    imageRatios.value = {};
  },
);

const n = computed(() => props.images.length);

/**
 * 判断当前屏图片等高排成单行时，总宽度是否不超过可用宽度
 */
const canFitSingleRow = computed(() => {
  if (n.value <= 1) return true;
  if (containerWidth.value <= 0 || containerHeight.value <= 0) return true;

  const availHeight = Math.max(100, containerHeight.value - 16);
  const availWidth = Math.max(100, containerWidth.value - 16);

  let totalWidth = 0;
  for (let i = 0; i < n.value; i++) {
    const ratio = imageRatios.value[i] || 0.707;
    totalWidth += availHeight * ratio;
  }
  totalWidth += (n.value - 1) * props.gap;

  return totalWidth <= availWidth;
});

const computedLayoutClass = computed(() => {
  if (n.value <= 1) return "layout-1";
  if (canFitSingleRow.value) return "layout-row-fit";
  if (n.value === 2) return "layout-2";
  if (n.value === 3) return "layout-3";
  return "layout-4";
});

/**
 * 比例感知动态分栏核心：
 * 将每张图的真实比例映射为 Grid 的 fr 列宽权重（如 0.77fr 0.35fr）
 * 并将网格总宽度约束为实际所需宽度（通过 max-width 配合 margin: auto 紧贴居中）
 * 从而使两张或多张图片的实际渲染高度完全自动对齐！
 */
const dynamicGridStyle = computed(() => {
  const count = n.value;
  const availHeight = Math.max(100, containerHeight.value - 16);
  const availWidth = Math.max(100, containerWidth.value - 16);

  if (count <= 1) {
    const r = imageRatios.value[0] || 0.707;
    const targetW = Math.min(availWidth, availHeight * r);
    return {
      display: "grid",
      gridTemplateColumns: "1fr",
      width: `${Math.round(targetW)}px`,
      height: "100%",
      maxHeight: "100%",
      margin: "0 auto",
      gap: `${props.gap}px`,
      background: props.background || "transparent",
    };
  }

  // 单行紧贴模式：按比例分配各列权重（如 0.77fr 0.35fr）
  if (canFitSingleRow.value) {
    const colWeights: string[] = [];
    let sumW = 0;
    for (let i = 0; i < count; i++) {
      const r = Number((imageRatios.value[i] || 0.707).toFixed(3));
      colWeights.push(`${r}fr`);
      sumW += availHeight * r;
    }
    sumW += (count - 1) * props.gap;
    const finalW = Math.min(availWidth, Math.round(sumW));

    return {
      display: "grid",
      gridTemplateColumns: colWeights.join(" "),
      gridTemplateRows: "1fr",
      width: `${finalW}px`,
      height: "100%",
      maxHeight: "100%",
      margin: "0 auto",
      gap: `${props.gap}px`,
      background: props.background || "transparent",
    };
  }

  // 超过单行宽度时：
  if (count === 2) {
    const r1 = Number((imageRatios.value[0] || 0.707).toFixed(3));
    const r2 = Number((imageRatios.value[1] || 0.707).toFixed(3));
    return {
      display: "grid",
      gridTemplateColumns: `${r1}fr ${r2}fr`,
      gridTemplateRows: "1fr",
      width: "100%",
      height: "100%",
      maxHeight: "100%",
      margin: "0 auto",
      gap: `${props.gap}px`,
      background: props.background || "transparent",
    };
  }

  if (count === 3) {
    // 3张图：左大（全高 r0），右上下二叠（平均各占半高，等效并排列宽为 0.5 * max(r1, r2)）
    const r0 = Number((imageRatios.value[0] || 0.707).toFixed(3));
    const r1 = Number((imageRatios.value[1] || 0.707).toFixed(3));
    const r2 = Number((imageRatios.value[2] || 0.707).toFixed(3));
    const rightColWeight = Number((0.5 * Math.max(r1, r2)).toFixed(3));

    return {
      display: "grid",
      gridTemplateColumns: `${r0}fr ${rightColWeight}fr`,
      gridTemplateRows: "1fr 1fr",
      gridTemplateAreas: `"main top" "main bottom"`,
      width: "100%",
      height: "100%",
      maxHeight: "100%",
      margin: "0 auto",
      gap: `${props.gap}px`,
      background: props.background || "transparent",
    };
  }

  // 4张图：2x2 田字
  const col1 = Number(Math.max(imageRatios.value[0] || 0.707, imageRatios.value[2] || 0.707).toFixed(3));
  const col2 = Number(Math.max(imageRatios.value[1] || 0.707, imageRatios.value[3] || 0.707).toFixed(3));

  return {
    display: "grid",
    gridTemplateColumns: `${col1}fr ${col2}fr`,
    gridTemplateRows: "1fr 1fr",
    width: "100%",
    height: "100%",
    maxHeight: "100%",
    margin: "0 auto",
    gap: `${props.gap}px`,
    background: props.background || "transparent",
  };
});
</script>

<style scoped>
.layout-outer-wrap {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: safe center;
  overflow: auto;
  box-sizing: border-box;
}

.multi-image-layout {
  padding: 8px;
  box-sizing: border-box;
  justify-content: safe center;
  align-content: center;
  margin: auto;
}

/* 单元格基础 */
.grid-cell {
  overflow: hidden;
  border-radius: 4px;
  position: relative;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  min-height: 0;
  width: 100%;
  height: 100%;
}

.grid-cell img {
  width: 100%;
  height: 100%;
  user-select: none;
  border-radius: 2px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}

/* 3张图区域映射 */
.layout-3 .cell-1 {
  grid-area: main;
}
.layout-3 .cell-2 {
  grid-area: top;
}
.layout-3 .cell-3 {
  grid-area: bottom;
}
</style>
