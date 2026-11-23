<!-- source: 3769d1d4a6 -->
# React 與 Vue

`@ferrochart/core` 可在任何框架中使用：在畫布掛載時建立圖表，在畫布移除時銷毀它。不需要安裝任何包裝套件。以 CSS 設定畫布尺寸；圖表會跟隨其尺寸與裝置像素比。

## React

```tsx
import { useEffect, useRef } from 'react';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

export function PriceChart({ bars }: { bars: OhlcRow[] }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let chart: FerroChart | undefined;
    let gone = false;
    FerroChart.create(canvas.current!, { theme: 'dark' }).then((c) => {
      if (gone) return c.destroy(); // unmounted while it was starting
      chart = c;
      chart.addCandlestickSeries().setData(bars);
    });
    return () => { gone = true; chart?.destroy(); };
  }, [bars]);
  return <canvas ref={canvas} style={{ width: '100%', height: 480 }} />;
}
```

處理即時資料時，請把系列保存在 ref 中並呼叫 `series.update(bar)`，而不是重新建立圖表。

## Vue

```vue
<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

const props = defineProps<{ bars: OhlcRow[] }>();
const canvas = ref<HTMLCanvasElement>();
let chart: FerroChart | undefined;

onMounted(async () => {
  chart = await FerroChart.create(canvas.value!, { theme: 'dark' });
  chart.addCandlestickSeries().setData(props.bars);
});
onBeforeUnmount(() => chart?.destroy());
</script>

<template>
  <canvas ref="canvas" style="width: 100%; height: 480px" />
</template>
```

## 伺服器端渲染

圖表需要瀏覽器（WebGPU 或 WebGL2）。在 Next.js、Nuxt 等框架中，請只在用戶端建立圖表：如上所示放在 `useEffect`／`onMounted` 中，它們永遠不會在伺服器上執行。
