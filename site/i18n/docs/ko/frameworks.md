<!-- source: 3769d1d4a6 -->
# React와 Vue

`@ferrochart/core`는 어떤 프레임워크에서든 동작합니다. 캔버스가 마운트되면 차트를
만들고 제거되면 차트를 제거하세요. 설치할 래퍼 패키지는 없습니다. 캔버스 크기는
CSS로 정하며, 차트는 그 크기와 디바이스 픽셀 비율을 따라갑니다.

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

실시간 데이터라면 차트를 다시 만들지 말고 시리즈를 ref에 보관한 채
`series.update(bar)`를 호출하세요.

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

## 서버 사이드 렌더링

차트에는 브라우저(WebGPU 또는 WebGL2)가 필요합니다. Next.js, Nuxt 같은
프레임워크에서는 클라이언트에서만 차트를 만드세요. 위와 같이 서버에서는 절대
실행되지 않는 `useEffect` / `onMounted` 안에서 만들면 됩니다.
