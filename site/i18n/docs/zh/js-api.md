<!-- source: 29952e0395 -->
# JavaScript API 概览

包名 `@ferrochart/core`。本页介绍如何加载模块，以及 API 各部分共同遵循的约定。完整的参考文档（每个类、方法和类型）由包的 TypeScript 声明生成：[JavaScript API 参考](https://ferrochart.com/docs/api)。

## 加载

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
```

`FerroChart.create` 和 `FerroPlot.create` 在首次调用时加载 WebAssembly 模块；并发调用共享同一次加载。如果需要在创建图表之前使用该模块（`randomWalk`、`new SyncGroup()`、`indicatorNames()`），或者要从其他 URL 加载 `.wasm` 文件，请自行调用默认导出 `init`：

```ts
import init, { SyncGroup } from '@ferrochart/core';

await init();                                     // next to the module (default)
await init({ module_or_path: '/assets/ferrochart_wasm_bg.wasm' });
const group = new SyncGroup();
```

`init` 可以调用任意多次；首次加载之后会立即返回。

### TypeScript

包内已附带类型声明。声明中使用了 `Symbol.dispose`，因此请把 `"ESNext.Disposable"` 加入 `compilerOptions.lib`，或者保留常见的 `skipLibCheck: true`。

### 打包工具与框架

该包是一个 ES 模块，会相对于自身 URL 获取其 `.wasm` 文件（`new URL('…_bg.wasm', import.meta.url)`），Vite、webpack 5、Rollup 和 esbuild 都能识别这种写法。只在浏览器中创建图表：在 React 中使用 effect，在 Vue 中使用 `onMounted`，并让该组件跳过服务端渲染（见 [React 与 Vue](frameworks.md)）。

## 类

| 类 | 创建方式 | 说明 |
|---|---|---|
| [`FerroChart`](https://ferrochart.com/docs/api/FerroChart) | `FerroChart.create(canvas, options?)` | `<canvas>` 上的金融图表：窗格、系列、坐标轴、十字光标、画线 |
| [`Series`](https://ferrochart.com/docs/api/Series) | `chart.addSeries`、`addCandlestickSeries`、`addVolumeSeries`、`addTransformSeries`、订单流的 `add*` | 一个数据系列 |
| [`Indicator`](https://ferrochart.com/docs/api/Indicator) | `chart.addIndicator` | 一个指标及其输出系列 |
| [`PriceLine`](https://ferrochart.com/docs/api/PriceLine) | `series.createPriceLine` | 某个价格处的水平线 |
| [`Drawings`](https://ferrochart.com/docs/api/Drawings) | `chart.drawings` | 图表的画线和画线工具 |
| [`SyncGroup`](https://ferrochart.com/docs/api/SyncGroup) | `new SyncGroup()` | 联动多个图表的十字光标、可见范围和画线 |
| [`FerroPlot`](https://ferrochart.com/docs/api/FerroPlot) | `FerroPlot.create(canvas, options?)` | 通用 x/y 绘图（数值、类别、日期） |
| [`Mark`](https://ferrochart.com/docs/api/Mark), [`Axis`](https://ferrochart.com/docs/api/Axis), [`Rule`](https://ferrochart.com/docs/api/Rule), [`Label`](https://ferrochart.com/docs/api/Label) | `FerroPlot` 的方法 | 绘图的组成部分 |

## 约定

### 时间

时间以 **UTC 毫秒**表示。列数据（`setData` 中的 `time`）接受 `number[]`、`Float64Array` 或 `BigInt64Array`；其他所有地方的单个时间（行、`update`、标记、画线锚点、`indexOfTime`）可以是 `number`、`bigint` 或 `Date`。时间必须严格递增。事件和 getter 以 `number` 返回时间（超出数据范围时为 `null`）。时间轴标签使用 UTC。

### 逻辑索引

K 线通过**逻辑索引**定位：初始数据的第一根 K 线为 0，下一根为 1，依此类推。之后前插的 K 线获得负索引，所以索引一经分配就不会改变。可见范围、十字光标以及转换方法（`coordinateToLogical`、`timeAtIndex`、`indexOfTime`）都使用逻辑索引。带小数的值表示 K 线中心之间的位置。

### 坐标

位置以距 canvas 左上角的 **CSS 像素**表示，与 DOM 事件一致。canvas 的尺寸由 CSS 决定；图表会把其后备存储设为 CSS 尺寸 × `devicePixelRatio`，并自动跟随尺寸变化。

### 颜色

`Color` 是 CSS 颜色字符串：`#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()`、`rgba()` 或 `'transparent'`。在选项中，`null` 表示使用主题的默认值。

### 选项对象

选项是普通对象。`applyOptions` 及类似方法只修改你传入的键；未传入的键保持原值。`series.options()` 之类的 getter 返回所有键（`Required<…>`）。

### 错误

方法会抛出 `Error`，其信息会指明问题所在（未知的指标名、无效的参数、长度不一致的列、时间未递增的 K 线、未知的 id）。图表保持不变。当请求的 GPU API 不可用时，`FerroChart.create` 会 reject：没有 Canvas 2D 回退；`renderer: 'auto'` 先尝试 WebGPU，再尝试 WebGL2。

### 事件

`chart.on(name, callback)` 订阅，`chart.off(name, callback)` 取消订阅。回调参数的类型由事件名推断。事件在产生它们的那一帧之后投递，而不是在输入处理函数内部。回调抛出的异常会报告到控制台（通过 `reportError`），其他回调照常执行。见[十字光标与事件](crosshair-events.md)。

### 生命周期与内存

图表会监听其 canvas 和 window，并自行重绘。图表在页面上时请保留对它的引用，移除时调用 `chart.destroy()`：这会移除监听器并释放 GPU 内存。未调用 `destroy` 就丢弃的图表可能会被垃圾回收并停止响应。

API 的每个对象都持有 WebAssembly 内存。图表返回的对象（`Series`、`Indicator`、`PriceLine`）随图表一起释放；`free()` 或 `using` 声明可以提前释放某一个对象。移除系列（`remove()`）是另一回事：它把系列从图表上拿掉。

### canvas 上处理的输入

指针（鼠标、触控笔、触摸）、滚轮、键盘（canvas 会获得 `tabindex="0"`）、双击、触控板双指缩放（Chrome 和 Firefox 中为 Ctrl+滚轮，Safari 中为 gesture 事件），以及由图表自行识别的双指触摸。canvas 的样式会被设置为 `touch-action: none`；使用 `touchScroll: 'page'` 时则为 `pan-y`（见[触屏](web.md#touch-screens)）。各手势的作用见[手势与键盘](interaction.md)。

## 测试数据

`randomWalk(seed, n, startMs, intervalMs)` 返回确定性的 OHLC 列，与 Rust 生成器 `ferrochart_core::testing::random_walk` 完全一致。示例和演示都使用它。

```ts
import init, { randomWalk } from '@ferrochart/core';

await init();
const bars = randomWalk(42, 10_000, Date.UTC(2024, 0, 1), 60_000);
```
