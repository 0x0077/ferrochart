<!-- source: 325f16fb0e -->
# 标记与价格线

标记是附着在 K 线上的小图形，可带文字，例如买卖信号或新闻。标记也可以显示一张图片（钱包或账户头像）或 `DB` 这样的简短徽标，K 线上的标记过多时还可以折叠成一个“+k”徽标。价格线是位于固定价格处的水平线，例如委托单、止损或目标价。两者都属于某个系列。每个系列还有一个最新值标签和一条最新值线，通过系列选项开启或关闭。

## 标记

### 设置标记

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers` 会替换该系列的所有标记。要添加一个标记，先读取当前列表，追加后再重新设置；要清空标记，传入空数组：

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### 标记字段

| 字段 | 类型 | 默认值 | 含义 |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | 必填 | UTC 毫秒（整数）或 `Date`；`markers()` 返回数字 |
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | 位于 K 线最高价之上或最低价之下。对于单值系列，位于数值之上或之下。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 实心图标 |
| `color` | `Color \| null` | `null` | 图标和文字颜色；`null` 表示使用系列的主色（K 线和柱使用上涨颜色） |
| `text` | `string` | `''` | 图标旁的文字，位于远离 K 线的一侧；为空则不显示 |
| `size` | `number \| null` | `null` | 直径（方形为边长），4–64 CSS px；`null` 表示 10 |
| `icon` | `string \| null` | `null` | 用 `chart.registerIcon` 添加的图片名称，绘制在圆形或方形内 |
| `label` | `string` | `''` | 绘制在圆形或方形内的文字，最多 8 个字符，例如 `'DB'`；不能与 `icon` 同用 |
| `border` | `Color \| null` | `null` | 沿图形边缘内侧绘制的圆环；`null` 表示不绘制 |
| `borderWidth` | `number` | `2` | 圆环宽度，CSS px |

未知的 `position` 或 `shape`、缺失或非整数的 `time`、无效的颜色、超出 4–64 的 `size`、超过 8 个字符的 `label`，或者在箭头上使用 `icon` 或 `label`、在同一个标记上同时使用两者，都会抛出异常，标记保持不变。

### 读取标记

`series.markers()` 返回按时间排序的标记，所有字段都已填充（使用默认值的地方 `color` 为 `null`）。排序是稳定的：时间相同的标记保持你给出的顺序。

标记在此列表中的索引就是 `click` 事件报告的 `target.index`（见[十字光标与事件](crosshair-events.md)）。由于列表经过排序，该索引可能与你传给 `setMarkers` 的数组中的位置不同。

### 放置与吸附

- 标记绘制在时间等于 `time` 的 K 线上。落在两根 K 线之间的时间会吸附到**后一根** K 线；晚于最后一根 K 线的时间吸附到最后一根。早于第一根 K 线的时间吸附到第一根。
- 如果系列在该 K 线处没有值（缺口），则不绘制该标记。
- 标记跟随系列的 `shift` 选项。
- 图标直径为 `size` CSS px（默认 10），与 K 线相距 4 CSS px。
- 同一根 K 线同一侧的多个标记按 `markers()` 的顺序向远离 K 线的方向堆叠。带文字的标记占用图标加一行文字的空间。
- 文字以 K 线为中心水平居中，`aboveBar` 时在图标上方，`belowBar` 时在图标下方，使用标记的颜色。
- 标记会被裁剪到绘图区内。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

### 图片、徽标与边框

先用一个名称注册图片，之后可以在任意数量的标记中使用这个名称。来源可以是 URL（会被抓取；来自其他源的图片需要 CORS 响应头）、`HTMLImageElement`、`ImageBitmap`、`ImageData` 或 canvas。图表会保留一份缩放到 64 × 64 像素以内的副本：

```ts
await chart.registerIcon('whale', '/avatars/whale.png');
await chart.registerIcon('dev', document.querySelector('img#dev') as HTMLImageElement);

candles.setMarkers([
  { time: t1, icon: 'whale', size: 24, border: '#2fbf71' },             // a buy
  { time: t2, icon: 'whale', size: 24, border: '#ef5350' },             // a sell
  { time: t2, position: 'belowBar', label: 'DB', color: '#2962ff', size: 20 },
  { time: t3, position: 'belowBar', label: '狙', color: '#ff9800', size: 20 },
]);
```

- 图片填满圆形（裁剪为圆形）或方形，绘制在标记颜色之上，因此透明图片会透出背后的颜色。箭头不能带图片或文字徽标。
- `registerIcon` 返回一个 promise；请等它完成后再使用该名称。用未注册的名称调用 `setMarkers` 会抛出异常。图片无法加载或读取时（抓取失败、来自其他源的图片没有 CORS 响应头），promise 会被拒绝：这时请为该标记改用 `label` 或普通图形。
- 再次注册同一名称会替换所有使用它的标记中的图片。`chart.unregisterIcon(name)` 会移除它（返回之前是否存在）；之后使用它的标记只绘制图形，`markers()` 报告其 `icon` 为 `null`。一个图表最多保存 1024 张图片。
- `label` 居中绘制，颜色与标记颜色形成对比，字号自动适配：拉丁字母约为直径的 0.45，中文、日文或韩文字符约为 0.6，文字较宽时会更小。
- `border` 圆环绘制在边缘内侧，因此不会改变标记的大小，例如买入用绿色圆环，卖出用红色圆环。

### 折叠拥挤的 K 线

当很多标记落在同一根 K 线上时，例如十几个交易者在同一分钟买入，可以在系列上设置 `markerStackLimit`。K 线每一侧最多使用这么多个位置；最后一个位置显示“+k”，代表未绘制的 k 个标记：

```ts
candles.applyOptions({ markerStackLimit: 4 });   // 3 markers, then "+k"
candles.applyOptions({ markerStackLimit: null }); // no limit (the default)
```

徽标是一个使用系列主色的圆形，大小与它所替代的标记相同。点击它会发送一个 `click` 事件，其 target 为 `{ type: 'markerOverflow', series, first, count }`：未绘制的标记是 `series.markers().slice(first, first + count)`，例如可以在弹出框中列出它们。

### 悬停标记

指针移到标记或“+k”徽标上时会触发 `markerHover` 事件，离开时以 `null` 触发。事件携带标记的索引、以 CSS px 表示的中心和直径，这正是提示框所需要的：

```ts
chart.on('markerHover', (e) => {
  if (!e) return hideTooltip();
  const list = e.series.markers();
  if (e.overflow > 0) {
    showTooltip(e.x, e.y - e.size / 2, `${e.overflow} more`, list.slice(e.index, e.index + e.overflow));
  } else {
    showTooltip(e.x, e.y - e.size / 2, list[e.index].text ?? '');
  }
});
```

`index` 是在 `series.markers()` 中的位置；对于徽标，它是第一个未绘制的标记，`overflow` 为 k（标记为 0）。悬停的标记在绘制帧时确定，因此该事件每帧最多发送一次。入场动画播放期间，不会有标记被视为悬停。见[十字光标与事件](crosshair-events.md)。

### 中文、日文、韩文与 emoji

标记文字和徽标可以使用任何文字。内置字体只包含拉丁字母、数字和常用符号；其他字符来自：

- **Web**：浏览器自身的字体（系统的 CJK 字体和彩色 emoji），无需任何配置，也不会增加包体积。
- **桌面、Python 和 C**：你添加的字体，按顺序查找：winit 中用 `WindowOptions { fonts, .. }`，Python 中用 `fc.Chart(fonts=[...])`，C 中用 `ferrochart_add_font`。不会扫描系统字体。通常存在的字体：`/System/Library/Fonts/Hiragino Sans GB.ttc`（macOS）、`C:/Windows/Fonts/msyh.ttc`（Windows）、`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`（Linux，需安装 Noto CJK 包）；也可以随应用附带 Noto Sans SC/JP/KR。

所有字体都没有的字符会画成 `□`。原生端不支持彩色 emoji 字体：轮廓 emoji 字体（Noto Emoji）会以文字颜色绘制。文字按字素簇排版：在网页端，带肤色的 emoji、家庭等 ZWJ 序列、国旗以及带组合音标的字母，都由浏览器作为一个字形测量和绘制。原生端仍逐字符绘制这些序列（字体不做整形）。不支持字形整形（shaping）和从右到左排版，所以阿拉伯文、希伯来文无法正确显示。

### Python、C 与 Rust

各语言绑定中的字段都相同：

```python
red = chart.add_icon(avatar)          # a (height, width, 4) or (height, width, 3) uint8 array
candles.set_markers([
    {"time": t, "icon": red, "size": 24, "border": "#2fbf71"},
    {"time": t, "position": "below_bar", "label": "DB", "color": "#2962ff"},
])
candles.apply_options(marker_stack_limit=4)
chart.on("marker_hover", lambda e: print(e))   # None when the pointer leaves
```

在 Python 中，`add_icon` 返回一个整数 id，`remove_icon(id)` 移除该图片。在 C 中，`ferrochart_add_icon` 和 `ferrochart_set_markers` 接受相同的字段（见 [C、C++ 与 C#](native.md)）。在 Rust 中，`Chart::add_icon` 接受一个 `IconImage`（非预乘 RGBA，最大 256 × 256），返回用于 `Marker::icon` 的 `IconId`；`ChartEvent::MarkerHover` 和 `ClickTarget::MarkerOverflow` 分别是对应的事件和点击目标。

### 哪些系列可以有标记

除成交量系列外的所有系列类型。在成交量系列上调用 `setMarkers` 会抛出异常。标记也可以放在指标输出上（`Indicator` 的 `outputs()` 就是普通系列）：

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### 示例：根据信号生成标记

```ts
import type { Marker, OhlcData } from '@ferrochart/core';

// Mark crossings of the close over a 20-bar SMA.
function crossMarkers(bars: OhlcData, sma: number[]): Marker[] {
  const out: Marker[] = [];
  const close = bars.close;
  for (let i = 1; i < close.length; i++) {
    const was = close[i - 1] - sma[i - 1];
    const now = close[i] - sma[i];
    if (Number.isNaN(was) || Number.isNaN(now)) continue;
    if (was <= 0 && now > 0) {
      out.push({ time: bars.time[i], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' });
    } else if (was >= 0 && now < 0) {
      out.push({ time: bars.time[i], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' });
    }
  }
  return out;
}

candles.setMarkers(crossMarkers(bars, smaValues));
```

## 价格线

### 创建价格线

```ts
const entry = candles.createPriceLine({
  price: 101.25,
  color: '#2962ff',
  lineWidth: 1,
  lineStyle: 'solid',
  title: 'Entry',
});
const stop = candles.createPriceLine({ price: 98.5, color: '#ef5350', title: 'Stop' });
```

`createPriceLine` 返回一个 `PriceLine`。价格以系列的单位表示，因此在百分比或指数化价格轴上，价格线会随系列移动（见[价格轴](price-scale.md)）。

### 选项

| 选项 | 类型 | 默认值 | 含义 |
|---|---|---|---|
| `price` | `number` | `0` | 价格，以系列的单位表示；必须是有限值 |
| `color` | `Color \| null` | `null` | 线条、标题和坐标轴标签的颜色；`null` 表示使用系列的主色 |
| `lineWidth` | `number` | `1` | CSS px，大于 0 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | 虚线样式 |
| `axisLabelVisible` | `boolean` | `true` | 在价格轴上显示价格标签 |
| `title` | `string` | `''` | 在绘图区右边缘、线条上方显示的文字；为空则不显示 |

坐标轴标签使用系列的价格格式（其 `pricePrecision`）。价格轴上的标签在会重叠时会被错开。位于可见价格范围之外的价格线不会绘制。

### 修改与移除

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

`remove()` 之后，在该对象上调用 `options`、`applyOptions` 或 `remove` 都会抛出异常。移除系列会一并移除其价格线。

对于成交量系列、非有限的价格、不是对象的选项参数或无效的选项值，`createPriceLine` 会抛出异常。

### 示例：可拖动的委托线

价格线本身不处理输入。可以把它与图表的事件和坐标转换结合起来，或者使用用户可以拖动的 `horizontal_line` 画线（见[画线工具](drawings.md)）：

```ts
const id = chart.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 1), price: 101.25 }],
  style: { line: '#2962ff' },
});

chart.on('drawingModified', (e: ChartEventMap['drawingModified']) => {
  if (e.id !== id) return;
  const price = chart.drawings.get(id)?.anchors[0].price;
  if (price !== undefined) console.log('order moved to', price);
});
```

## 最新值标签与最新值线

每个系列都会在价格轴上以标签形式、并在绘图区中以一条水平点线显示其最新值。当最后一根 K 线收盘价不低于开盘价时（对于基线系列：不低于基准值），两者使用上涨颜色，否则使用下跌颜色；折线、面积、柱状图和散点系列使用其主色。

| 系列选项 | 默认值 | 含义 |
|---|---|---|
| `lastValueVisible` | `true` | 在价格轴上显示最新值标签 |
| `priceLineVisible` | `true` | 在最新值处显示点线 |
| `pricePrecision` | `null` | 标签的小数位数；`null` 表示自动 |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

成交量系列两者都不绘制。指标输出创建时为 `priceLineVisible: false`；其最新值标签会显示。可以逐个输出关闭：

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

系列选项见[系列](series.md)。
