<!-- source: d3368c1d0b -->
# 保存与恢复

`chart.saveState()` 返回图表中由用户操作改变的部分：缩放、滚动位置、窗格高度、价格轴设置、十字光标模式和画线。`chart.loadState(state)` 把它们恢复回去。状态是与 JSON 兼容的普通数据，因此可以保存在 `localStorage`、文件或你的服务器上。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

const state: ChartState = chart.saveState();
localStorage.setItem('chart-layout', JSON.stringify(state));
```

## 状态包含什么

| 字段 | 内容 |
|---|---|
| `version` | 格式版本，当前为 `1` |
| `barSpacing` | K 线间距，单位 CSS px（缩放级别） |
| `rightOffset` | 最后一根 K 线右侧的空白，单位为 K 线根数（滚动位置） |
| `crosshairMode` | `'normal'`、`'magnet'` 或 `'hidden'` |
| `panes` | 每个窗格一项，从上到下（见下文） |
| `drawings` | 所有已完成的画线，从下到上（见下文） |

`panes` 的每一项：

| 字段 | 内容 |
|---|---|
| `stretch` | 相对于其他窗格的高度 |
| `priceScale.mode` | `'normal'`、`'logarithmic'`、`'percentage'` 或 `'indexedTo100'` |
| `priceScale.inverted` | 是否把较高的价格画在下方 |
| `priceScale.margins` | `{ top, bottom }`，占窗格高度的比例 |
| `priceScale.range` | 关闭自动缩放时（用户拖动了价格轴，或你设置了范围）为 `{ low, high }`；自动缩放时为 `null` |

`drawings` 的每一项：

| 字段 | 内容 |
|---|---|
| `pane` | 窗格的序号，从上到下（0 为主窗格），不是窗格 id |
| `visible` | 是否显示 |
| `locked` | 是否锁定、禁止编辑 |
| `data` | `{ kind, anchors, style, params }`，与 `chart.drawings.get(id)` 相同 |

锚点以时间（UTC ms）和价格存储，因此当数据变化或加载了更多历史数据时，画线仍停留在正确的 K 线上。

## 状态不包含什么

状态描述的是用户如何布置图表，而不是图表显示什么。以下内容**不会**被保存：

- 数据、系列及其选项；
- 指标、价格带、标记和价格线；
- 图层插件和已注册的画线工具；
- 主题、渲染器及其他 `FerroChart.create` 选项；
- 同步组成员关系；
- 画线 id、选中状态、已激活的工具、正在创建中的画线，以及撤销历史。

这些都由你的应用自行创建，然后在其上加载状态。请把重建它们所需的信息（品种、周期、指标列表和参数）与状态保存在一起。

## 加载状态

`loadState` 需要一个与保存时设置方式相同的图表：尤其是**窗格数量相同**。窗格按位置匹配，因此在加载之前，要按相同顺序创建会添加窗格的系列和指标（例如采用默认放置方式的 `'macd'` 或 `'rsi'`，或 `addPane()`）。同样要先加载数据；恢复的画线按时间定位。

```ts
import { FerroChart, randomWalk, type ChartState } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;
declare const saved: ChartState;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(randomWalk(1, 2_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(candles);          // same pane: no new pane
chart.addIndicator(candles, 'macd');     // adds a pane, as before saving

chart.loadState(saved);                  // two panes on both sides
```

加载会做以下事情：

- 设置 K 线间距、右侧留白和十字光标模式。
- 每个窗格获得其 stretch 和价格轴设置。保存时带有 `range` 的窗格会保持该范围并关闭自动缩放；保存时为 `range: null` 的窗格会重新开启自动缩放。
- 移除当前所有画线，并添加保存的画线。它们会获得新的 id，图表会为它们发送 `drawingRemoved` 和 `drawingCreated` 事件。
- 清空撤销历史。

### 用 JavaScript 编写的画线工具

用 `chart.drawings.register(kind, tool)` 注册的类型的画线，只能加载到注册了该类型的图表中。请在调用 `loadState` 之前注册你的工具。见[画线](drawings.md)和[插件](plugins.md)。

## 错误

当状态不匹配时，`loadState` 会抛出 `Error`，图表保持不变。不会部分应用任何内容。

| 原因 | 示例信息 |
|---|---|
| 不是对象 | `state must be an object from saveState()` |
| 缺少必需字段 | `state: missing 'barSpacing'` |
| 未知名称 | `state: unknown crosshair mode 'x'`, `state: unknown price scale mode 'x'` |
| 更新的格式版本 | `state version is newer than supported` |
| 窗格数量不同 | `state has a different number of panes` |
| K 线间距不是有限正数，或右侧留白不是有限值 | `invalid bar spacing or right offset` |
| 边距超出 0 到 1（或两者相加达到 1 及以上）；范围不是有限且递增的值，或在对数坐标下不为正 | `invalid price scale state` |
| 画线所在的窗格序号不存在 | `drawing on a missing pane` |
| 图表无法构建的画线（未知类型、锚点数量错误、样式或参数无效） | 取决于具体画线 |

不是正数的窗格 `stretch` 会被替换为 1，而不会报错。

如果保存的状态可能已经过时（例如你修改了应用的默认指标之后），请捕获错误并回退到默认布局：

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

function restore(chart: FerroChart, json: string | null): boolean {
  if (json === null) return false;
  try {
    chart.loadState(JSON.parse(json) as ChartState);
    return true;
  } catch (e) {
    console.warn('saved layout ignored:', e);
    return false;
  }
}

restore(chart, localStorage.getItem('chart-layout'));
```

## 完整示例

下面的页面在 `localStorage` 中为每个品种保存一份布局。setup 函数创建状态之外的所有内容（系列、数据、指标及其颜色），其余部分由状态恢复。每当用户修改画线以及页面被隐藏时，都会保存布局。

```ts
import { FerroChart, randomWalk, type ChartState, type Series } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

/** What the app needs to rebuild a chart; kept next to the chart state. */
interface Layout {
  symbol: string;
  indicators: { name: string; params: Record<string, number | string> }[];
  state: ChartState;
}

const key = (symbol: string) => `layout:${symbol}`;

function loadBars(symbol: string) {
  // Replace with your data source.
  const seed = [...symbol].reduce((s, c) => s + c.charCodeAt(0), 0);
  return randomWalk(seed, 5_000, Date.UTC(2024, 0, 1), 60_000);
}

async function openChart(symbol: string): Promise<FerroChart> {
  const chart = await FerroChart.create(canvas, { theme: 'dark' });

  // 1. Series and data (not part of the state).
  const candles: Series = chart.addCandlestickSeries();
  candles.setData(loadBars(symbol));
  chart.addVolumeSeries(candles);

  // 2. Indicators, in the order they were saved, so the panes match.
  const json = localStorage.getItem(key(symbol));
  const saved: Layout | null = json ? (JSON.parse(json) as Layout) : null;
  const indicators = saved?.indicators ?? [{ name: 'sma', params: { n: 50 } }, { name: 'rsi', params: { n: 14 } }];
  for (const { name, params } of indicators) {
    chart.addIndicator(candles, name, params);
  }

  // 3. The saved view, layout and drawings.
  if (saved) {
    try {
      chart.loadState(saved.state);
    } catch (e) {
      console.warn('saved layout ignored:', e);
    }
  }

  // Save after every drawing change and when the page is hidden.
  const save = () => {
    const layout: Layout = { symbol, indicators, state: chart.saveState() };
    localStorage.setItem(key(symbol), JSON.stringify(layout));
  };
  for (const event of ['drawingCreated', 'drawingModified', 'drawingRemoved'] as const) {
    chart.on(event, save);
  }
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') save();
  });
  return chart;
}

await openChart('BTCUSD');
```

这个示例中有三处细节在实践中很重要：

- 指标列表与状态保存在一起。如果用户添加或移除了指标，窗格数量会变化，旧的状态就无法再加载。
- 用户拖动画线时会发送 `drawingModified`，最多每帧一次。这么频繁地把一个小状态写入 `localStorage` 没有问题；如果保存到服务器，请对调用做防抖。
- 缩放和滚动位置的变化不会触发任何画线事件，所以示例在页面隐藏时也会保存。如果需要更早保存它们，请监听 `visibleRangeChanged`。

## 只保存画线

如果只想保留画线（例如把它们复制到窗格布局不同的另一个图表），请通过画线 API 而不是状态读取：

```ts
import { FerroChart, type DrawingData } from '@ferrochart/core';

declare const source: FerroChart;
declare const target: FerroChart;

const drawings: Required<DrawingData>[] = source.drawings
  .ids()
  .filter((id) => source.drawings.paneOf(id) === source.mainPane)
  .map((id) => source.drawings.get(id))
  .filter((d): d is Required<DrawingData> => d !== null);

for (const d of drawings) target.drawings.add(d);   // to the main pane
```

如果要让打开的多个图表之间的画线保持同步，见[同步多个图表](sync.md)。
