import { computed } from "vue";

export type LayoutClass = "layout-1" | "layout-2" | "layout-3" | "layout-4";

export function useGridLayout(imageCount: () => number) {
  // 根据图片数量返回布局类名
  const layoutClass = computed<LayoutClass>(() => {
    const n = imageCount();
    if (n <= 1) return "layout-1";
    if (n === 2) return "layout-2";
    if (n === 3) return "layout-3";
    return "layout-4"; // 4张及以上（超出部分翻屏）
  });

  // 当前屏应显示的图片切片（支持 >4 张时分页）
  const pageImages = <T>(images: T[], page: number, perScreen = 4): T[] => {
    const start = (page - 1) * perScreen;
    return images.slice(start, start + perScreen);
  };

  const totalPages = (total: number, perScreen = 4): number =>
    Math.max(1, Math.ceil(total / perScreen));

  return { layoutClass, pageImages, totalPages };
}
