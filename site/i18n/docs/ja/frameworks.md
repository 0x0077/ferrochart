<!-- source: 3769d1d4a6 -->
# ReactとVue

`@ferrochart/core`はどのフレームワークでも動作します。canvasがマウントされたときにチャートを作成し、取り除かれるときに破棄してください。インストールが必要なラッパーパッケージはありません。canvasのサイズはCSSで指定します。チャートはそのサイズとデバイスピクセル比に追従します。

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

リアルタイムデータの場合は、シリーズをrefに保持し、チャートを作り直す代わりに`series.update(bar)`を呼んでください。

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

## サーバーサイドレンダリング

チャートにはブラウザー（WebGPUまたはWebGL2）が必要です。Next.js、Nuxtなどのフレームワークでは、クライアント側でのみ作成してください。上の例のように`useEffect` / `onMounted`の中で作成すれば、サーバーで実行されることはありません。
