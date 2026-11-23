<!-- source: 9914dfe76f -->
# 価格スケールとペイン

チャートは、時間軸を共有するペインを縦に積み重ねたものです。各ペインは右側に独自の価格スケールを持ちます。この章では、ペインの作成とサイズ設定、ペインへのシリーズの配置、価格スケールのオプションを扱います。

## ペイン

新しいチャートにはペインが1つ、メインペインがあります。`chart.mainPane`は最上段のペインのidで、ペインを省略可能な引数として取るすべてのメソッドのデフォルトです。

```ts
const main = chart.mainPane;
const volumePane = chart.addPane(0.25); // below the others, a quarter of the main pane's height
const oscPane = chart.addPane(0.3);

console.log(chart.panes());             // [main, volumePane, oscPane], top to bottom
```

| メソッド | 動作 |
|---|---|
| `addPane(stretch?)` | 他のペインの下にペインを追加し、そのidを返します。`stretch`は他のペインに対する相対的な高さ（デフォルト1）で、0より大きい有限の数でなければなりません。そうでない場合`addPane`は例外を投げます。 |
| `removePane(pane)` | ペインを、そのシリーズ、描画、レイヤープラグインとともに削除します。最後のペインは削除できません。 |
| `panes()` | 上から下へのペインid。 |
| `mainPane` | 最上段のペインのid。 |
| `setPaneStretch(pane, stretch)` | ペインの相対的な高さを変更します。`stretch`は正の有限の数でなければなりません。 |
| `paneStretch(pane)` | ペインの相対的な高さ。不明なidでは`undefined`。 |

高さはstretch係数に比例します。係数が1、0.25、0.3なら、各ペインは利用可能な高さの1/1.55、0.25/1.55、0.3/1.55を得ます。ユーザーは2つのペインの間の区切り線をドラッグでき、そうするとその2つのペインの係数が変わります。

削除されたペインのidは再利用されません。最上段のペインを削除すると、次のペインがメインペインになります。`paneStretch`以外のメソッドは、不明なペインidを渡すと例外を投げます。

```ts
chart.setPaneStretch(oscPane, 0.5);
console.log(chart.paneStretch(oscPane)); // 0.5

chart.removePane(oscPane);               // its series go with it
```

ペインのレイアウト（ペインの数、stretch係数、価格スケールの設定）は保存される状態に含まれます。[状態の保存](state.md)を参照してください。

## ペインへのシリーズの配置

シリーズを追加するすべてのメソッドは、最後の引数としてペインを取ります。省略するとシリーズはメインペインに置かれます。`series.pane`でシリーズの位置がわかります。

```ts
const candles = chart.addCandlestickSeries();          // main pane
const sma = chart.addSeries('line', { color: '#2962ff' });
const pane = chart.addPane(0.3);
const rsi = chart.addSeries('line', { color: '#7e57c2' }, pane);
rsi.setData(rsiValues);

console.log(rsi.pane === pane); // true
```

シリーズはそのペインに留まります。移動するには、削除してから別のペインに追加し直してください。インジケーターは配置の引数でペインを選びます。[インジケーター](indicators.md)を参照してください。

ペインのすべてのシリーズはその価格スケールを共有し、まとめて自動スケールされます。

### 出来高の配置

`addVolumeSeries(source, pane?)`は、ペイン上に他に何があるかによって動作が変わります。

- **価格シリーズのあるペイン上**（デフォルト：メインペインで、ローソク足の隣）では、出来高は独自の別スケールを持ち、ペインの下部4分の1を占めます。価格の範囲には影響せず、価格軸は価格を表示し続けます。
- **専用のペイン上**では、出来高はペインの価格スケールを使い、ペイン全体を埋めます。バーは下端から立ち上がり（下マージン0）、軸のラベルにはK、M、Bの接尾辞が付きます（`1.25M`）。

```ts
// Volume under the candles, in the same pane.
chart.addVolumeSeries(candles);

// Or in a pane of its own below.
const volPane = chart.addPane(0.25);
chart.addVolumeSeries(candles, volPane);
```

## 価格スケール

`applyPriceScaleOptions(options, pane?)`はペイン（デフォルト：メインペイン）の価格スケールを変更します。省略したキーは値を保ちます。すべてが適用されるか何も適用されないかのどちらかで、1つでも無効な値があれば例外を投げ、スケールはすべてのオプションを元のまま保ちます。`priceScale(pane?)`はすべてのオプションを返します。

```ts
chart.applyPriceScaleOptions({ mode: 'logarithmic' });
chart.applyPriceScaleOptions({ margins: { top: 0.2, bottom: 0.1 } }, pane);

const s = chart.priceScale();
// { mode: 'logarithmic', autoScale: true, inverted: false,
//   margins: { top: 0.1, bottom: 0.1 }, range: { low: 98.2, high: 112.7 } }
```

| オプション | 型 | デフォルト | 意味 |
|---|---|---|---|
| `mode` | `'normal' \| 'logarithmic' \| 'percentage' \| 'indexedTo100'` | `'normal'` | 価格を高さに対応付ける方法。[モード](#modes)を参照 |
| `autoScale` | `boolean` | `true` | 範囲が表示中のデータに追従するかどうか |
| `inverted` | `boolean` | `false` | 高い価格を下に描画する |
| `margins` | `{ top, bottom }` | `{ top: 0.1, bottom: 0.1 }` | データの上下の空白。ペインの高さに対する割合 |
| `range` | `{ low, high }` | データに追従 | 表示する価格の範囲（マージンを除く）。設定すると自動スケールがオフになります。 |

`priceScale().range`は、自動スケール中も含め、常に現在表示されている範囲です。

オプションは`mode`、`margins`、`inverted`、`range`、`autoScale`の順に適用されます。無効な値は例外を投げますが、この順序でそれより前のオプションはすでに適用されています。

### モード

| モード | 軸に表示されるもの |
|---|---|
| `'normal'` | 線形スケールの価格 |
| `'logarithmic'` | 対数スケールの価格。等しい距離は等しい比率を表します。価格が大きく変動した長期の履歴に便利です。 |
| `'percentage'` | 最初に表示されている値からの変化率：(p − base) / \|base\| × 100。小数2桁と`%`で表示されます |
| `'indexedTo100'` | 最初に表示されている値を100とした指数：p / \|base\| × 100。小数2桁で表示されます |

`'percentage'`と`'indexedTo100'`のモードでは、ペインの各シリーズがそれぞれ自分の最初に表示されている値（最初に表示されている足の終値、または値シリーズの最初に表示されている値）で正規化されます。これにより、価格水準が大きく異なるシリーズを1つのスケールで比較できます。ユーザーがパンすると、基準は最初に表示されている足とともに移動します。範囲、軸ラベル、クロスヘアの価格はパーセントまたは指数ポイントで表されます。

価格のモード（`'normal'`、`'logarithmic'`）と相対的なモード（`'percentage'`、`'indexedTo100'`）を切り替えると、以前の範囲は別の単位なので、自動スケールがオンになります。

### 自動スケールと固定範囲

デフォルトでは、範囲はペイン上のすべてのシリーズの表示中のデータの高値と安値（にマージンを加えたもの）に追従し、パン、ズーム、新しいデータの受信に合わせて更新されます。`range`を設定すると固定されます。

```ts
chart.applyPriceScaleOptions({ range: { low: 90, high: 110 } }); // autoScale is now false
chart.applyPriceScaleOptions({ autoScale: true });               // follow the data again
```

`low`は`high`より小さく、どちらも有限でなければなりません。対数モードでは`low`は0より大きくなければなりません。相対的なモードでは、範囲はパーセントまたは指数ポイントです。

`range`は自動スケールをオフにするので、`{ range, autoScale: true }`は矛盾しており例外を投げます。`{ range, autoScale: false }`は問題ありません。

ユーザーも範囲を変更できます。価格軸のドラッグや価格軸上でのホイール操作（またはプロット内でのAlt + ホイール）で価格がスケールされ、自動スケールはオフになります。価格軸をダブルクリックするとオンに戻ります。[ジェスチャーとキーボード](interaction.md#gestures)を参照してください。

### マージン

`margins`は、データをペインの上端と下端から離します。どちらの値も0以上1未満で、合計は1未満でなければなりません。両方のキーが必須です。

```ts
// Leave the top 30 % free for a legend.
chart.applyPriceScaleOptions({ margins: { top: 0.3, bottom: 0.05 } });
```

### 反転

`inverted: true`はスケールを反転し、高い価格が下に描画されます。債券の利回りを価格と対比させる場合や、逆数の通貨ペアに使われることがあります。

```ts
chart.applyPriceScaleOptions({ inverted: true });
```

## 例：2つの銘柄の比較

1つのペイン上の2つのラインシリーズをパーセントモードにすると、表示の左端からの相対的なパフォーマンスがわかります。

```ts
const a = chart.addSeries('line', { color: '#2962ff' });
const b = chart.addSeries('line', { color: '#ff6d00' });
a.setData({ time, value: closesA }); // around 180
b.setData({ time, value: closesB }); // around 0.95

chart.applyPriceScaleOptions({ mode: 'percentage' });
```

## 例：3ペインのレイアウト

メインペインに移動平均付きのローソク足、その下に出来高、最下段にオシレーターを置きます。

```ts
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const volPane = chart.addPane(0.2);
chart.addVolumeSeries(candles, volPane);

const oscPane = chart.addPane(0.25);
const osc = chart.addSeries('histogram', { color: '#26a69a' }, oscPane);
osc.setData({ time: bars.time, value: oscillator });

// Fixed range for the oscillator, a little room at the top of the main pane.
chart.applyPriceScaleOptions({ range: { low: -1, high: 1 } }, oscPane);
chart.applyPriceScaleOptions({ margins: { top: 0.15, bottom: 0.1 } });
```
