<!-- source: c2e84c153f -->
# シリーズの種類

シリーズとは、ひとつの描き方で描画されるひとまとまりのデータです。ローソク足、ライン、ヒストグラムなどがあります。チャートには任意の数のシリーズを任意の数のペインに配置できます。チャート内のすべてのシリーズは1本の時間軸を共有します（[データ](data.md)を参照）。

## シリーズの追加

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);

const candles = chart.addSeries('candlestick');           // main pane
const sma = chart.addSeries('line', { color: '#2962ff' }); // with options
const pane = chart.addPane(0.3);
const osc = chart.addSeries('histogram', {}, pane);        // on another pane
```

`addSeries(kind, options?, pane?)`は`Series`ハンドルを返します。`pane`を省略すると、シリーズはメイン（最上段）のペインに配置されます。`addCandlestickSeries(options?, pane?)`は`addSeries('candlestick', ...)`の省略形です。未知の種類やペインidを指定すると例外がthrowされます。

`Series`には読み取り専用のプロパティが3つあります。

| プロパティ | 意味 |
|---|---|
| `id` | チャート内で一意の数値。同じシリーズのハンドル（たとえば`click`イベントから得たもの）は同じidを持ちます。 |
| `kind` | `addSeries`に渡した種類。出来高シリーズは`'volume'`、変換シリーズのうちKagiとポイント・アンド・フィギュアは`'kagi'` / `'pointFigure'`（Renkoとレンジバーは`'candlestick'`または`'ohlc'`を返します）、プラグインシリーズは`'custom'`です。 |
| `pane` | シリーズが配置されているペインのid。 |

## 種類

| 種類 | データ | 描画内容 |
|---|---|---|
| `'candlestick'` | OHLC | ローソク足：始値から終値までの実体と、安値から高値までのヒゲ |
| `'ohlc'` | OHLC | 左に始値、右に終値の目盛りが付いたバー |
| `'heikinAshi'` | OHLC | 渡した足から計算される平均足 |
| `'line'` | 値 | 値を結ぶライン |
| `'area'` | 値 | ペインの下端までグラデーションで塗りつぶされたライン |
| `'baseline'` | 値 | ラインと、`baseValue`の上下で色の異なる塗りつぶし |
| `'histogram'` | 値 | `baseValue`から各値までのバー。ポイントごとに色を指定可能 |
| `'points'` | 値 | 各値に打たれる点 |

出来高ヒストグラムは`addVolumeSeries`で追加し（[出来高](#volume)を参照）、Renko、レンジバー、Kagi、ポイント・アンド・フィギュアは`addTransformSeries`で追加します（[足の変換](#bar-transforms)を参照）。

OHLCシリーズは`{ time, open, high, low, close, volume? }`を、値シリーズは`{ time, value, color? }`を受け取ります。両方の形式、行配列、検証、リアルタイム更新については[データ](data.md)で説明しています。

### ローソク足

```ts
const candles = chart.addCandlestickSeries({
  color: '#26a69a',      // up candles (close >= open)
  downColor: '#ef5350',  // down candles
});
candles.setData(randomWalk(1, 500, Date.UTC(2024, 0, 1), 60_000));
```

`color`と`downColor`のどちらも設定しない場合、ヒゲにはテーマのヒゲの色が使われます。どちらか一方でも設定すると、ヒゲは実体と同じ色になります。

### OHLCバー

`'ohlc'`は`'candlestick'`と同じデータとオプションを受け取り、伝統的なバーチャートを描画します。

### 平均足

`'heikinAshi'`は通常のOHLCの足を受け取り、チャートがそこから平均足の値を計算します。リアルタイム更新にも対応しています。色の扱いはローソク足と同じです。

### ライン

```ts
const line = chart.addSeries('line', { color: '#2962ff', lineWidth: 1, lineStyle: 'dashed' });
line.setData({ time: [0, 60_000, 120_000, 180_000], value: [10, 11, NaN, 12] });
```

`NaN`の値はギャップになり、そこを通るラインは描画されません。

### エリア

エリアは、下側に垂直方向のグラデーションが付いたラインです。`topFill`はライン位置の色、`bottomFill`はペイン下端の色です。

```ts
chart.addSeries('area', {
  color: '#2962ff',
  topFill: 'rgba(41, 98, 255, 0.4)',
  bottomFill: 'rgba(41, 98, 255, 0)',
});
```

デフォルトでは、どちらの塗りも`color`で、不透明度は上端が40%、下端が2%です。

### ベースライン

ベースラインシリーズは`baseValue`を境に分かれます。上側ではラインが`color`、塗りが`topFill`になり、下側ではラインが`downColor`、塗りが`bottomFill`になります。

```ts
chart.addSeries('baseline', {
  baseValue: 100,
  color: '#26a69a',
  downColor: '#ef5350',
});
```

デフォルトでは、塗りは2つのラインの色を不透明度20%にしたものです。

### ヒストグラム

バーは`baseValue`（デフォルトは0）から各値まで伸びます。自動スケールの範囲には常に`baseValue`が含まれます。

ポイントごとに個別の色を指定できます。`value`と並べて`color`カラムを渡してください。要素はCSSの色、シリーズの色を使う場合は`null`、または`0xRRGGBBAA`値の`Uint32Array`（`0`はシリーズの色を意味します）です。

```ts
const hist = chart.addSeries('histogram', { color: '#26a69a' }, pane);
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: [null, '#ef5350', null],
});

// The same colors as packed integers.
hist.setData({
  time: [0, 60_000, 120_000],
  value: [1.5, -0.7, 0.4],
  color: new Uint32Array([0, 0xef5350ff, 0]),
});
```

ポイントごとの色は、最も詳細な表示のときにだけ描画されます。ズームアウトして複数の足が1ピクセルを共有し統合されるようになると、シリーズの色が使われます。他の種類も`color`カラムを受け付けますが、無視します。`update`では色を設定できず、追加されたポイントはシリーズの色になります。

### ポイント

`'points'`は各値に点を描画します。`lineWidth`は点の直径（CSS px）です。ギャップ（`NaN`）には点が描画されません。

```ts
const dots = chart.addSeries('points', { color: '#ff9800', lineWidth: 4 });
```

### 出来高

出来高シリーズは、OHLCシリーズの`volume`カラムを描画します。独自のデータは持たず、元のシリーズに対する`setData`と`update`によって更新されます。

```ts
const candles = chart.addCandlestickSeries();
const volume = chart.addVolumeSeries(candles);          // main pane
// or: chart.addVolumeSeries(candles, chart.addPane(0.25));
```

元のシリーズはOHLCシリーズ（ローソク足、OHLCバー、平均足）または変換シリーズである必要があります。バーは、元の足が陽線か陰線かに応じてテーマの出来高の色で塗られます。価格も表示しているペインでは、出来高はペイン下部の4分の1に独自のスケールで表示されます。専用のペインでは、ペイン全体を使います。[価格スケールとペイン](price-scale.md#volume-placement)を参照してください。

元のシリーズを削除すると、その出来高シリーズも削除されます。

## シリーズのオプション

オプションは`addSeries`に渡し、後から`applyOptions`で変更します。省略したキーは現在の値を保持します。`options()`はすべてのキーを返します。

```ts
const s = chart.addSeries('line', { color: '#2962ff' });
s.applyOptions({ lineWidth: 3 });       // color stays '#2962ff'
console.log(s.options().lineWidth);     // 3
s.applyOptions({ color: null });        // back to the theme color
```

| オプション | 型 | デフォルト | 対象 | 意味 |
|---|---|---|---|---|
| `color` | `Color \| null` | テーマ | すべて | ラインの色（ライン、エリア、ポイント）、陽線のローソク足とバー、ヒストグラムのバー、ベースラインの基準より上のライン、Kagiの陽線とポイント・アンド・フィギュアのX列 |
| `downColor` | `Color \| null` | テーマ | candlestick、ohlc、heikinAshi、baseline、変換 | 陰線のローソク足とバー、ベースラインの基準より下のライン、Kagiの陰線とポイント・アンド・フィギュアのO列 |
| `topFill` | `Color \| null` | [エリア](#area)、[ベースライン](#baseline)を参照 | area、baseline | エリアのライン位置の塗り、ベースラインの基準より上の塗り |
| `bottomFill` | `Color \| null` | [エリア](#area)、[ベースライン](#baseline)を参照 | area、baseline | エリアの下端の塗り、ベースラインの基準より下の塗り |
| `lineWidth` | `number` | `2` | line、area、baseline、points、Kagi、ポイント・アンド・フィギュア | ラインの太さ（CSS px）。ポイントの場合は点の直径。正の値である必要があります。 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | line、area、baseline | ラインの破線パターン |
| `step` | `boolean` | `false` | line、area、baseline | ステップライン。[ステップライン](#step-lines)を参照 |
| `baseValue` | `number` | `0` | baseline、histogram | ベースラインの塗りが分かれる位置、ヒストグラムのバーの起点。有限の値である必要があります。 |
| `lastValueVisible` | `boolean` | `true` | volume以外すべて | 価格軸上の最終値ラベル |
| `priceLineVisible` | `boolean` | `true` | volume以外すべて | 最終値の位置に引かれる水平の点線 |
| `shift` | `number` | `0` | すべて | 足数単位の表示シフト。[シフト](#shift)を参照 |
| `pricePrecision` | `number \| null` | `null` | すべて | 価格ラベルの小数点以下の桁数。[価格の精度](#price-precision)を参照 |

無効な値（未知のラインスタイル、不正な形式の色、負の太さ）を指定すると例外がthrowされ、シリーズは以前のオプションを保持します。

出来高シリーズは常にテーマの出来高の色を使います。

### 色

色はCSSの文字列で指定します：`#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`、`rgb(r, g, b)`、`rgba(r, g, b, a)`（アルファは0から1）、または`'transparent'`。`'red'`のような色名は受け付けません。`null`はテーマのデフォルトを意味します。`options()`は色を`#rrggbb`、半透明の場合は`rgba(...)`で返し、テーマの色の場合は`null`を返します。

### ステップライン

`step: true`を指定すると、ラインは次のポイントの時刻まで水平に進み、そこから垂直にその値まで移動します。金利やポジションサイズのように、変わるまで同じ値が続くデータに使います。

```ts
const rate = chart.addSeries('line', { step: true, color: '#7e57c2' }, pane);
rate.setData({
  time: [Date.UTC(2024, 0, 1), Date.UTC(2024, 2, 20), Date.UTC(2024, 6, 31)],
  value: [5.5, 5.25, 5.0],
});
```

### シフト

`shift`は論理インデックス`j`のポイントを`j + shift`の位置に描画します。正の値はシリーズを右（未来方向）に、負の値は左に移動します。整数である必要があります。自動スケール、クロスヘア、`dataAt`は描画された位置に従います。一目均衡表の先行スパンと遅行スパンはこれを使っています。

```ts
// A moving average drawn 10 bars ahead (a "displaced" moving average).
const dma = chart.addSeries('line', { shift: 10 });
```

### 価格の精度

`pricePrecision`は、価格軸ラベル、最終値ラベル、クロスヘアラベルの小数点以下の桁数を0から12の範囲で設定します。`null`（デフォルト）は自動で、表示中の価格に対して小数点以下2桁以上かつ有効数字4桁以上になります。非常に小さい価格は、ゼロの個数を下付き数字で表記します：`0.0₄1234`は0.00001234です。

ペインのラベルは、そのペインの最初の価格シリーズの精度を使います。価格スケールの`'percentage'`モードと`'indexedTo100'`モードでは、軸は常に小数点以下2桁です。0から12の整数または`null`以外の値は例外をthrowします。

```ts
candles.applyOptions({ pricePrecision: 5 }); // FX quotes
candles.applyOptions({ pricePrecision: null }); // automatic again
```

## 足の変換

変換シリーズは、通常の足（またはティック）を受け取り、チャート手法によって変換した結果を描画します。Renkoのブロック、レンジバー、Kagiのライン、ポイント・アンド・フィギュアの列です。これらの手法は時間を無視し、各要素が時間軸上に専用のスロットを持ちます。

```ts
const renko = chart.addTransformSeries('renko', { boxSize: 0.5 });
renko.setData(randomWalk(1, 5_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(renko); // optional
chart.fitContent();
```

`addTransformSeries(name, params, options?, pane?)`は`Series`を返します。ローソク足シリーズとまったく同じように`setData`と`update`でデータを渡します。ティックは4つの価格がすべて等しい足として渡します。

```ts
renko.update({ time: tick.time, open: tick.price, high: tick.price, low: tick.price, close: tick.price, volume: tick.size });
```

### パラメーター

価格はデータと同じ単位です。

| 変換 | パラメーター | 描画形式 |
|---|---|---|
| `'renko'` | `boxSize`（価格）**または** `atr`（ATRの期間、整数）、`kind?` | ローソク足（デフォルト）またはOHLCバー |
| `'rangeBars'` | `range`（価格）、`kind?` | ローソク足（デフォルト）またはOHLCバー |
| `'kagi'` | `reversal`（価格）**または** `reversalPercent` | Kagiのライン |
| `'pointFigure'` | `boxSize`（価格）、`reversal?`（ボックス数、整数、デフォルトは3） | XとOの列 |

`kind`は`'candlestick'`（デフォルト）または`'ohlc'`です。数値は有限の正の値である必要があります。二者択一のパラメーターを両方指定した場合（`boxSize`と`atr`、または`reversal`と`reversalPercent`）、どちらも指定しない場合、未知の名前を指定した場合は例外がthrowされます。

```ts
chart.addTransformSeries('renko', { atr: 14, kind: 'ohlc' });
chart.addTransformSeries('rangeBars', { range: 1 });
chart.addTransformSeries('kagi', { reversalPercent: 2 });
chart.addTransformSeries('pointFigure', { boxSize: 1, reversal: 3 });
```

（これらの行はそれぞれ別のチャートが必要です。[制約](#constraints)を参照してください。）

### Renko

Renkoは終値だけを使います。終値が直前のブロックから`boxSize`分以上動くたびにブロックが追加されます。同じ方向に続く場合は1ボックス、反転する場合は2ボックスの値動きが必要です。最初のブロックは最初の終値を基準にします。

`atr: n`を指定すると、ボックスサイズは最初の`n`本の足のATR（アベレージ・トゥルー・レンジ）になり、以後は固定されます。それまでは何も描画されません。

Renkoには形成中のブロックはありません。ブロックは完成した時点で初めて表示されます。

### レンジバー

すべてのバーは、安値から高値までの幅がちょうど`range`になります。入力された足の中では、陽線なら始値→安値→高値→終値、陰線なら始値→高値→安値→終値の順に価格が動いたとみなします。価格が現在のバーの範囲を外れると、そのバーは完成し、次のバーがその端から始まります。最後のバーは形成中で、更新に応じて変化します。

### Kagi

Kagiのラインは終値をその方向に追い、終値が`reversal`以上（またはラインの終点価格の`reversalPercent`パーセント以上）反転すると折り返します。ラインは直前の肩を上回ると太線（陽線、`color`で描画）に、直前の腰を下回ると細線（陰線、`downColor`）になります。最後のラインは形成中です。

### ポイント・アンド・フィギュア

価格は`boxSize`のボックスに分割されます。終値が上方向に新しいボックスを埋めていく間はXの列が伸び、`reversal`ボックス分の反転でOの列が始まります。逆の場合も同様です。Xの列は`color`、Oの列は`downColor`を使います。最後の列は形成中です。

### 制約

- 1つのチャートに置ける変換シリーズは最大1つで、出来高以外の他のシリーズを並べることはできません。2つ目の変換シリーズや他のシリーズを追加すると例外がthrowされ、すでにシリーズがあるチャートに変換シリーズを追加した場合も同様です。元の足を表示するには別のチャートを使い、必要に応じて[同期](sync.md)で両者を連動させてください。
- 各要素の時刻は、その要素を完成させた入力足の時刻です（それが増加にならない場合は、直前の要素の1ミリ秒後）。`timeAtIndex`と時間軸にはこれらの時刻が表示されます。
- 各足の入力出来高は、その後に形成される要素に加算されます。入力出来高がない場合、出来高は空になります。
- 1本の入力足から生成できる要素は最大65536個です。値動きに比べてボックスやレンジが小さすぎて1本の足がこれを超える場合、呼び出しは例外をthrowし、シリーズはクリアされます。
- ギャップの足（すべての価格が`NaN`）はスキップされます。

## 2つのシリーズ間のバンド

`addBand(upper, lower, options?)`は、同じペインにある2つの単一値シリーズ（ライン、エリア、ベースライン、ヒストグラム、ポイント）の間の領域を塗りつぶします。`upper`が`lower`以上の部分には`fill`が、下回る部分には`fillCrossed`が使われます。バンドはペインのシリーズの背後（出来高の上）に描画されます。

```ts
const upper = chart.addSeries('line', { color: '#26a69a' });
const lower = chart.addSeries('line', { color: '#ef5350' });
upper.setData({ time, value: highs });
lower.setData({ time, value: lows });

const band = chart.addBand(upper, lower, {
  fill: 'rgba(38, 166, 154, 0.15)',
  fillCrossed: 'rgba(239, 83, 80, 0.15)',
});

chart.setBandOptions(band, { fillCrossed: null }); // keys left out keep their value
console.log(chart.bandOptions(band));               // { fill: 'rgba(...)', fillCrossed: null }
chart.removeBand(band);
```

`addBand`はバンドのidを返します。`fill`が`null`（または未指定）の場合は上側シリーズの色を不透明度15%にしたものになり、`fillCrossed`が`null`の場合は`fill`と同じになります。`bandOptions`は未知のidに対して`null`を返します。バンドは、どちらかのシリーズが削除されると自動的に削除されます。2つのOHLCシリーズ、異なるペインのシリーズ、同じシリーズを2回指定した場合は例外がthrowされます。

バンドを持つインジケーター（ボリンジャーバンド、ケルトナーチャネル、一目均衡表）は自動的にバンドを作成します。[インジケーター](indicators.md)を参照してください。

## シリーズの削除

```ts
line.remove();
```

`remove()`はシリーズ、そのデータ、マーカー、プライスライン、およびそのシリーズを使うバンドを削除します。OHLCシリーズを削除すると、その出来高シリーズも削除されます。その後、ハンドルのすべてのメソッドは例外（`'the series was removed'`）をthrowします。データを持つ最後のシリーズを削除すると、時間軸はリセットされます。

インジケーターの出力は個別に削除できません。そのうちの1つに対して`remove()`を呼ぶと例外がthrowされます。代わりにインジケーター自体を削除してください（`indicator.remove()`）。

ペイン全体をそのすべてのシリーズとともに削除するには、`chart.removePane(id)`を使います（[価格スケールとペイン](price-scale.md)を参照）。
