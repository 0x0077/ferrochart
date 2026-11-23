<!-- source: 3769d1d4a6 -->
# React 与 Vue

`@ferrochart/core` 可以在任何框架中使用：在 canvas 挂载时创建图表，在它被移除时销毁图表。无需安装任何封装包。用 CSS 设置 canvas 的尺寸；图表会跟随其尺寸和设备像素比。

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

对于实时数据，请把系列保存在 ref 中并调用 `series.update(bar)`，而不是重新创建图表。

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

## 服务端渲染

图表需要浏览器（WebGPU 或 WebGL2）。在 Next.js、Nuxt 及类似框架中，只在客户端创建图表：像上面那样放在 `useEffect` / `onMounted` 中，它们永远不会在服务端运行。
