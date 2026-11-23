<!-- source: b0ef03b731 -->
# マーカーと価格ライン

マーカーは、足に付ける小さなアイコンで、テキストも付けられます。売買シグナルやニュースなどに使います。価格ラインは固定価格の水平線で、注文、ストップ、ターゲットなどに使います。どちらもシリーズに属します。各シリーズには最新値ラベルと最新値ラインもあり、シリーズのオプションでオン・オフを切り替えます。

## マーカー

### マーカーの設定

```ts
candles.setMarkers([
  { time: Date.UTC(2024, 0, 15), position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: Date.UTC(2024, 1, 2), position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
  { time: Date.UTC(2024, 1, 20), shape: 'circle', text: 'Earnings' },
]);
```

`setMarkers`はシリーズのマーカーをすべて置き換えます。1つ追加するには、現在のリストを読み出し、末尾に加えて設定し直します。消去するには空の配列を渡します。

```ts
candles.setMarkers([...candles.markers(), { time: Date.UTC(2024, 2, 1), text: 'Split' }]);
candles.setMarkers([]);
```

### マーカーのフィールド

| フィールド | 型 | デフォルト | 意味 |
|---|---|---|---|
| `time` | `number \| bigint \| Date` | 必須 | UTCミリ秒（整数）または`Date`。`markers()`は数値を返します |
| `position` | `'aboveBar' \| 'belowBar'` | `'aboveBar'` | 足の高値の上、または安値の下。単一値のシリーズでは値の上または下。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 塗りつぶしのアイコン |
| `color` | `Color \| null` | `null` | アイコンとテキストの色。`null`ではシリーズのメインカラー（ローソク足とバーでは陽線の色）を使います |
| `text` | `string` | `''` | アイコンの隣、足と反対側に表示するテキスト。なしの場合は空 |

不明な`position`や`shape`、`time`の欠落や整数でない`time`、無効な色は例外を投げ、マーカーは元のままです。

### マーカーの読み出し

`series.markers()`は、すべてのフィールドを埋めたマーカーを時刻順にソートして返します（デフォルトを使う箇所では`color`は`null`）。ソートは安定で、同じ時刻のマーカーは渡した順序を保ちます。

このリストにおけるマーカーのインデックスが、`click`イベントが`target.index`として通知する値です（[クロスヘアとイベント](crosshair-events.md)を参照）。リストはソートされているため、このインデックスは`setMarkers`に渡した配列での位置と異なる場合があります。

### 配置とスナップ

- マーカーは、時刻が`time`と等しい足に描画されます。2本の足の間の時刻は**次の**足にスナップし、最後の足より後の時刻は最後の足にスナップします。最初の足より前の時刻は最初の足にスナップします。
- その足でシリーズに値がない（欠損）場合、マーカーは描画されません。
- マーカーはシリーズの`shift`オプションに従います。
- アイコンの大きさは10 CSS pxで、足から4 CSS px離れています。
- 同じ足の同じ側にある複数のマーカーは、`markers()`の順で足から離れる方向に積み重なります。テキスト付きのマーカーは、アイコンとテキスト1行分の場所を取ります。
- テキストは足の中央に揃えられ、`aboveBar`ではアイコンの上、`belowBar`ではアイコンの下に、マーカーの色で表示されます。
- マーカーはプロット領域でクリップされます。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

### マーカーを持てるシリーズ

出来高シリーズを除くすべての種類のシリーズです。出来高シリーズで`setMarkers`を呼ぶと例外を投げます。インジケーターの出力にもマーカーを付けられます（`Indicator`の`outputs()`は通常のシリーズです）。

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];
rsi.setMarkers([{ time: t, position: 'aboveBar', shape: 'square', text: 'OB' }]);
```

### 例：シグナルからマーカーを作る

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

## 価格ライン

### 価格ラインの作成

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

`createPriceLine`は`PriceLine`を返します。価格はシリーズの単位なので、パーセントや指数の価格スケールではラインはシリーズとともに動きます（[価格スケール](price-scale.md)を参照）。

### オプション

| オプション | 型 | デフォルト | 意味 |
|---|---|---|---|
| `price` | `number` | `0` | シリーズの単位での価格。有限でなければなりません |
| `color` | `Color \| null` | `null` | ライン、タイトル、軸ラベルの色。`null`ではシリーズのメインカラーを使います |
| `lineWidth` | `number` | `1` | CSS px。0より大きい値 |
| `lineStyle` | `'solid' \| 'dashed' \| 'dotted'` | `'dashed'` | 破線のパターン |
| `axisLabelVisible` | `boolean` | `true` | 価格軸上の価格ラベル |
| `title` | `string` | `''` | プロットの右端でラインの上に表示するテキスト。なしの場合は空 |

軸ラベルはシリーズの価格フォーマット（その`pricePrecision`）を使います。価格軸上のラベルは、重なりそうなときは互いに離して配置されます。表示中の価格範囲の外にある価格ラインは描画されません。

### 変更と削除

```ts
entry.applyOptions({ price: 101.5 });         // only the keys you pass change
console.log(entry.options());                   // every option: { price, color, lineWidth, ... }
stop.applyOptions({ title: 'Stop (moved)', lineStyle: 'dotted' });

stop.remove();
```

`remove()`の後、そのオブジェクトの`options`、`applyOptions`、`remove`を呼ぶと例外を投げます。シリーズを削除すると、その価格ラインも削除されます。

`createPriceLine`は、出来高シリーズ、有限でない価格、オブジェクトでないoptions引数、無効なオプション値に対して例外を投げます。

### 例：ドラッグできる注文ライン

価格ラインは自分では入力を処理しません。チャートのイベントと座標変換を組み合わせるか、ユーザーがドラッグできる`horizontal_line`描画を使ってください（[描画ツール](drawings.md)を参照）。

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

## 最新値ラベルとライン

各シリーズは、最新の値を価格軸上のラベルと、プロットを横切る点線の水平線で表示します。どちらも、最後の足の終値が始値以上（ベースラインシリーズでは基準値以上）なら陽線の色、そうでなければ陰線の色を使います。ライン、エリア、ヒストグラム、ポイントのシリーズはメインカラーを使います。

| シリーズオプション | デフォルト | 意味 |
|---|---|---|
| `lastValueVisible` | `true` | 価格軸上の最新値ラベル |
| `priceLineVisible` | `true` | 最新値の点線 |
| `pricePrecision` | `null` | ラベルの小数桁数。`null`は自動 |

```ts
const line = chart.addSeries('line', { lastValueVisible: true, priceLineVisible: false });
candles.applyOptions({ priceLineVisible: false });
```

出来高シリーズはどちらも描画しません。インジケーターの出力は`priceLineVisible: false`で作成され、最新値ラベルは表示されます。出力ごとにオフにできます。

```ts
for (const s of chart.addIndicator(candles, 'boll').outputs()) {
  s.applyOptions({ lastValueVisible: false });
}
```

シリーズのオプションについては[シリーズ](series.md)で説明しています。
