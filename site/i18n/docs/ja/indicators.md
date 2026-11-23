<!-- source: 32f28d0f34 -->
# インジケーター

FerroChartには30種類のテクニカルインジケーターが組み込まれています。インジケーターはソースシリーズを読み取り、1つ以上の出力シリーズを計算し、ソースの変更に合わせてそれらを最新の状態に保ちます。出力は通常のシリーズなので、他のシリーズと同じようにスタイルを設定したり、値を読み取ったり、マーカーを付けたりできます。

## インジケーターの追加

```ts
const sma = chart.addIndicator(candles, 'sma', { n: 50 });
const boll = chart.addIndicator(candles, 'boll', { n: 20, k: 2 });
const macd = chart.addIndicator(candles, 'macd', { fast: 12, slow: 26, signal: 9 });
const rsi = chart.addIndicator(candles, 'rsi');        // all defaults
```

`chart.addIndicator(source, name, params?, placement?)`は`Indicator`を返します。

| 引数 | 意味 |
|---|---|
| `source` | 計算元のシリーズ。出来高シリーズとカスタムシリーズ以外の任意のシリーズ。 |
| `name` | 下の表にある名前のいずれか（例：`'sma'`、`'stochRsi'`）。`snake_case`の表記（`'stoch_rsi'`、`'volume_ma'`）も受け付けます。 |
| `params` | 名前で指定するパラメーター（camelCase）。省略したものはデフォルト値になります。 |
| `placement` | 出力の配置先。デフォルトは`'default'`。 |

インジケーターは、`addIndicator`が戻る前に既存のすべてのデータに対して一括で計算されます。

### 配置

```ts
type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;
```

| 値 | 出力の配置先 |
|---|---|
| `'default'` | インジケーターの通常の配置先：移動平均とチャネルは`'overlay'`、オシレーターは`'newPane'`、出来高移動平均は`'volume'`（表を参照） |
| `'overlay'` | ソースのペインの、その価格スケール上 |
| `'newPane'` | このインジケーター用に作成される、他のペインの下の新しいペイン |
| `'volume'` | ソースの出来高シリーズのペインの、出来高スケール上。ソースに出来高シリーズがない場合は新しいペイン |
| ペインid | そのペイン（`chart.panes()`、`chart.mainPane`、`chart.addPane()`、`series.pane`から取得） |

```ts
// Two oscillators in one pane.
const rsi = chart.addIndicator(candles, 'rsi');
const pane = rsi.outputs()[0].pane;
chart.addIndicator(candles, 'stochRsi', {}, pane);
```

`'newPane'`で（または出来高シリーズがない状態の`'volume'`で）作成されたペインは、他に何も残っていなければインジケーターと一緒に削除されます。

### ソースシリーズ

OHLCシリーズ（ローソク足、OHLCバー、平均足）は、インジケーターに始値、高値、安値、終値、出来高を提供します。単一値シリーズ（ライン、エリア、ベースライン、ヒストグラム、ポイント）は`open = high = low = close = value`を提供し、出来高はありません。そのため、ラインシリーズのRSIは計算できますが、OBVは計算できません。

出来高を必要とするインジケーター（VWMA、出来高移動平均、VWAP、MFI、OBV、CMF）は、ソースが単一値シリーズの場合や、データに出来高カラムがないOHLCシリーズの場合に例外をthrowします。

ソースに値がない足（他のシリーズにしか存在しない時刻など）はスキップされます。そこでの出力は`NaN`になり、その足はインジケーターの計算期間に数えられません。

### インジケーターのインジケーター

ソースには別のインジケーターの出力も指定できます。たとえば、RSIのペインにRSIの9本SMAを表示するには次のようにします。

```ts
const rsiLine = chart.addIndicator(candles, 'rsi').outputs()[0];
const rsiAvg = chart.addIndicator(rsiLine, 'sma', { n: 9 }, rsiLine.pane);
```

RSIを削除すると、SMAも削除されます。

## 組み込みインジケーター

`indicatorNames()`は、この表の順序で30個の名前を返します。出力は`outputs()`が返す順に記載しています。（histogram）または（points）と記載がない出力はすべてラインです。

| 名前 | インジケーター | パラメーター（デフォルト） | 出力 | デフォルトの配置 |
|---|---|---|---|---|
| `sma` | 単純移動平均 | `n`（20）、`source` | sma | overlay |
| `ema` | 指数平滑移動平均 | `n`（20）、`source` | ema | overlay |
| `wma` | 加重移動平均 | `n`（20）、`source` | wma | overlay |
| `hma` | ハル移動平均 | `n`（20、2以上）、`source` | hma | overlay |
| `vwma` | 出来高加重移動平均 | `n`（20）、`source` | vwma | overlay |
| `dema` | 二重指数移動平均（DEMA） | `n`（20）、`source` | dema | overlay |
| `tema` | 三重指数移動平均（TEMA） | `n`（20）、`source` | tema | overlay |
| `volumeMa` | 出来高の移動平均 | `n`（20） | vma | volume |
| `boll` | ボリンジャーバンド | `n`（20）、`k`（2）、`source` | upper、mid、lower、upper–lower間のバンド | overlay |
| `keltner` | ケルトナーチャネル | `n`（20）、`m`（2）、`atrN`（10） | upper、mid、lower、upper–lower間のバンド | overlay |
| `donchian` | ドンチャンチャネル | `n`（20） | upper、mid、lower | overlay |
| `ichimoku` | 一目均衡表 | `tenkan`（9）、`kijun`（26）、`senkou`（52）、`shift`（26） | 転換線、基準線、先行スパンA、先行スパンB、遅行スパン、先行スパンA–B間のバンド | overlay |
| `sar` | パラボリックSAR | `afStart`（0.02）、`afStep`（0.02）、`afMax`（0.2） | sar（points） | overlay |
| `supertrend` | スーパートレンド | `atrN`（10）、`m`（3） | long、short | overlay |
| `vwap` | 出来高加重平均価格 | `anchor`（`'day'`）、`utcOffsetMs`（0） | vwap | overlay |
| `macd` | MACD | `fast`（12）、`slow`（26）、`signal`（9）、`source` | macd、signal、hist（histogram） | new pane |
| `rsi` | 相対力指数 | `n`（14）、`source` | rsi | new pane |
| `stoch` | ストキャスティクス | `k`（14）、`smoothK`（3）、`d`（3） | %K、%D | new pane |
| `stochRsi` | ストキャスティクスRSI | `rsiN`（14）、`stochN`（14）、`smoothK`（3）、`d`（3）、`source` | %K、%D | new pane |
| `cci` | 商品チャネル指数（CCI） | `n`（20） | cci | new pane |
| `willr` | ウィリアムズ%R | `n`（14） | %R | new pane |
| `mfi` | マネーフローインデックス | `n`（14） | mfi | new pane |
| `adx` | ADXと方向性指数（DI） | `n`（14） | adx、+DI、−DI | new pane |
| `atr` | アベレージ・トゥルー・レンジ | `n`（14） | atr | new pane |
| `obv` | オンバランスボリューム | なし | obv | new pane |
| `cmf` | チャイキン・マネーフロー | `n`（20） | cmf | new pane |
| `roc` | 変化率（%） | `n`（10）、`source` | roc | new pane |
| `momentum` | モメンタム | `n`（10）、`source` | momentum | new pane |
| `trix` | TRIX | `n`（15）、`source` | trix | new pane |
| `aroon` | アルーン | `n`（14） | up、down | new pane |

### パラメーターの値

| パラメーター | 値 |
|---|---|
| 期間（`n`、`fast`、`slow`、`signal`、`stoch`の`k`、`smoothK`、`d`、`rsiN`、`stochN`、`atrN`、`tenkan`、`kijun`、`senkou`、`shift`） | 1以上の整数（`hma`の`n`は2以上） |
| 乗数（`boll`の`k`、`m`） | 0より大きい有限の数値 |
| `afStart`、`afStep`、`afMax` | 0より大きい有限の数値。`afStart`は`afMax`以下 |
| `source` | `'close'`（デフォルト）、`'open'`、`'high'`、`'low'`、`'hl2'` (H+L)/2、`'hlc3'` (H+L+C)/3、`'ohlc4'` (O+H+L+C)/4 |
| `anchor`（`vwap`） | `'day'`（デフォルト）、`'week'`（週は月曜始まり）、`'month'`、または累積を開始するUTC msの時刻 |
| `utcOffsetMs`（`vwap`） | 日・週・月を区切る前にUTCに加算する整数のミリ秒。たとえばニューヨークの標準時なら`-5 * 3600_000` |

```ts
chart.addIndicator(candles, 'ema', { n: 21, source: 'hlc3' });
chart.addIndicator(candles, 'vwap', { anchor: 'week' });
chart.addIndicator(candles, 'vwap', { anchor: Date.UTC(2024, 2, 1) }); // anchored VWAP
chart.addIndicator(candles, 'vwap', { utcOffsetMs: 9 * 3600_000 });   // sessions in UTC+9
```

パラメーター名は表のとおりに正確に書く必要があります。インジケーターが使わないキー（`atr_n`のようなsnake_case表記を含む）を指定すると、受け付けるキーを列挙したエラーがthrowされます。

```ts
chart.addIndicator(candles, 'sma', { length: 20 });
// Error: unknown parameter 'length' for indicator 'sma' (expected: n, source)
```

`addTransformSeries`も同じ方法でパラメーターを検査します。

### 計算式

概要のみ記載します。`P`は選択した`source`の価格です。「ウォームアップ」は、出力が`NaN`になる先頭の足の本数です。

**移動平均とチャネル**

- **SMA**：`P`の直近`n`個の値の平均。ウォームアップ`n − 1`。
- **EMA**：`e = e + α(P − e)`、`α = 2/(n + 1)`。初期値は最初の`n`個の値のSMA。ウォームアップ`n − 1`。
- **WMA**：最新の値から順に重み`n, n−1, …, 1`。ウォームアップ`n − 1`。
- **HMA**：`WMA(2·WMA(P, n/2) − WMA(P, n), √n)`（半分と平方根は切り捨て）。ウォームアップ`n + ⌊√n⌋ − 2`。
- **VWMA**：`n`本の足にわたる`Σ(P·V) / ΣV`（出来高の合計が0のときは`NaN`）。ウォームアップ`n − 1`。
- **DEMA**：`2·e1 − e2`。ここで`e1 = EMA(P, n)`、`e2 = EMA(e1, n)`。ウォームアップ`2(n − 1)`。
- **TEMA**：`3·e1 − 3·e2 + e3`、`e3 = EMA(e2, n)`。ウォームアップ`3(n − 1)`。
- **出来高移動平均**：出来高のSMA。ウォームアップ`n − 1`。
- **BOLL**：mid = `SMA(P, n)`、upperとlower = mid ± `k` × `n`本の足にわたる母標準偏差。ウォームアップ`n − 1`。
- **ケルトナー**：mid = `EMA(close, n)`、upperとlower = mid ± `m` × `ATR(atrN)`。ウォームアップ`max(n − 1, atrN)`。
- **ドンチャン**：upper = `n`本の足の最高値、lower = 最安値、mid = その平均。ウォームアップ`n − 1`。
- **一目均衡表**：転換線と基準線は、それぞれ`tenkan`本と`kijun`本の足にわたる高値・安値レンジの中値です。先行スパンA = (転換線 + 基準線)/2、先行スパンB = `senkou`本の足にわたる中値で、どちらも`shift`本先にずらして描画されます（最後の`shift`個の値は最後の足より右に表示されます）。遅行スパンは終値を`shift`本前にずらして描画したものです。
- **SAR**：加速因子が`afStart`から`afStep`刻みで`afMax`まで増えるパラボリックSAR。TA-Libと同じ方法で計算します。ウォームアップ1。
- **スーパートレンド**：トレンドが続く間は狭まる方向にしか動かないバンド`HL2 ± m·ATR(atrN)`。終値がアクティブなバンドを越えるとトレンドが反転します。`long`（下側のバンド）は上昇トレンドで、`short`（上側のバンド）は下降トレンドで値を持ち、それ以外は`NaN`です。ウォームアップ`atrN`。
- **VWAP**：アンカー以降の`Σ(HLC3·V) / ΣV`。日・週・月ごと（UTCに`utcOffsetMs`を加えた時刻で判定）にリセットされるか、指定した時刻以降に累積します（それ以前は`NaN`）。ウォームアップ0。

**オシレーター**

- **MACD**：macd = `EMA(P, fast) − EMA(P, slow)`、signal = `EMA(macd, signal)`、hist = macd − signal。ウォームアップはmacdが`slow − 1`、signalとhistが`slow + signal − 2`。
- **RSI**：上昇幅と下落幅をワイルダーの平滑化（`r = r + (x − r)/n`）し、`100·gain/(gain + loss)`。両方が0のときは50。ウォームアップ`n`。
- **ストキャスティクス**：raw = `k`本の足にわたる`100·(C − lowest low)/(highest high − lowest low)`（レンジが0のときは50）。%K = `SMA(raw, smoothK)`、%D = `SMA(%K, d)`。ウォームアップは%Kが`k + smoothK − 2`、%Dはさらに`d − 1`。
- **ストキャスティクスRSI**：`RSI(P, rsiN)`に`stochN`本の足でストキャスティクスの式を適用し、ストキャスティクスと同様に平滑化します。ウォームアップは%Kが`rsiN + stochN + smoothK − 2`、%Dはさらに`d − 1`。
- **CCI**：`(TP − SMA(TP, n)) / (0.015 · mean deviation)`、`TP` = HLC3。平均偏差が0のときは0。ウォームアップ`n − 1`。
- **ウィリアムズ%R**：`n`本の足にわたる`−100·(highest high − C)/(highest high − lowest low)`。レンジが0のときは−50。ウォームアップ`n − 1`。
- **MFI**：`n`本の足にわたる`100·positive flow/(positive + negative flow)`。フロー = HLC3 × 出来高で、その符号はHLC3の変化に従います。両方が0のときは50。ウォームアップ`n`。
- **ADX**：+DIと−DIはワイルダー平滑化した方向性の動きとトゥルー・レンジから求めます。adx = `100·|+DI − −DI|/(+DI + −DI)`のワイルダー平均。ウォームアップはDIが`n`、adxが`2n − 1`。
- **ATR**：トゥルー・レンジのワイルダー平均。ウォームアップ`n`。
- **OBV**：出来高の累積値。終値が上昇した足では加算し、下落した足では減算します。ウォームアップ0。
- **CMF**：`n`本の足にわたる`Σ(MFM·V)/ΣV`、`MFM = ((C − L) − (H − C))/(H − L)`。ウォームアップ`n − 1`。
- **ROC**：`100·(P / P[n bars ago] − 1)`。ウォームアップ`n`。
- **モメンタム**：`P − P[n bars ago]`。ウォームアップ`n`。
- **TRIX**：`EMA(EMA(EMA(P, n), n), n)`の前の足からの変化率（%）。ウォームアップ`3(n − 1) + 1`。
- **アルーン**：直近`n + 1`本の足にわたる`100·(n − bars since the highest high)/n`（up）と、最安値についての同じ式（down）。ウォームアップ`n`。

## 出力のスタイル

`indicator.outputs()`は出力シリーズを返します。他のシリーズと同様に`applyOptions`でスタイルを設定します（[シリーズ](series.md)を参照）。

```ts
const [macdLine, signalLine, hist] = macd.outputs();
macdLine.applyOptions({ color: '#2962ff', lineWidth: 2 });
signalLine.applyOptions({ color: '#ff9800', lineStyle: 'dashed' });
hist.applyOptions({ color: '#787b86' });

const [upper, mid, lower] = boll.outputs();
mid.applyOptions({ lineStyle: 'dotted' });
```

出力は、ラインの太さ1 CSS px（ポイントは点の直径3）、出力ごとに固定の色、`priceLineVisible: false`、最終値ラベルを表示、という設定で作成されます。`outputs()`は呼び出すたびに同じシリーズへの新しいハンドルを返します。比較には`id`を使ってください。

出力を単独で削除することはできません。出力に対して`series.remove()`を呼ぶと例外がthrowされます。代わりにインジケーターを削除してください。

### バンド

BOLLとケルトナーは上側と下側のラインの間を塗りつぶし、一目均衡表は先行スパンAと先行スパンBの間の雲を塗りつぶします（AがBより上なら緑、下なら赤）。`indicator.bands()`はこれらの塗りのidを返します。色は`chart.setBandOptions`で変更します。

```ts
const [band] = boll.bands();
chart.setBandOptions(band, { fill: 'rgba(41, 98, 255, 0.08)' });

const ichimoku = chart.addIndicator(candles, 'ichimoku');
const [cloud] = ichimoku.bands();
chart.setBandOptions(cloud, { fill: 'rgba(38, 166, 154, 0.25)', fillCrossed: 'rgba(239, 83, 80, 0.25)' });
console.log(chart.bandOptions(cloud)); // { fill, fillCrossed }
```

| バンドのオプション | 意味 |
|---|---|
| `fill` | 上側の出力が下側の出力以上の部分 |
| `fillCrossed` | 下回る部分 |

`null`を指定するとデフォルトに戻ります。BOLLとケルトナーのデフォルトは上側ラインの色を不透明度15%にしたものです（`fillCrossed`は`fill`と同じ）。塗りを非表示にするには`'transparent'`を渡します。その他のインジケーターにはバンドがなく、`bands()`は空の配列を返します。

## 削除

```ts
macd.remove();
```

`remove()`は、インジケーター、その出力シリーズ、それらのプライスラインとマーカー、バンド、出力から計算されたすべてのインジケーター、そしてインジケーターが作成したペイン（空になった場合）を削除します。その後、そのオブジェクトに対する`outputs()`、`bands()`、`remove()`は例外をthrowします。

ソースシリーズを削除すると、そのインジケーターも削除されます。

## パラメーターの変更

パラメーターはその場で変更できません。インジケーターを削除して、もう一度追加してください。カスタムスタイルを保持したい場合は、先にそれを読み取っておきます。

```ts
import type { FerroChart, Indicator, Series, Placement } from '@ferrochart/core';

function replaceIndicator(
  chart: FerroChart,
  old: Indicator,
  source: Series,
  name: string,
  params: Record<string, number | string>,
  placement?: Placement,
): Indicator {
  // `shift` is set by the indicator (Ichimoku), so it is not copied.
  const styles = old.outputs().map((s) => {
    const { shift, ...style } = s.options();
    return style;
  });
  old.remove();
  const next = chart.addIndicator(source, name, params, placement);
  next.outputs().forEach((s, i) => {
    if (styles[i]) s.applyOptions(styles[i]);
  });
  return next;
}

let sma = chart.addIndicator(candles, 'sma', { n: 20 });
sma = replaceIndicator(chart, sma, candles, 'sma', { n: 50 });
```

オシレーターを削除すると、それが作成したペインも（他に何もなければ）削除されるため、置き換えたインジケーターは最下部の新しいペインに配置されます。オシレーターを固定の位置に置きたい場合は、`chart.addPane()`で自分でペインを作成し、そのidを配置先として渡してください。自分で作成したペインはインジケーターと一緒に削除されません。

## エラー

次の場合、`addIndicator`は`Error`をthrowし、何も変更しません。

| 問題 | メッセージ（例） |
|---|---|
| 未知の名前 | `unknown indicator 'smaa'` |
| 0以上の整数でない期間 | `'n' must be a non-negative integer` |
| 期間が0 | `invalid argument: periods must be at least 1` |
| `n`が2未満の`hma` | `invalid argument: the period is too short` |
| 0以下または有限でない乗数 | `invalid argument: multipliers must be finite and > 0` |
| `afStart`が`afMax`より大きい | `invalid argument: af_start must not exceed af_max` |
| 数値が必要な場所に数値以外 | `'k' must be a finite number` |
| 未知の`source` | `unknown source 'median'` |
| 無効なVWAPの`anchor` | `'anchor' must be 'day', 'week', 'month' or a time in ms` |
| 整数でない`utcOffsetMs` | `'utcOffsetMs' must be an integer` |
| 無効な`placement` | `placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id` |
| 未知のペインid | `unknown id` |
| ソースに出来高シリーズを指定 | `invalid argument: volume series have no data of their own; use the source series` |
| 出来高のないソースに出来高系インジケーター | `invalid argument: the indicator needs volume` |

```ts
try {
  chart.addIndicator(candles, 'rsi', { n: 0 });
} catch (e) {
  console.error((e as Error).message); // invalid argument: periods must be at least 1
}
```

## リアルタイム更新

インジケーターは自動的にソースに追従します。

- 最後の足と同じ時刻で`series.update(bar)`を呼ぶと、すべての出力の最後の値が更新されます。それより後の時刻なら値が1つ追加されます。計算されるのは新しい足または変更された足だけで、シリーズ全体ではありません。
- `series.setData(...)`を呼ぶと、新しいデータに対してインジケーターが再計算されます。

インクリメンタル更新の結果は、シリーズ全体を再計算した結果と同じです。出力はソースと同じフレームで変化します。

```ts
const rsi = chart.addIndicator(candles, 'rsi').outputs()[0];

feed.onTick((bar) => {
  candles.update(bar);
  const last = chart.dataRange();
  if (last) console.log('RSI', rsi.dataAt(last.to)?.value);
});
```

## 名前の一覧

```ts
import init, { indicatorNames } from '@ferrochart/core';

await init(); // not needed once a chart has been created
console.log(indicatorNames()); // ['sma', 'ema', ..., 'aroon']
```
