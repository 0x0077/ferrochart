<!-- source: 7f51672705 -->
# 主题与样式

本章介绍图表的外观以及它如何融入页面：主题、颜色、坐标轴与留白、交互、十字光标、大量小图、导出图片、署名标志、入场动画、GPU 渲染器、尺寸调整和清理。除非特别说明，这里的内容同样适用于 `FerroPlot`。

## 主题

主题在创建图表时选择：

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' }); // 'light' (default) or 'dark'
```

主题决定背景、网格、文字、坐标轴和十字光标的颜色，K 线默认的涨跌颜色，以及系列、指标和画线的默认颜色。

主题也可以是一个对象：在 `base` 主题（默认 `'light'`）之上覆盖的 CSS 颜色，未给出的键取自 `base`。用 `applyOptions` 可以修改已有图表的主题，视图和数据保持不变。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  theme: { base: 'dark', background: '#0b0d12', up: '#30a0f0', down: '#f09030', fontSize: 13 },
});
chart.applyOptions({ theme: 'light' });
```

可用的键有 `background`、`text`、`grid`、`axisLine`、`up`、`down`、`upWick`、`downWick`、`volumeUp`、`volumeDown`、`crosshair`、`crosshairLabelBg`、`crosshairLabelText`、`line`、`fontSize` 和 `axisPadding`；未知的键会报错。`chart.options().theme` 读回当前主题。

任何未设置或设为 `null` 的颜色都跟随主题。只在需要的地方显式设置颜色，其余部分就能在两种主题下保持一致。

## 系列颜色

系列的颜色来自 `SeriesOptions`，可在创建时设置，也可以之后用 `series.applyOptions` 设置：

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 2, lineStyle: 'dashed' });
const area = chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});

line.applyOptions({ color: null }); // back to the theme's color
```

| 选项 | 用途 |
|---|---|
| `color` | 折线、面积和基线（基准值以上）系列的线条颜色；上涨的 K 线和柱；柱状图的柱 |
| `downColor` | 下跌的 K 线和柱；基线系列在基准值以下的线条 |
| `topFill` | 面积图顶部的填充；基线以上的填充 |
| `bottomFill` | 面积图底部的填充；基线以下的填充 |
| `lineWidth` | 线宽，单位 CSS px |
| `lineStyle` | `'solid'`、`'dashed'` 或 `'dotted'` |

面积填充是从 `topFill` 到 `bottomFill` 的渐变。柱状图系列还接受逐柱颜色（`setData` 的 `color` 列）。所有系列选项以及每种系列绘制的内容，见[系列](series.md)。

其他对象有各自的颜色选项：价格线（`createPriceLine({ color })`）、标记（每个标记的 `color`）、两个系列之间的价格带（`addBand(upper, lower, { fill, fillCrossed })`）以及画线（`style: { line, fill, text }`）。见[系列](series.md)、[标记](markers.md)、[指标](indicators.md)和[画线](drawings.md)。

## 颜色

颜色是以下形式之一的 CSS 颜色字符串（类型 `Color`）：

| 形式 | 示例 |
|---|---|
| `#rgb`, `#rgba` | `'#f80'`, `'#f808'` |
| `#rrggbb`, `#rrggbbaa` | `'#ff8800'`, `'#ff880080'` |
| `rgb(r, g, b)` | `'rgb(255, 136, 0)'` |
| `rgba(r, g, b, a)` | `'rgba(255, 136, 0, 0.5)'`，alpha 取 0 到 1 |
| `transparent` | `'transparent'` |

各通道取值 0 到 255；用逗号、空格或 `/` 分隔。**不**接受命名颜色（`'red'`）、`hsl()` 和百分比：调用会抛出异常，例如 `'color': invalid color 'red'`。

从 `options()` 读回的颜色写成 `#rrggbb`，不透明度不为 1 时写成 `rgba(r, g, b, a)`；跟随主题的颜色读回为 `null`。

逐点颜色列（柱状图数据、绘图标记）也接受由 `0xRRGGBBAA` 值组成的 `Uint32Array`，这样在大数据量时可以避免创建字符串。

## 坐标轴、网格与留白

App 和表格里的图表常常完全没有坐标轴。`priceAxis` 和 `timeAxis` 显示或隐藏坐标轴，`grid` 控制网格线（`true`、`false` 或 `{ horizontal, vertical }`），`padding` 在绘图区周围留出空白（CSS px，一个数字或 `{ top, right, bottom, left }`）。隐藏的坐标轴把空间让给绘图区。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  priceAxis: false,
  grid: { vertical: false },
  padding: { top: 72, bottom: 40 },
});
chart.applyOptions({ timeAxis: false });
```

这些选项以及下两节的选项都可以用在 `create` 和 `applyOptions` 中，`chart.options()` 会读回它们。在 Python 中它们是 snake_case 的关键字参数（`Chart(preset="sparkline")`、`chart.apply_options(price_axis=False)`）；在 C 中是 `FerroChartAppearance`、`FerroChartInteraction`、`FerroChartCrosshairStyle` 和 `FerroChartTheme` 结构体。

## 交互

`interaction` 开关平移、缩放和十字光标：`{ pan, zoom, crosshair }`，或用 `false` 全部关闭。被关闭的输入图表不会使用，所以在触屏上页面照常滚动和缩放。点击仍会上报。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { interaction: { pan: false, zoom: false } });
```

## 十字光标样式

`crosshairStyle` 决定十字光标画哪些部分：`vertical` 和 `horizontal` 两条线、坐标轴上的 `labels`，以及第一个系列取值处的 `dot`（`{ radius, color }`，`null` 表示不画；颜色默认取该系列的颜色）。一条竖线加一个圆点是行情卡片常见的十字光标。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, {
  crosshairStyle: { horizontal: false, labels: false, dot: { radius: 5 } },
});
```

## 大量小图

`preset: 'sparkline'` 是放在表格单元格里的图表：没有坐标轴、没有网格、留白 2、不能交互，十字光标是一条竖线加一个圆点。与它一起给出的选项会叠加在上面。

浏览器每个页面只保留大约 16 个 WebGL 上下文，而每个图表都有自己的 GPU 设备。页面上有很多小图时，从池中创建它们：它们共用一个设备（在 WebGL2 上共用一个上下文）。视口之外的图表不绘制，滚动回来时再画。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvases: HTMLCanvasElement[];

const pool = await FerroChart.createPool();
const charts = await Promise.all(canvases.map((c) => pool.create(c, { preset: 'sparkline' })));
// …
pool.destroy(); // destroys every chart of the pool
```

在 WebGL2 上，池中的图表用 2D 上下文绘制自己的画布，因此画布不能已有其他上下文。

## 导出图片

`chart.toBlob(type?, quality?)` 绘制一帧，返回 PNG（默认）、WebP 或 JPEG 的 `Blob`；`chart.toImageBitmap()` 返回 `ImageBitmap`。图片的像素尺寸与画布相同，不含署名标志（它不在画布里）。`FerroPlot` 有相同的方法。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const png = await chart.toBlob();
const jpeg = await chart.toBlob('image/jpeg', 0.9);
```

## 署名标志

默认情况下，最下方窗格的左下角、紧贴时间轴上方会显示一个小小的 FerroChart 标志。它链接到 ferrochart.com，指针悬停时会显示名称。

该标志只是一种致谢，而非许可证要求：你可以关闭它。未设置 `attributionLogo` 时，只有画布不小于 320 × 160 CSS px 时才显示标志，画布尺寸跨过这个界限时随之出现或消失；设为 `true` 或 `false`（或调用 `setAttributionLogo`）后固定不变。

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { attributionLogo: false });

chart.setAttributionLogo(true);  // show it
chart.setAttributionLogo(false); // remove it
```

它的放置方式：

- 它是紧跟在 canvas 之后插入的一个 `<a>` 元素，以绝对定位放在 canvas 旁边；它不会绘制到 canvas 中，因此不会出现在截图或引擎渲染的图像里。
- 布局变化时会更新它的位置。它相对于 canvas 的 offset parent 定位，与 canvas 本身一致。
- 如果在要添加标志时 canvas 没有父元素，则不会添加。

`FerroPlot` 有相同的选项和方法；它的标志位于绘图区左下角。通过 `attachSidePlot` 附加到图表上的绘图会失去自己的标志，因为图表的标志同时代表两者（见[绘图](plots.md)）。

## 入场动画

`playIntro` 先显示空图表（网格和坐标轴），然后从左到右绘制数据。适合在图表首次出现时使用，例如在落地页上或用户打开某个品种时。

```ts
import { FerroChart, randomWalk } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const chart = await FerroChart.create(canvas, { theme: 'dark' });
chart.addCandlestickSeries().setData(randomWalk(1, 300, Date.UTC(2024, 0, 1), 3_600_000));

await chart.playIntro({ duration: 1200, delay: 200, pen: '#4cc9f0' });
// The animation has ended (or was ended early).
```

`IntroOptions`：

| 选项 | 含义 | 默认值 |
|---|---|---|
| `duration` | 从左到右绘制数据所用的时间，ms | `900` |
| `delay` | 先显示空图表的时间，ms | `0` |
| `pen` | 绘制前沿处发光线条的颜色；`null` 表示不显示 | `null` |

行为：

- 图表会立即（在下一帧之前）以空白状态显示，所以请在 `setData` 之后紧接着调用 `playIntro`，以免完整图表一闪而过。
- 价格线、数值标签和十字光标在动画结束时出现。
- 在 canvas 上按下、滚动滚轮、双指缩放、双击或按键会提前结束动画。在图表上移动指针不会。
- `chart.finishIntro()` 会立即结束动画。
- 再次调用 `playIntro` 会先结束正在运行的动画。
- 返回的 promise 在动画结束时 resolve，不论以何种方式结束（图表被销毁时也会）。它从不 reject；无效的选项（负数或非有限的 `duration` 或 `delay`、无效的 `pen` 颜色）会在调用时直接抛出异常。

对于 `FerroPlot`，`playIntro` 从左到右绘制标记；参考线标签和悬停高亮在结束时出现。

## 渲染器

FerroChart 使用 GPU 绘制：浏览器支持时使用 WebGPU，否则使用 WebGL2。没有 Canvas 2D 回退。

| `renderer` 选项 | 行为 |
|---|---|
| `'auto'`（默认） | 先 WebGPU，再 WebGL2 |
| `'webgpu'` | 仅 WebGPU |
| `'webgl2'` | 仅 WebGL2 |

当请求的 API 都不可用（`'WebGPU is not
available'`、`'WebGL2 is not available'`，或两者以 `;` 连接），或选项值未知时，`create` 会 reject。`renderer` 属性表明正在使用哪种 API：

```ts
import { FerroChart } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

try {
  const chart = await FerroChart.create(canvas);
  console.log(chart.renderer); // 'webgpu' or 'webgl2'
} catch (e) {
  canvas.replaceWith(Object.assign(document.createElement('p'), {
    textContent: 'This chart needs WebGPU or WebGL2.',
  }));
}
```

在支持 WebGPU 的浏览器上强制使用 `'webgl2'`，是测试回退路径的简单方法。

## 设置 canvas 尺寸

canvas 的尺寸用 CSS 设置。FerroChart 会自行把 canvas 的后备存储（`canvas.width` 和 `canvas.height`）设为 CSS 尺寸乘以 `devicePixelRatio`，使图表在高分辨率屏幕上保持清晰。不要自己设置 canvas 的 `width` 和 `height`。

```html
<div style="height: 60vh">
  <canvas id="chart" style="display: block; width: 100%; height: 100%"></canvas>
</div>
```

- `ResizeObserver` 会监视 canvas 的内容框。当其 CSS 尺寸变化时，后备存储和布局随之调整，图表重绘。你无需调用任何 resize 方法。
- 当 `devicePixelRatio` 变化时（浏览器缩放、把窗口移到另一块屏幕），后备存储也会调整尺寸。
- 后备存储至少为 1 × 1 px。如果创建图表时它位于隐藏元素中，一旦该元素获得尺寸就会开始绘制。
- `display: block` 可以避免行内 canvas 下方出现几像素的空隙。
- 在 flex 或 grid 布局中，给 canvas 设置 `min-height: 0`（或 `min-width: 0`），以便容器可以把它缩小。

创建图表时，canvas 还会获得 `tabindex="0"`（除非它已有 `tabindex`），以便接收按键；获得 `touch-action: none`，使触摸手势平移和缩放的是图表而不是页面（使用 `touchScroll: 'page'` 时为 `pan-y`，将竖向滑动交给页面，见[触屏](web.md#touch-screens)）；以及 `outline: none`。指针位于图表上时会设置光标样式。

## 清理

```ts
import { FerroChart, FerroPlot } from '@ferrochart/core';

declare const chart: FerroChart;
declare const plot: FerroPlot;

chart.destroy();
plot.destroy(); // FerroPlot's equivalent
```

`destroy()`：

- 移除 canvas 上的 DOM 事件监听器以及所有用 `on` 添加的回调；
- 离开图表的同步组；
- 移除署名标志；
- resolve 尚未完成的 `playIntro` promise；
- 释放 GPU 资源。

canvas 元素仍留在页面中；如果不再需要，请自行移除。`destroy()` 之后，图表对象不可再使用，对其 `Series`、`Indicator` 等句柄的调用会抛出异常（`the FerroChart was
destroyed`）。不再显示的图表请销毁：每个图表都占用 GPU 内存，而且浏览器会限制一个页面可拥有的 WebGL 上下文数量。

在 React 或 Vue 中，在组件挂载时创建图表，在卸载时销毁它；见 [React 与 Vue](frameworks.md)。
