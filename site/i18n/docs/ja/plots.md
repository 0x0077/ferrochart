<!-- source: de9b1ca7fd -->
# プロット（FerroPlot）

`FerroChart`は時系列の足を中心に設計されています。それ以外のもの、たとえばボラティリティ・スマイル、リターンのヒストグラム、セクター別の棒グラフ、損益のウォーターフォール、相関ヒートマップなどには`FerroPlot`を使います。これは数値軸、対数軸、カテゴリー軸、日付軸を備えた汎用のx/yプロットです。チャートと同じレンダラー（WebGPU、次にWebGL2）、同じcanvasの扱い、同じテーマを使います。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

const plot = await FerroPlot.create(canvas, { theme: 'dark' });

const smile = plot.addMark('line', { pointSize: 5 });
smile.setData({
  x: [80, 90, 100, 110, 120],          // strikes
  y: [0.31, 0.26, 0.24, 0.25, 0.29],   // implied volatility
});
plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 0 }, title: 'IV' });
plot.axis('x').applyOptions({ title: 'Strike' });
```

プロットは次の要素で構成されます。

- **マーク**：ライン、エリア、散布図、棒、ウォーターフォール、セル。それぞれが独自のデータとオプションを持ちます。
- 3本の**軸**：下端の`'x'`、`'right'`、`'left'`（デフォルトでは非表示）。
- **ルール**：ある値の位置に引く水平線・垂直線、または網掛けのバンド。
- **ラベル**：データ座標上のテキストまたはシンボル。

## プロットの作成

`FerroPlot.create(canvas, options?)`は`FerroChart.create`と同様にPromiseを返し、初回使用時にWebAssemblyモジュールを読み込みます。canvasのサイズはCSSで決まります。[テーマとスタイル](styling.md#sizing-the-canvas)を参照してください。

| オプション | 値 | デフォルト |
|---|---|---|
| `theme` | `'light'`、`'dark'` | `'light'` |
| `renderer` | `'auto'`、`'webgpu'`、`'webgl2'` | `'auto'` |
| `crosshair` | ホバー中の要素を通るクロスヘアライン | `false` |
| `hoverHighlight` | ホバー中の要素をハイライトする | `true` |
| `attributionLogo` | プロット領域の左下に表示するFerroChartのロゴ | `true` |

`FerroPlot`のその他のメンバー：

| メンバー | 説明 |
|---|---|
| `addMark(kind, options?): Mark` | マークを追加する。先に追加したマークの上に描画されます |
| `axis(name): Axis` | `'x'`、`'left'`、`'right'` |
| `addRule(options): Rule` | ある値の位置のラインまたはバンド |
| `addLabel(options): Label` | ある点に置くテキストまたはシンボル |
| `fit()` | すべての軸を設定された範囲に戻す。自動の軸は再びデータに追従します |
| `on(event, callback)`、`off(event, callback)` | イベント：`hover`、`click`、`viewChange` |
| `playIntro(options?)`、`finishIntro()` | イントロアニメーション（チャートと同じ） |
| `setAttributionLogo(on)` | ロゴを表示または削除する |
| `renderer` | `'webgpu'`または`'webgl2'` |
| `destroy()` | リスナーとロゴを削除し、GPUリソースを解放する（`dispose()`は以前の名前の同じメソッドです） |

ユーザーはプロットをパン・ズームできます。ドラッグでパン、ホイールまたはピンチでポインター位置を中心にx軸をズーム（y軸上、またはAltを押しながらだとその軸をズーム）、Shift+ホイールと横スクロールでパン、軸に沿ったドラッグでその軸をズーム、ダブルクリックでフィットします。カテゴリー軸はパンできますが、ズームはできません。

## マーク

`plot.addMark(kind, options?)`は、`MarkKind`の種類と`MarkOptions`でマークを追加し、`Mark`ハンドルを返します。

| 種類 | 描画内容 | 使用するデータ |
|---|---|---|
| `'line'` | データ順に結んだ点 | `x`、`y` |
| `'area'` | ラインと`y0`（または`baseValue`）の間の領域 | `x`、`y`、`y0?` |
| `'scatter'` | 点ごとのシンボル | `x`、`y`、`size?`、`value?`、`color?`、`label?` |
| `'bars'` | `y0`（または`baseValue`）から`y`までの棒 | `x`、`y`、`y0?`、`value?`、`color?`、`label?` |
| `'waterfall'` | カテゴリー軸上の累計の棒 | `x`、`y`、`step?` |
| `'cells'` | グリッドの色付きセル（ヒートマップ） | `x`、`y`、`value` |

### データ

`mark.setData(data)`はデータを置き換えます。すべてのカラムは同じ長さです。型付き配列はカラムごとに1回の`memcpy`でコピーされます。

| カラム | 型 | 意味 |
|---|---|---|
| `x` | `Float64Array \| number[]` | 数値、UTC ms（日付軸）、またはカテゴリー番号。横棒の場合は値 |
| `y` | `Float64Array \| number[]` | 値。`NaN`はギャップ。横棒の場合はy軸上の位置 |
| `y0` | `Float64Array \| number[]` | エリアの下端、棒の起点 |
| `value` | `Float64Array \| number[]` | カラースケールで色付けされる値（セル、棒、散布図） |
| `size` | `Float32Array \| number[]` | 散布図の直径（CSS px） |
| `color` | `(string \| null)[] \| Uint32Array` | 点ごとの色（`null`：マークの色）、または`0xRRGGBBAA` |
| `step` | `('delta' \| 'subtotal' \| 'total')[]` | ウォーターフォールでの役割。デフォルトはすべて`'delta'` |
| `label` | `(string \| null)[]` | 点ごとのテキスト：散布図のラベル。棒とセルでは値のテキストを置き換えます |

カラムの長さが異なる場合、`x`が有限でない場合、カテゴリー軸上の値が0以上の整数でない場合、`setData`は例外をthrowし、以前のデータを保持します。

データを変更する方法は3つあります。

| メソッド | 用途 |
|---|---|
| `setData(data)` | すべてを置き換える |
| `append(data)` | 末尾に点を追加する。`data`はマークのデータと同じカラムを持つ必要があります |
| `update(point)` | 最後の点を置き換える（ライブの値など）。O(1)、アロケーションなし |

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const pnl = plot.addMark('area', { color: '#26a69a', downColor: '#ef5350' });
plot.axis('x').applyOptions({ format: 'date' });

const t0 = Date.UTC(2024, 0, 2);
pnl.setData({ x: [t0, t0 + 60_000, t0 + 120_000], y: [0, 120, 80] });
pnl.append({ x: [t0 + 180_000], y: [150] });   // a new point
pnl.update({ x: t0 + 180_000, y: 165 });         // the last point changed
```

`update`は`MarkPoint`（`{ x, y, y0?, value?, size?, color? }`）を受け取ります。マークにないカラムのフィールドは無視されます。マークにデータがない場合や`x`が有限でない場合は例外をthrowします。カラースケールの定義域と自動のセルサイズは、最後の`setData`または`append`で計算された値が保持されます。

### オプション

オプションは`addMark`に渡し、`mark.applyOptions(options)`で変更します。省略したキーは現在の値を保持します。`mark.options()`はすべてのオプションを返します。色の`null`はデフォルトを意味します。`color`ではプロットのパレットの次の色（最初のマークはテーマのラインの色）、それ以外ではテーマの色になります。

| オプション | 対象 | 意味 | デフォルト |
|---|---|---|---|
| `yAxis` | すべて | `'left'`または`'right'` | `'right'` |
| `orientation` | bars、waterfall | `'vertical'`または`'horizontal'` | `'vertical'` |
| `color` | すべて | メインの色 | パレット |
| `downColor` | area、waterfall | 基準値より下のエリア、ウォーターフォールの減少項目 | テーマ |
| `totalColor` | waterfall | 小計と合計 | テーマ |
| `fillAlpha` | area | 塗りの不透明度、0〜1 | `0.25` |
| `lineWidth` | line、area | CSS px、0より大きい値 | `2` |
| `lineStyle` | line、area | `'solid'`、`'dashed'`、`'dotted'` | `'solid'` |
| `stepLine` | line、area | ステップ：次のxまで水平、その後垂直 | `false` |
| `pointSize` | line、scatter | ライン：各点のシンボル（0：なし）。散布図：`size`カラムがないときの直径 | `0` |
| `icon` | line、scatter | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'` | `'circle'` |
| `baseValue` | area、bars | `y0`がないエリアと棒の起点 | `0` |
| `group` | bars | 同じグループの棒は1つのスロットを横に並んで共有する。`null`でなし | `null` |
| `stack` | bars、area | 同じスタックのマークは積み上げられる（正の値と負の値は別々）。`null`でなし | `null` |
| `band` | bars、cells | スロットに占める割合、0（含まない）〜1。`null`でデフォルト（棒は0.8、セルは1） | `null` |
| `colorScale` | cells、bars、scatter | `value`を色に対応付ける方法（後述）。`null`でデフォルトの連続スケール | `null` |
| `showValues` | bars、waterfall、cells | 収まる場合、各要素に値を表示する | `false` |
| `valueFormat` | bars、waterfall、cells | その値の形式（[形式](#formats)を参照） | `'auto'` |
| `cellSize` | cells | 数値軸上でのデータ単位の`[width, height]`。`null`：異なるx同士、異なるy同士の最小の間隔 | `null` |
| `hoverable` | すべて | ホバーとクリックでヒットする | `true` |
| `visible` | すべて | 描画する | `true` |

無効な値は例外をthrowします。たとえば`'fillAlpha' must be between 0 and 1`や`'icon' must be 'circle', 'square', 'arrowUp' or 'arrowDown'`です。

`mark.id`はプロット内で一意で、`mark.kind`は種類、`mark.remove()`はマークを削除します。

### カテゴリー軸上の棒

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Tech', 'Energy', 'Banks', 'Health']);

// Two series side by side in each slot.
const q1 = plot.addMark('bars', { group: 1, showValues: true });
const q2 = plot.addMark('bars', { group: 1, showValues: true });
q1.setData({ x: [0, 1, 2, 3], y: [4.2, -1.3, 2.1, 0.8] });
q2.setData({ x: [0, 1, 2, 3], y: [3.1, 0.6, -0.4, 1.9] });

plot.axis('right').applyOptions({ format: { type: 'percent', decimals: 1 } });
plot.addRule({ value: 0, lineStyle: 'solid' });
```

カテゴリー`k`は`k`から`k + 1`までのスロットを占め、ラベルは`labels[k]`です。`group: 1`の代わりに`stack: 1`を指定すると、棒は積み上げられます。横棒にするには、`orientation: 'horizontal'`を設定し、左または右の軸をカテゴリー軸にして、カラムの意味を入れ替えます。`x`に値を、`y`にカテゴリー番号を入れます。

### ウォーターフォール

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const x = plot.axis('x');
x.applyOptions({ type: 'category' });
x.setCategories(['Revenue', 'COGS', 'Gross', 'Opex', 'Tax', 'Net']);

const pnl = plot.addMark('waterfall', {
  color: '#26a69a',       // increases
  downColor: '#ef5350',   // decreases
  totalColor: '#5d6d7e',  // subtotals and totals
  showValues: true,
  valueFormat: 'compact',
});
pnl.setData({
  x: [0, 1, 2, 3, 4, 5],
  y: [1_200_000, -450_000, 0, -380_000, -90_000, 0],
  step: ['delta', 'delta', 'subtotal', 'delta', 'delta', 'total'],
});
```

`'delta'`は自身の値を累計に加えます。`'subtotal'`または`'total'`は0から累計までの棒を描画し、累計は変えません。その`y`は使われません。hoverイベントとclickイベントでは、小計または合計の`hit.y`は累計になります。

### サイズ・色・ラベル付きの散布図

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const assets = plot.addMark('scatter', { colorScale: 'diverging' });
assets.setData({
  x: [12.1, 18.4, 25.0, 9.8],          // volatility, %
  y: [6.2, 9.1, 14.7, 3.9],            // return, %
  size: [10, 18, 24, 8],               // diameter, CSS px
  value: [0.4, -0.2, 1.1, -0.6],       // e.g. Sharpe ratio, colored
  label: ['Bonds', 'Gold', 'Tech', 'Cash'],
});
plot.axis('x').applyOptions({ title: 'Volatility %' });
plot.axis('right').applyOptions({ title: 'Return %', includeZero: true });
```

### セル（ヒートマップ）

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const names = ['SPX', 'NDX', 'GLD', 'TLT'];
const corr = [
  [1, 0.92, 0.05, -0.31],
  [0.92, 1, 0.02, -0.35],
  [0.05, 0.02, 1, 0.21],
  [-0.31, -0.35, 0.21, 1],
];

for (const name of ['x', 'right'] as const) {
  const a = plot.axis(name);
  a.applyOptions({ type: 'category' });
  a.setCategories(names);
}

const x: number[] = [], y: number[] = [], value: number[] = [];
corr.forEach((row, i) => row.forEach((v, j) => { x.push(j); y.push(i); value.push(v); }));

const cells = plot.addMark('cells', {
  colorScale: { center: 0, domain: [-1, 1] },
  showValues: true,
  valueFormat: { type: 'fixed', decimals: 2 },
});
cells.setData({ x, y, value });
```

数値軸上では、セルの幅と高さは`cellSize`データ単位、または異なる`x`の値同士、異なる`y`の値同士の最小の間隔になります。

## 軸

`plot.axis(name)`は`'x'`、`'left'`、`'right'`の軸を返します。左の軸はデフォルトで非表示です。マークは`yAxis: 'left'`でこれを使います。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('left').applyOptions({ visible: true, title: 'Volume', format: 'compact' });
plot.addMark('bars', { yAxis: 'left', color: 'rgba(120, 120, 120, 0.5)' });
plot.addMark('line', { yAxis: 'right' });
```

### `AxisOptions`

`axis.applyOptions(options)`はオプションを変更します。省略したキーは現在の値を保持します。`axis.options()`はすべてのオプションを返します。

| オプション | 意味 | デフォルト |
|---|---|---|
| `type` | `'linear'`、`'log'`、`'category'` | `'linear'` |
| `range` | `'auto'`（データに追従）、または固定の`[lo, hi]` | `'auto'` |
| `includeZero` | 自動範囲：0を範囲内に含める | `false` |
| `margin` | 自動範囲：両端に加える割合、0〜1 | `0.05`（カテゴリー：`0`） |
| `format` | 目盛りラベルの書式（後述） | `'auto'` |
| `inverted` | 値が右へ（x）または上へ（y）向かって減少する | `false` |
| `visible` | 目盛り、ラベル、軸線を描画する | `true`（左：`false`） |
| `grid` | 目盛りの位置のグリッド線（カテゴリー軸ではスロットの境界） | `true`（カテゴリー：`false`） |
| `title` | 軸の横に表示するテキスト。`null`で削除 | `null` |
| `locked` | パンとズームで軸を変更しない | `false` |

`type`を変更すると、`grid`、`margin`、`range`はその型のデフォルトに戻ります。`visible`、`title`、`format`、`inverted`、`locked`は保持されます。

自動範囲は、その軸上のマークの表示中のデータをカバーします。棒、エリア、ウォーターフォールでは、0にある端にはマージンが付かないため、棒は軸の上に立ちます。カテゴリー軸はすべてのカテゴリーにわたります。

エラー：固定範囲は2つの異なる有限の数値で、対数軸では正の値である必要があります。

### 対数軸

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('right').applyOptions({ type: 'log' });
```

0以下の値は対数軸上に位置を持たないため、描画されません。

### カテゴリー軸

カテゴリー軸は等幅のスロットを持ちます。カテゴリー`k`は`[k, k + 1)`を占め、その中心は`k + 0.5`です。その軸上のデータは0以上の整数です。カテゴリー名は`axis.setCategories(labels)`で付けます（最大1 000 000個）。

### 日付軸

日付軸は、値がUTCミリ秒で`'date'`形式を持つ線形軸です。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

plot.axis('x').applyOptions({ format: 'date' });                              // UTC
plot.axis('x').applyOptions({ format: { type: 'date', utcOffsetMinutes: 60 } }); // UTC+1
```

### 形式

`PlotFormat`は`AxisOptions.format`と`MarkOptions.valueFormat`で使われます。

| 形式 | 表記 |
|---|---|
| `'auto'` | 小数点以下2桁以上かつ有効数字4桁以上。小さい値ではゼロの個数を下付き数字で表記（チャートの価格ラベルと同じ） |
| `'compact'` | K、M、Bの接尾辞 |
| `{ type: 'fixed', decimals }` | 固定の小数点以下桁数、0〜12 |
| `{ type: 'percent', decimals }` | 固定の小数点以下桁数と`%`記号。値はすでにパーセント単位（`5`は`5%`と表記） |
| `'date'` | 値はUTC msで、UTCの日付と時刻として表記 |
| `{ type: 'date', utcOffsetMinutes }` | 同上、UTCからのオフセット付き（−1440〜1440分） |

### 表示範囲

| メソッド | 説明 |
|---|---|
| `axis.view(): [number, number]` | 最後のフレーム時点での表示範囲 |
| `axis.setView(lo, hi)` | `lo..hi`を表示する（カテゴリー軸ではカテゴリー単位）。`plot.fit()`までデータへの追従を止めます。`viewChange`イベントは送られません |
| `plot.fit()` | すべての軸を設定された範囲に戻す |

## ルール

ルールは、ある値の位置でプロットを横切るライン、または2つの値の間の網掛けバンドです。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const target = plot.addRule({ value: 100, label: 'target' });          // horizontal line on the right axis
plot.addRule({ axis: 'x', value: Date.UTC(2024, 5, 1), label: 'FOMC' }); // vertical line
plot.addRule({ value: 30, to: 70, color: 'rgba(120, 120, 255, 0.12)' }); // band

target.applyOptions({ value: 105 });
target.remove();
```

| オプション | 意味 | デフォルト |
|---|---|---|
| `axis` | `value`の軸。`'x'`なら垂直のルール | `'right'` |
| `value` | 位置 | `0` |
| `to` | バンドのもう一方の端。`null`ならライン | `null` |
| `color` | 色。`null`ならテーマのクロスヘアの色（バンドは不透明度12%） | `null` |
| `lineWidth` | CSS px | `1` |
| `lineStyle` | `'solid'`、`'dashed'`、`'dotted'` | `'dashed'` |
| `label` | 値の位置で軸上に表示するテキスト。`null`でなし | `null` |
| `labelBackground` | ラベルをルールの色のボックス上に表示する（そうでなければルールの色で文字を描く） | `true` |

## ラベル

ラベルは、データ座標上の点に置くテキスト、シンボル、またはその両方です。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const peak = plot.addLabel({ x: 3, y: 14.7, text: 'peak', icon: 'arrowDown', align: 'center', offset: [0, -14] });
peak.remove();
```

| オプション | 意味 | デフォルト |
|---|---|---|
| `x`、`y` | 点（必須） | |
| `yAxis` | `'left'`または`'right'` | `'right'` |
| `text` | テキスト（アイコンがあれば空でもよい） | `''` |
| `icon` | `'circle'`、`'square'`、`'arrowUp'`、`'arrowDown'`、または`null` | `null` |
| `color` | `null`ならテーマのテキストの色 | `null` |
| `align` | 点に対するテキストの配置：`'left'`、`'center'`、`'right'` | `'left'` |
| `offset` | 点からのテキストのオフセット（CSS px、右方向と下方向） | `[6, 0]` |

ラベルは変更できません。削除して新たに追加してください。

## カラースケール

セルは常に`value`カラムをカラースケールで色付けします。棒と散布図のマークは、`value`カラムがある場合にそうし、ない場合は`color`を使います。`color`カラムで指定した点自身の色は、値よりも優先されます。

| `colorScale` | 意味 |
|---|---|
| `null`または`'sequential'` | 低い値から高い値へ、濃い青から黄色 |
| `'diverging'` | 0を中心に、テーマの下落色、中間の中立色、上昇色 |
| `{ stops?, center?, domain?, quantile? }` | 独自のスケール |

オブジェクト形式：

| キー | 意味 |
|---|---|
| `stops` | `[position, color][]`。位置は0〜1の昇順（1つ以上のストップが必要） |
| `center` | この値を中心とする発散型のスケールにする。スケールの中央が`center`になり、そこから遠い側によって両側の範囲が決まります |
| `domain` | 固定の`[lo, hi]`。範囲外の値には端の色が付きます |
| `quantile` | `domain`がない場合：スケールは`quantile`分位点から`1 − quantile`分位点までの値をカバーするため、外れ値は飽和します。0〜0.5、デフォルト0.01 |

`stops`がない場合、`center`のあるスケールは発散型の色を、ないスケールは連続型の色を使います。

```ts
import { FerroPlot } from '@ferrochart/core';

declare const plot: FerroPlot;

const heat = plot.addMark('cells', {
  colorScale: { stops: [[0, '#0d1b4c'], [0.5, '#219ebc'], [1, '#f4e04d']], quantile: 0.05 },
});
```

定義域は`setData`と`append`でデータから計算され、`update`ではそのまま保持されます。

## ヒットテストとイベント

プロットはポインターの下にある要素（ラインや散布図の点、棒、セル）を見つけます。それをハイライトし（`hoverHighlight`）、そこを通るクロスヘアラインを描画し（`crosshair`）、イベントで通知します。

```ts
interface PlotEventMap {
  hover: { hit: PlotHit | null };                 // the hovered element changed
  click: { hit: PlotHit | null; x: number; y: number };
  viewChange: {};                                  // an axis range changed by pan, zoom or fit()
}

interface PlotHit {
  mark: Mark;      // compare with mark.id: it is a new handle each time
  index: number;   // index of the point in the mark's data
  x: number;
  y: number;       // waterfall subtotals and totals: the running total
  position: { x: number; y: number }; // where it is drawn, CSS px from the canvas' top-left
}
```

`click`では、`x`と`y`はポインター位置での軸の値です（`y`は右の軸の値、右の軸が非表示なら左の軸の値）。`hoverable: false`のマークは決してヒットしません。

```ts
import { FerroPlot, type PlotHit } from '@ferrochart/core';

declare const plot: FerroPlot;
declare const tooltip: HTMLElement;

const bars = plot.addMark('bars', { showValues: true });
bars.setData({ x: [0, 1, 2], y: [5, 6, 7] });

plot.on('hover', ({ hit }: { hit: PlotHit | null }) => {
  if (!hit || hit.mark.id !== bars.id) {
    tooltip.hidden = true;
    return;
  }
  tooltip.hidden = false;
  tooltip.textContent = `#${hit.index}: ${hit.y}`;
  tooltip.style.transform = `translate(${hit.position.x}px, ${hit.position.y}px)`;
});

plot.on('click', ({ hit, x, y }: { hit: PlotHit | null; x: number; y: number }) => {
  console.log(hit ? `clicked point ${hit.index}` : `empty space at ${x}, ${y}`);
});
```

イベントは、それを発生させたフレームの後に配信されます。

## プロットをチャートのペインに揃える

`chart.attachSidePlot(plot, pane?)`は、チャートのペインの横にプロットを配置し、同じ価格が両方で同じ高さになるようにします。チャートは毎フレーム、ペインの対応付けに従って、プロットの右の軸をプロット領域の上端と下端の価格に設定します。canvasのページ上の位置は毎フレーム読み取られるため、プロットはチャートの横のどこにあってもよく、プロット領域がペインより高くても低くても構いません。

- プロットの右の軸はロックされ、チャートが唯一の情報源になります。アタッチ時にペインの価格スケールが対数であれば、右の軸は対数軸になります。
- プロットのアトリビューションロゴは削除され、チャートのロゴが両方を代表します。
- `chart.detachSidePlot(plot)`で追従を止めます。右の軸はロックされたまま、最後の範囲を保持します。

### 例：価格帯別出来高

価格帯別出来高（ボリュームプロファイル）は、表示中の足の各価格でどれだけの出来高があったかを、ローソク足の横に横棒で示します。

```html
<div style="display: flex; height: 520px">
  <canvas id="chart" style="flex: 1; min-width: 0"></canvas>
  <canvas id="profile" style="width: 160px"></canvas>
</div>
```

```ts
import { FerroChart, FerroPlot, randomWalk, type OhlcData } from '@ferrochart/core';

const chart = await FerroChart.create(document.getElementById('chart') as HTMLCanvasElement, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
const bars: OhlcData = randomWalk(3, 3_000, Date.UTC(2024, 0, 1), 60_000);
candles.setData(bars);
chart.addVolumeSeries(candles);

const profile = await FerroPlot.create(document.getElementById('profile') as HTMLCanvasElement, {
  theme: 'dark',
  hoverHighlight: false,
});
profile.axis('x').applyOptions({ visible: false, grid: false });
profile.axis('right').applyOptions({ visible: false, grid: false }); // the chart shows the prices
const rows = profile.addMark('bars', { orientation: 'horizontal', color: 'rgba(41, 98, 255, 0.5)', band: 0.9 });
chart.attachSidePlot(profile);

/** Volume per price row of the bars `from..=to` (indices into `bars`). */
function volumeProfile(from: number, to: number, rowSize: number) {
  const byRow = new Map<number, number>();
  for (let i = from; i <= to; i++) {
    const row = Math.floor(bars.close[i] / rowSize);
    byRow.set(row, (byRow.get(row) ?? 0) + (bars.volume?.[i] ?? 0));
  }
  const y: number[] = [], x: number[] = [];
  for (const [row, volume] of byRow) {
    y.push((row + 0.5) * rowSize); // price at the middle of the row
    x.push(volume);
  }
  return { x, y };
}

const n = bars.close.length;
const update = (from: number, to: number) => {
  const a = Math.max(0, Math.floor(from));
  const b = Math.min(n - 1, Math.ceil(to));
  if (a <= b) rows.setData(volumeProfile(a, b, 0.5));
};
update(n - 200, n - 1);
chart.on('visibleRangeChanged', ({ from, to }: { from: number; to: number }) => update(from, to));
```

数値軸上の横棒の太さは、`y`の値同士の最小の間隔に`band`を掛けたものになるため、同じサイズの行は`band: 1`で隙間なく接します。ユーザーがチャートをスクロールまたはズームすると、`visibleRangeChanged`によって表示中の足についてプロファイルが再計算され、価格範囲は自動的に追従します。ローソク足の横に板情報（デプス）を表示する場合も同じ構成で、買い・売りの数量を`x`に、その価格を`y`にし、最後のレベルのライブ変化には`rows.update`を使います。

## 後片付け

`plot.destroy()`は、プロットのリスナーとロゴを削除し、GPUリソースを解放します。サイドプロットの場合はチャートへの追従も止まります。その後、`Mark`、`Axis`、`Rule`、`Label`のハンドルは例外（`the FerroPlot was destroyed`）をthrowします。canvas要素はページに残ります。
