<!-- source: 2b4219641f -->
# lightweight-charts からの移行

lightweight-charts向けに書かれたコードをFerroChartで使う方法は2つあります。

1. **互換エントリー** `@ferrochart/core/lightweight-charts`。lightweight-charts v4およびv5のAPIのうちよく使われる部分をFerroChart上に実装したもので、既存のチャートはimportを変更するだけで動くことがよくあります。既存のページでFerroChartを試したり、段階的に移行したりするときに使います。
2. **ネイティブAPI** `@ferrochart/core`。このガイドの他の章で説明しているAPIで、すべての機能を利用できます。大規模データ向けのカラム形式データ、インジケーター、描画、同期、オーダーフロー、プロットなどです。

## 互換エントリー

```ts
// Before:
// import { createChart, CandlestickSeries, LineSeries } from 'lightweight-charts';
import { createChart, CandlestickSeries, LineSeries, type UTCTimestamp } from '@ferrochart/core/lightweight-charts';

const container = document.getElementById('chart') as HTMLElement;
const chart = createChart(container, { layout: { background: { color: '#131722' } } });

const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData([
  { time: 1700000000 as UTCTimestamp, open: 10, high: 12, low: 9, close: 11 },
  { time: 1700000060 as UTCTimestamp, open: 11, high: 13, low: 10, close: 12 },
]);

const sma = chart.addSeries(LineSeries, { color: '#2962ff', lineWidth: 2 });
sma.setData([{ time: 1700000060 as UTCTimestamp, value: 11.5 }]);

chart.timeScale().fitContent();
```

### 対応している機能

| 領域 | 対応内容 |
|---|---|
| 作成 | `createChart(container, options)`、要素またはそのidを指定 |
| シリーズ | `CandlestickSeries`、`BarSeries`、`LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries`を使った`addSeries(definition, options?, paneIndex?)`、v4のメソッド`addCandlestickSeries`、`addBarSeries`、`addLineSeries`、`addAreaSeries`、`addBaselineSeries`、`addHistogramSeries`、`removeSeries` |
| データ | `setData`、`update`、`dataByIndex`。時刻は`UTCTimestamp`の秒、`'YYYY-MM-DD'`文字列、`BusinessDay`オブジェクトのいずれか。ヒストグラムデータの足ごとの`color` |
| シリーズAPI | `applyOptions`、`options`、`seriesType`、`priceScale`、`createPriceLine`、`removePriceLine` |
| マーカー | v4の`series.setMarkers` / `series.markers`、v5の`createSeriesMarkers(series, markers)`（`setMarkers`、`markers`、`detach`） |
| 時間軸 | `fitContent`、`scrollToRealTime`、`scrollToPosition`、`scrollPosition`、`getVisibleLogicalRange`、`setVisibleLogicalRange`、`getVisibleRange`、`setVisibleRange`、`logicalToCoordinate`、`coordinateToLogical`、`timeToCoordinate`、`coordinateToTime`、表示中の論理範囲と時間範囲の購読、`applyOptions` / `options`（`barSpacing`、`rightOffset`）、`width` |
| 価格スケール | `chart.priceScale('right', paneIndex)`と`series.priceScale()`：`scaleMargins`、`mode`、`invertScale`、`autoScale`を指定した`applyOptions` / `options` |
| クロスヘア | `subscribeCrosshairMove` / `unsubscribeCrosshairMove`（`time`、`logical`、`point`、`paneIndex`、`seriesData`付き） |
| チャート | `applyOptions` / `options`（`width`、`height`、`layout`、`grid`、`crosshair`、`rightPriceScale.visible`、`handleScroll`、`handleScale`、`timeScale.barSpacing`、`timeScale.rightOffset`、`timeScale.visible`）、`resize`、`remove`、`timeScale`、`priceScale` |
| 定数 | `ColorType`、`CrosshairMode`、`LineStyle`、`LineType`、`PriceScaleMode`、`isUTCTimestamp`、`isBusinessDay` |

TypeScriptの型宣言（`ISeriesApi`、`IChartApi`、`Time`、`UTCTimestamp`、`SeriesMarker`など）も含まれています。

### 非同期の起動

GPUのセットアップは非同期で行われますが、`createChart`はlightweight-chartsと同様にすぐに戻ります。チャートの準備が整う前に行われた呼び出しはキューに入れられ、準備ができた時点で順番に実行されます。これが問題になる場面のために、次の2つが追加されています。

| メンバー | 説明 |
|---|---|
| `chart.whenReady(): Promise<void>` | GPUのセットアップが完了し、キューに入った呼び出しが実行された時点で解決されます |
| `chart.ferrochart: FerroChart \| null` | 内部のネイティブチャート。準備ができるまでは`null` |

`getVisibleLogicalRange()`、`getVisibleRange()`、`coordinateToTime()`、`dataByIndex()`などのゲッターは、チャートの準備ができるまで`null`を返します（`priceScale().options()`は`{}`を返します）。これらを読み取る前に`whenReady()`をawaitしてください。

```ts
import { createChart } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
// ... add series and data ...
await chart.whenReady();
console.log(chart.timeScale().getVisibleLogicalRange());
```

キューに入った呼び出しで発生したエラーは、throwされるのではなく`console.error`でコンソールに出力されます。エラーの原因となった呼び出しはすでに戻っているためです。チャートの準備ができた後は、通常どおりエラーがthrowされます。

### 相違点

**描画とサイズ**

- チャートは、`createChart`がコンテナに追加する`<canvas>`に描画されます。`width`/`height`オプションを指定しない場合、canvasはコンテナ全体（`100%` × `100%`）を占め、そのサイズに追従するので、コンテナに高さを指定してください。`autoSize`は不要で、無視されます。
- `width`、`height`、`resize(width, height)`はcanvasのCSSサイズを設定します。
- `remove()`はチャートを破棄し、そのcanvasを削除します。
- WebGPUまたはWebGL2が必要です。[テーマとスタイル](styling.md#renderer)を参照してください。

**テーマと色**

- `layout.background.color`は背景色を設定し、基になるテーマを決めます。`#rrggbb`（または`#rrggbbaa`）で書かれた暗い色ならダークテーマ、それ以外ならライトテーマです。`layout.textColor`と`layout.fontSize`は文字を設定します。グラデーション背景は`topColor`で描かれ、`fontFamily`は無視されます。いずれも`applyOptions`で変更できます。
- `grid.vertLines` / `grid.horzLines`：`visible`でそれぞれ縦線・横線を隠せます。グリッドの色は1つだけです（両方指定した場合は横線の色）。
- `rightPriceScale.visible`（または`priceScale('right').applyOptions({ visible })`）と`timeScale.visible`で軸を隠せます。価格軸はすべてのペインでまとめて隠れます。
- `handleScroll`と`handleScale`：`false`、またはサブオプションがすべて`false`のオブジェクトでパンやズームをオフにします。サブオプションが1つでも`true`ならオンのままです（FerroChartのスイッチはそれぞれ1つです）。
- `crosshair.mode`：`Normal`、`Magnet`、`Hidden`が動作します。`MagnetOHLC`は`Magnet`として扱われます。`vertLine.visible`と`horzLine.visible`でそれぞれの線を隠せます。ラベルは両方の線の`labelVisible`が`false`でない限り表示されます。
- `watermark`、`localization`、`kineticScroll`など、対応するものがないチャートオプションは受け付けられますが無視されます。
- 色はFerroChartが受け付ける形式で指定する必要があります：`#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()`、`rgba()`、`transparent`。`'red'`のような色名はエラーになります。[色](styling.md#colors)を参照してください。

**シリーズのオプション。** 使用されるのは次のオプションだけです。それ以外は受け付けられますが無視されます。

| シリーズ | 使用されるオプション |
|---|---|
| ローソク足、バー | `upColor`、`downColor` |
| ライン | `color` |
| エリア | `lineColor`、`topColor`、`bottomColor` |
| ベースライン | `topLineColor`、`bottomLineColor`、`topFillColor1`、`bottomFillColor1`、`baseValue: { type: 'price', price }` |
| ヒストグラム | `color`、`base` |
| すべて | `lineWidth`、`lineStyle`、`lineType`、`lastValueVisible`、`priceLineVisible`、`priceFormat.precision` |

そのため、`wickUpColor`、`borderUpColor`などのローソク足の色、`crosshairMarker*`、`priceFormat.minMove`、`priceFormat.type`、`title`、`visible`は効果がありません。`LineStyle.LargeDashed`は破線、`LineStyle.SparseDotted`は点線で描画されます。`LineType.WithSteps`は階段状に、`LineType.Curved`は直線の線分で描画されます。

**価格スケール**

- 各ペインには右側に価格スケールが1つあります。左側やオーバーレイの価格スケールはありません。`'right'`以外の`priceScaleId`（出来高オーバーレイでよく使われる`''`を含む）で作成したシリーズには専用のペインが割り当てられ、その高さはデフォルトのペインの30%で、警告が出力されます。出来高については、ネイティブの`addVolumeSeries`（後述）を使えばメインペインの下部に描画されます。
- `'right'`以外のidで`chart.priceScale(id)`を呼ぶと、警告が出力され、右側のスケールが返されます。

**時刻**

- 時刻はUTCのミリ秒に変換されます。`'YYYY-MM-DD'`文字列と`BusinessDay`オブジェクトはUTCの午前0時になります。その他の文字列形式は例外をthrowします。
- チャートが最初に受け取った時刻（`setData`または`update`）の形式によって、チャートが返す時刻の形式が決まります。イベント、`getVisibleRange`、`coordinateToTime`、`dataByIndex`、クロスヘアの`time`は、それに応じて秒、文字列、`BusinessDay`オブジェクトを使います。
- 論理インデックスは、lightweight-chartsと同様に最初の足が0です。

**データ**

- ホワイトスペース項目（`{ time }`のみ）は、ローソク足とバーのデータからは削除され、ライン、エリア、ベースライン、ヒストグラムのデータでは欠損（ギャップ）になります。
- このAPIではOHLCデータに出来高はありません。出来高シリーズにはネイティブAPIを使ってください。

**マーカー**

- `position: 'inBar'`は足の上に描画され、未知の`shape`は円として描画されます。`size`はデフォルトサイズの倍数です（1は10 CSS pxで、4〜64 pxの範囲に収められます）。`id`は無視されます。

**未対応の機能**

- `subscribeClick`は何もせず、警告を出力します。代わりに`chart.ferrochart`経由でネイティブの`click`イベントを使ってください。
- `series.priceToCoordinate`と`series.coordinateToPrice`は`null`を返し、警告を出力します。
- カスタムシリーズ、シリーズプリミティブ、ペインプリミティブ。FerroChart独自の拡張機能については[プラグイン](plugins.md)を参照してください。
- `scrollToPosition(position, animated)`は`animated`を無視します。
- `IPriceLine.options()`は、すべてのオプションではなく、渡したオプションだけを返します。

### 互換エントリーからネイティブ機能を使う

`chart.ferrochart`はネイティブの`FerroChart`なので、ページは既存のlightweight-chartsの呼び出しを残したまま、ネイティブ機能を併用できます。

```ts
import { createChart, CandlestickSeries } from '@ferrochart/core/lightweight-charts';

declare const container: HTMLElement;

const chart = createChart(container);
chart.addSeries(CandlestickSeries);
await chart.whenReady();

const fc = chart.ferrochart;
if (fc) {
  fc.drawings.startTool('trend_line');
  fc.on('click', (e) => console.log('clicked at', e.time, e.price));
}
```

ネイティブオブジェクト経由で追加したシリーズは互換レイヤーからは認識されません（たとえば、クロスヘアイベントの`seriesData`には含まれません）。

## ネイティブAPIへの移行

ネイティブAPIは、いくつかの全般的な点でlightweight-chartsと異なります。

- **作成は非同期。** `FerroChart.create(canvas, options)`はPromiseを返し、コンテナではなくCSSでサイズ指定された`<canvas>`要素を受け取ります。
- **時刻はUTCのミリ秒**で、`number`、`bigint`、または（行形式では）`Date`を使います。lightweight-chartsの秒に1000を掛けてください。
- **データはカラム形式にできる。** `setData`は行の配列も受け付けますが、大規模データでは型付き配列のカラムの方がはるかに高速です。
- **種類は文字列。** `chart.addSeries('candlestick', options, pane)`のように指定します。ペインはインデックスではなくidです。
- **オプションはフラット。** シリーズの色は`color`、`downColor`、`topFill`、`bottomFill`で、テーマは`'light'`または`'dark'`です。
- **イベントは`on`/`off`を使う。** `subscribeCrosshairMove(...)`の代わりに`chart.on('crosshairMove', ...)`を使います。

### 並べて比較

lightweight-charts：

```js
import { createChart, CandlestickSeries, HistogramSeries } from 'lightweight-charts';

const chart = createChart(document.getElementById('chart'), {
  layout: { background: { color: '#131722' }, textColor: '#d1d4dc' },
});
const candles = chart.addSeries(CandlestickSeries, { upColor: '#26a69a', downColor: '#ef5350' });
candles.setData(rows); // [{ time: 1700000000, open, high, low, close }]

const volume = chart.addSeries(HistogramSeries, { priceScaleId: '' });
volume.priceScale().applyOptions({ scaleMargins: { top: 0.8, bottom: 0 } });
volume.setData(rows.map((r) => ({ time: r.time, value: r.volume })));

chart.subscribeCrosshairMove((param) => {
  const bar = param.seriesData.get(candles);
  if (bar) legend.textContent = `C ${bar.close}`;
});
chart.timeScale().fitContent();
```

FerroChart：

```ts
import { FerroChart } from '@ferrochart/core';

declare const rows: { time: number; open: number; high: number; low: number; close: number; volume: number }[];
declare const legend: HTMLElement;

const canvas = document.getElementById('chart') as HTMLCanvasElement; // style="width: 100%; height: 480px"
const chart = await FerroChart.create(canvas, { theme: 'dark' });

const candles = chart.addCandlestickSeries({ color: '#26a69a', downColor: '#ef5350' });
candles.setData(rows.map((r) => ({ ...r, time: r.time * 1000 }))); // seconds to ms; volume included
chart.addVolumeSeries(candles); // reads the volume column, bottom of the main pane

chart.on('crosshairMove', ({ point }) => {
  if (point?.bar) legend.textContent = `C ${point.bar.close}`;
});
chart.fitContent();
```

### よく使う呼び出しの対応表

| lightweight-charts | FerroChart |
|---|---|
| `createChart(container, options)` | `await FerroChart.create(canvas, { theme, renderer })` |
| `chart.addSeries(CandlestickSeries, o)` / `addCandlestickSeries(o)` | `chart.addCandlestickSeries(o)`または`chart.addSeries('candlestick', o)` |
| `BarSeries` / `addBarSeries` | `chart.addSeries('ohlc', o)` |
| `LineSeries`、`AreaSeries`、`BaselineSeries`、`HistogramSeries` | `chart.addSeries('line' \| 'area' \| 'baseline' \| 'histogram', o)` |
| `chart.addSeries(def, o, paneIndex)` | `chart.addSeries(kind, o, chart.panes()[paneIndex])`。新しいペインは`chart.addPane()`で作成 |
| `priceScaleId: ''`を指定した出来高ヒストグラム | `chart.addVolumeSeries(candles)` |
| `chart.removeSeries(series)` | `series.remove()` |
| `series.setData(items)` | `series.setData(rows)`（時刻はms）または`series.setData({ time, open, high, low, close, volume })`のカラム |
| `series.update(item)` | `series.update(bar)`（時刻はms） |
| `series.applyOptions({ upColor, downColor })` | `series.applyOptions({ color, downColor })` |
| `series.applyOptions({ lineColor, topColor, bottomColor })` | `series.applyOptions({ color, topFill, bottomFill })` |
| `series.applyOptions({ priceFormat: { precision } })` | `series.applyOptions({ pricePrecision })` |
| `series.createPriceLine(o)` / `removePriceLine(line)` | `series.createPriceLine(o)` / `line.remove()` |
| `series.setMarkers(m)` / `createSeriesMarkers(series, m)` | `series.setMarkers(m)`（時刻はms） |
| `series.dataByIndex(i)` | `series.dataAt(logicalIndex)` |
| `chart.timeScale().fitContent()` | `chart.fitContent()` |
| `chart.timeScale().scrollToRealTime()` | `chart.scrollToRealtime()` |
| `timeScale().scrollToPosition(n)` / `scrollPosition()` | `chart.setRightOffset(n)` / `chart.rightOffset` |
| `timeScale().getVisibleLogicalRange()` | `chart.visibleLogicalRange()` |
| `timeScale().setVisibleLogicalRange({ from, to })` | `chart.setVisibleLogicalRange(from, to)` |
| `timeScale().setVisibleRange({ from, to })` | `chart.applySync({ type: 'visibleRange', from, to })`（ms） |
| `timeScale().applyOptions({ barSpacing, rightOffset })` | `chart.setBarSpacing(px)`、`chart.setRightOffset(bars)` |
| `timeScale().logicalToCoordinate(i)` / `coordinateToLogical(x)` | `chart.logicalToCoordinate(i)` / `chart.coordinateToLogical(x)` |
| `timeScale().timeToCoordinate(t)` | `chart.logicalToCoordinate(chart.indexOfTime(ms))` |
| `timeScale().coordinateToTime(x)` | `chart.timeAtIndex(chart.coordinateToLogical(x))` |
| `timeScale().subscribeVisibleLogicalRangeChange(h)` | `chart.on('visibleRangeChanged', h)` |
| `chart.priceScale('right').applyOptions({ scaleMargins, mode, invertScale, autoScale })` | `chart.applyPriceScaleOptions({ margins, mode, inverted, autoScale }, pane)` |
| `PriceScaleMode.Logarithmic` | `mode: 'logarithmic'`（ほかに`'normal'`、`'percentage'`、`'indexedTo100'`） |
| `chart.applyOptions({ crosshair: { mode: CrosshairMode.Magnet } })` | `chart.setCrosshairMode('magnet')` |
| `chart.subscribeCrosshairMove(h)` | `chart.on('crosshairMove', h)` |
| `chart.subscribeClick(h)` | `chart.on('click', h)` |
| `chart.applyOptions({ width, height })` / `resize(w, h)` | canvasのCSSサイズを設定すると、チャートが追従します |
| `chart.remove()` | `chart.destroy()` |

論理インデックスには1点だけ違いがあります。FerroChartでは古いデータを先頭に追加しても足の論理インデックスは変わらないため、最初の足のインデックスが負になることがあります。`chart.dataRange()`は最初と最後の足のインデックスを返します。

ネイティブの各呼び出しの詳細については、[シリーズ](series.md)、[データ](data.md)、[時間軸](time-scale.md)、[価格スケール](price-scale.md)、[クロスヘアとイベント](crosshair-events.md)、[マーカー](markers.md)を参照してください。
