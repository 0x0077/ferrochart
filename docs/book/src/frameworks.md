# React and Vue

`@ferrochart/core` works in any framework: create the chart when the canvas
is mounted and destroy it when it is removed. There are no wrapper packages
to install. Size the canvas with CSS; the chart follows its size and the
device pixel ratio.

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

For live data, keep the series in a ref and call `series.update(bar)` instead
of recreating the chart.

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

## Server-side rendering

The chart needs a browser (WebGPU or WebGL2). In Next.js, Nuxt and similar
frameworks, create it only on the client: in `useEffect` / `onMounted` as
above, which never run on the server.
