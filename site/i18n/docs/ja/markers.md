<!-- source: 610fb1bc72 -->
# マーカーと価格ライン

マーカーは、足に付ける小さな図形で、テキストも付けられます。売買シグナルやニュースなどに使います。マーカーには画像（ウォレットやアカウントのアバター）や`DB`のような短いバッジも表示でき、混み合った足ではマーカーを「+k」バッジにまとめられます。価格ラインは固定価格の水平線で、注文、ストップ、ターゲットなどに使います。どちらもシリーズに属します。各シリーズには最新値ラベルと最新値ラインもあり、シリーズのオプションでオン・オフを切り替えます。

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
| `id` | `number` | `0` | アプリケーション側の識別子。0〜2^53−1の整数で、0はなしを意味します。クリックとホバーのイベントで通知されます。 |
| `position` | `'aboveBar' \| 'belowBar' \| 'atPrice'` | `'aboveBar'` | 高値の上、安値の下、または`price`を中心とする位置。単一値のシリーズでは値の上または下。 |
| `price` | `number` | なし | `'atPrice'`では必須で、有限の値。他の位置では指定できません。シリーズの価格単位で指定します。 |
| `shape` | `'circle' \| 'square' \| 'arrowUp' \| 'arrowDown'` | `'circle'` | 塗りつぶしのアイコン |
| `color` | `Color \| null` | `null` | アイコンとテキストの色。`null`ではシリーズのメインカラー（ローソク足とバーでは陽線の色）を使います |
| `text` | `string` | `''` | アイコンの隣、足と反対側に表示するテキスト。なしの場合は空 |
| `size` | `number \| null` | `null` | 直径（正方形では一辺）、4〜64 CSS px。`null`では10 |
| `icon` | `string \| null` | `null` | `chart.registerIcon`で追加した画像の名前。円または正方形の中に描画されます |
| `label` | `string` | `''` | 円または正方形の中に描く最大8文字のテキスト（`'DB'`など）。`icon`とは併用できません |
| `border` | `Color \| null` | `null` | 図形の縁の内側に描くリング。`null`ではなし |
| `borderWidth` | `number` | `2` | リングの幅（CSS px） |

不明な`position`や`shape`、`time`の欠落や整数でない`time`、無効な色、4〜64の範囲外の`size`、8文字を超える`label`、矢印への`icon`や`label`の指定、または1つのマーカーへの両方の指定は例外を投げ、マーカーは元のままです。`id`が安全に表現できる非負整数の範囲外の場合、`atPrice`で`price`が欠落しているか有限でない場合、他の位置に`price`を指定した場合も例外を投げます。

### マーカーの読み出し

`series.markers()`は、すべてのフィールドを埋めたマーカーを時刻順にソートして返します（デフォルトを使う箇所では`color`は`null`）。ソートは安定で、同じ時刻のマーカーは渡した順序を保ちます。

このリストにおけるマーカーのインデックスが、`click`イベントが`target.index`として通知する値です（[クロスヘアとイベント](crosshair-events.md)を参照）。リストはソートされているため、このインデックスは`setMarkers`に渡した配列での位置と異なる場合があります。

### マーカーに独自のデータを持たせる

各マーカーに独自の`id`を付け、同じオブジェクトにアプリケーションのデータを追加できます。JavaScriptの`click`と`markerHover`イベントは、独自のフィールドも含めた元のオブジェクトを`marker`として返します。

```ts
import type { Marker } from '@ferrochart/core';

const trades: (Marker & { trade: { wallet: string; quantity: number } })[] = [
  { id: 101, time: Date.UTC(2024, 0, 15), position: 'belowBar', text: 'Buy',
    trade: { wallet: 'wallet-a', quantity: 2 } },
];
candles.setMarkers(trades);

chart.on('click', ({ target }) => {
  if (target?.type !== 'marker' || target.series.id !== candles.id) return;
  const marker = target.marker as (typeof trades)[number];
  console.log(target.id, marker.trade.wallet, marker.trade.quantity);
});
chart.on('markerHover', (e) => {
  if (!e || e.series.id !== candles.id || e.overflow > 0) return;
  const marker = e.marker as (typeof trades)[number];
  console.log(e.id, marker.trade.wallet);
});
```

上の型アサーションは、このシリーズのマーカーに追加した独自のフィールドを表しています。`index`は時刻順にソートした後の位置であり、アプリケーションの識別子ではありません。イベントから自分のデータを取り出すには、`id`または元の`marker`を使ってください。「+k」バッジの`id`と`marker`は、独立したバッジオブジェクトではなく、描画されなかった最初のマーカーを指します。`markers()`は正規化されたマーカーのフィールドを返し、元のオブジェクトは返しません。`setMarkers`でマーカーを置き換える際に独自のフィールドを保持する必要がある場合は、元のリストをアプリケーション側で保持してください。

### 配置とスナップ

- マーカーは、時刻が`time`と等しい足に描画されます。2本の足の間の時刻は**次の**足にスナップし、最後の足より後の時刻は最後の足にスナップします。最初の足より前の時刻は最初の足にスナップします。
- その足でシリーズに値がない（欠損）場合、マーカーは描画されません。
- マーカーはシリーズの`shift`オプションに従います。
- アイコンの大きさは`size` CSS px（デフォルトは10）です。足の上または下に置くマーカーは、足から4 CSS px離れています。
- 同じ足の同じ側にある、足の上または下に置く複数のマーカーは、`markers()`の順で足から離れる方向に積み重なります。テキスト付きのマーカーは、アイコンとテキスト1行分の場所を取ります。
- テキストは足の中央に揃えられ、`aboveBar`ではアイコンの上、`belowBar`ではアイコンの下に、マーカーの色で表示されます。
- `'atPrice'`のマーカーは有限の`price`を中心に置かれ、テキストは右側に表示されます。時刻のスナップは行われますが、積み重ねやクラスタリングの対象にはなりません。同じ時刻と価格に複数のマーカーを置くと、重なる場合があります。
- マーカーはプロット領域でクリップされます。

```ts
// Two signals on the same bar: the first sits next to the low, the second below it.
candles.setMarkers([
  { time: t, position: 'belowBar', shape: 'arrowUp', text: 'Entry' },
  { time: t, position: 'belowBar', shape: 'circle', text: 'Alert' },
]);
```

特定の価格で起きたイベントを表示するには：

```ts
candles.setMarkers([
  { id: 102, time: t, position: 'atPrice', price: 101.25, shape: 'circle', text: 'Fill' },
]);
```

### 画像、バッジ、枠線

画像は一度だけ名前を付けて登録し、その名前をいくつのマーカーでも使えます。ソースにはURL（取得されます。別オリジンの画像にはCORSヘッダーが必要です）、`HTMLImageElement`、`ImageBitmap`、`ImageData`、またはcanvasを指定できます。チャートは64 × 64ピクセルに収まるよう縮小したコピーを保持します。

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

- 画像はマーカーの色の上に、円（丸くクリップ）または正方形いっぱいに描画されるため、透明な画像では背後の色が見えます。矢印には画像もラベルも付けられません。
- `registerIcon`はPromiseを返します。名前を使う前に完了を待ってください。登録されていない名前で`setMarkers`を呼ぶと例外を投げます。画像を読み込めない、または読み取れない場合（取得の失敗、CORSヘッダーのない別オリジンの画像）はPromiseがrejectされます。そのマーカーには`label`や通常の図形で代替してください。
- 同じ名前で再登録すると、その名前を使うすべてのマーカーの画像が置き換わります。`chart.unregisterIcon(name)`で削除できます（登録されていたかどうかを返します）。その画像を使っていたマーカーは図形のみを描画し、`markers()`はその`icon`を`null`として返します。1つのチャートが保持できる画像は最大1024個です。
- `label`はマーカーの色と対照的な色で中央に、収まる大きさで描画されます。ラテン文字では直径の約0.45、中国語・日本語・韓国語の文字では約0.6で、テキストが幅広い場合はさらに小さくなります。
- `border`のリングは縁の内側に描かれるため、マーカーの大きさは変わりません。たとえば買いには緑、売りには赤のリングを使えます。

### 混み合った足をまとめる

1本の足に多数のマーカーが集まる場合（同じ1分間に十数人のトレーダーが買うなど）は、シリーズに`markerStackLimit`を設定します。足の各側で使う場所はその数までとなり、最後の場所には描画されなかったk個のマーカーを表す「+k」が表示されます。

```ts
candles.applyOptions({ markerStackLimit: 4 });   // 3 markers, then "+k"
candles.applyOptions({ markerStackLimit: null }); // no limit (the default)
```

バッジはシリーズのメインカラーの円で、置き換えるマーカーと同じ大きさです。クリックすると、ターゲットが`{ type: 'markerOverflow', series, first, count, id, marker, bars }`の`click`が送られます。`first`は描画されなかった最初のマーカーのソート後のインデックス、`count`はその側で描画されなかった数で、`id`/`marker`は最初のマーカーを識別します。`bars`は最初と最後の論理インデックスを示し、1本の足でまとめた場合は両者が同じです。ソート済みリストには反対側のマーカーが間に入る場合があるため、`count`はそのリストの連続した部分を表すものではありません。

### 複数の足にまたがるクラスタリング

`markerCluster`にCSSピクセル単位の正の有限距離を設定すると、近くの足のマーカーをまとめられます。`null`（デフォルト）ではクラスタリングが無効になります。

```ts
candles.applyOptions({ markerCluster: 28, markerStackLimit: 4 });
// Later, keep per-bar folding but turn off cross-bar clustering:
candles.applyOptions({ markerCluster: null });
```

各区画には`ceil(markerCluster / bar spacing)`個の論理インデックスが入ります（最小1個）。区画は論理インデックスに揃うため、パンしてもマーカーのグループは変わりません。ズームすると区画の大きさが変わる場合があります。区画の各側で2本以上の足にマーカーがある場合、その側のマーカーが最も多い足に、最初のマーカーと「+k」バッジだけを表示します。`atPrice`マーカーは対象外です。

このようなグループでは、クラスタリングが`markerStackLimit`より優先されます。それ以外では、`markerStackLimit`が引き続き1本の足でのまとめ方を制御します。バッジのクリックは`markerOverflow`ターゲットを持ち、ホバーでは`overflow > 0`となります。イベントの`bars: { from, to }`は、バッジが表す最初と最後の論理インデックスを両端を含む範囲で示します。詳細の一覧を作るには、この範囲、`marker`が置かれている側、アプリケーション側のマーカーデータを使ってください。`first`/`index`と個数だけでは、メンバーの連続したリストを取り出せません。[論理インデックス](data.md#logical-indices)と[クリックイベント](crosshair-events.md#click)を参照してください。

### マーカーのホバー

`markerHover`イベントは、ポインターがマーカーまたは「+k」バッジに乗ったときに発生し、離れたときには`null`で発生します。マーカーのインデックスと、CSS px単位の中心と直径を持つので、ツールチップに必要な情報がそろいます。

```ts
chart.on('markerHover', (e) => {
  if (!e) return hideTooltip();
  if (e.overflow > 0) {
    showTooltip(e.x, e.y - e.size / 2, `${e.overflow} more · bars ${e.bars.from}–${e.bars.to}`);
  } else {
    showTooltip(e.x, e.y - e.size / 2, e.marker.text ?? '');
  }
});
```

`index`は`series.markers()`における位置です。バッジでは描画されなかった最初のマーカーを指し、`overflow`はkです（マーカーでは0）。`id`と`marker`はそのマーカーを識別し、`bars`はマーカーまたはバッジが表す、両端を含む論理インデックスの範囲です。ホバー中のマーカーはフレームの描画時に判定されるため、イベントは1フレームにつき最大1回です。イントロアニメーションの再生中は、どのマーカーもホバー中とはみなされません。[クロスヘアとイベント](crosshair-events.md)を参照してください。

### 中国語、日本語、韓国語、絵文字

マーカーのテキストとラベルには任意の文字体系を使えます。同梱フォントはラテン文字、数字、一般的な記号のみに対応しており、それ以外の文字は次のフォントから描画されます。

- **Web**：ブラウザー自身のフォント（システムのCJKフォントとカラー絵文字）。設定は不要で、パッケージにも何も追加されません。
- **デスクトップ、Python、C**：追加したフォントを順に検索します。winitでは`WindowOptions { fonts, .. }`、Pythonでは`fc.Chart(fonts=[...])`、Cでは`ferrochart_add_font`です。システムフォントは走査しません。通常存在するフォントは`/System/Library/Fonts/Hiragino Sans GB.ttc`（macOS）、`C:/Windows/Fonts/msyh.ttc`（Windows）、`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`（Linux、Noto CJKパッケージ導入時）です。あるいはNoto Sans SC/JP/KRをアプリケーションに同梱してください。

どのフォントにもない文字は`□`として描画されます。ネイティブではカラー絵文字フォントには対応しておらず、アウトライン絵文字フォント（Noto Emoji）はテキストの色で描画されます。テキストは書記素クラスタ単位でレイアウトされます。Web では、肌の色付きの絵文字、家族などの ZWJ シーケンス、国旗、結合アクセント付きの文字が、ブラウザによって1つのグリフとして計測・描画されます。ネイティブではこれらのシーケンスは1文字ずつ描画されます（フォントのシェーピングは行いません）。シェーピングと右から左へのレイアウトには対応していないため、アラビア語やヘブライ語は正しく表示されません。

### Python、C、Rust

各バインディングはそれぞれの命名規則に従います。

```python
red = chart.add_icon(avatar)          # a (height, width, 4) or (height, width, 3) uint8 array
candles.set_markers([
    {"time": t, "icon": red, "size": 24, "border": "#2fbf71"},
    {"time": t, "position": "below_bar", "label": "DB", "color": "#2962ff"},
])
candles.apply_options(marker_stack_limit=4, marker_cluster=28)
chart.on("marker_hover", lambda e: print(e))   # None when the pointer leaves
```

Pythonのマーカー辞書は`id`と、`price`を伴う`position="at_price"`を受け取ります。イベントは`id`とバッジの`bars`を通知します。元のオブジェクトを`marker`として取り出せるのはJavaScriptの機能です。ネイティブPythonでは、idと独自のオブジェクトの対応をアプリケーション側で保持してください。Rustでは`Marker::id`、`Marker::price`、`MarkerPosition::AtPrice`を使い、`SeriesOptions::marker_cluster`でクラスタリングを制御します。Cでは`FerroChartMarker.id`、`.price`、`FERROCHART_MARKER_AT_PRICE`を使い、シリーズのオプションには`marker_cluster`を指定します。

Pythonでは、`add_icon`は整数のidを返し、`remove_icon(id)`で画像を削除します。Cでは、`ferrochart_add_icon`と`ferrochart_set_markers`が同じフィールドを受け取ります（[C、C++、C#](native.md)を参照）。Rustでは、`Chart::add_icon`が`IconImage`（ストレートRGBA、最大256 × 256）を受け取り、`Marker::icon`用の`IconId`を返します。イベントは`ChartEvent::MarkerHover`、クリックターゲットは`ClickTarget::MarkerOverflow`です。

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
