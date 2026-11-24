<!-- source: bc2d94fef4 -->
# クロスヘアとイベント

チャートは、その上で起きたことをイベントで通知します。クロスヘアの移動、クリック、表示範囲の変化、描画の作成や編集などです。この章ではすべてのイベントについて、ペイロードと発火タイミングを一覧にし、よくある使い方を2つ紹介します。クロスヘアに追従する凡例と、マーカーやシリーズのクリックへの応答です。

## クロスヘアのモード

クロスヘアはプロット上のポインターに追従します。垂直線はすべてのペインを横切り、水平線と価格ラベルはポインターの下にあるペインに属します。時刻ラベルは時間軸に表示されます。

| モード | 動作 |
|---|---|
| `'normal'`（デフォルト） | xは最も近い足の中心にスナップし、yはポインターに追従します |
| `'magnet'` | `'normal'`と同じですが、yはその足（ペイン内の最初の価格シリーズ）の始値・高値・安値・終値のうち最も近い値にスナップします |
| `'hidden'` | クロスヘアを描画せず、`crosshairMove`は`null`を通知します |

```ts
chart.setCrosshairMode('magnet');
console.log(chart.crosshairMode); // 'magnet'
```

不明なモードを指定すると例外が投げられます。モードは保存される状態に含まれます（`saveState().crosshairMode`、[状態の保存](state.md)を参照）。

マグネットモードは描画の配置中にも適用され、アンカーは最も近い始値・高値・安値・終値にスナップします（[描画ツール](drawings.md)を参照）。

## 購読

```ts
import type { ChartEventMap } from '@ferrochart/core';

const onMove = (e: ChartEventMap['crosshairMove']) => {
  console.log(e.point?.time);
};

chart.on('crosshairMove', onMove);
// later
chart.off('crosshairMove', onMove);
```

- `on(name, callback)`でリスナーを追加します。不明なイベント名を渡すと例外が投げられます。
- `off(name, callback)`で削除します。`on`に渡したものと同じ関数オブジェクトを渡してください。
- 同じ関数を2回追加すると2回呼ばれます。`off`は両方を削除します。
- コールバックからは`on`や`off`を含め、チャートのどのメソッドでも呼べます。
- コールバック内で投げられた例外は、捕捉されなかったエラーと同様に報告されます（`reportError`：コンソールに表示され、`window.onerror`にも届きます）。他のコールバックはそのまま実行されます。

コールバックの引数はイベント名から型付けされるため、`chart.on('crosshairMove', (e) => e.point?.time)`には型注釈が不要です。上の例のようにコールバックを別に宣言する場合は、`ChartEventMap[name]`（パッケージからエクスポートされています）でペイロードの型を指定できます。

## 配信タイミング

チャートは`requestAnimationFrame`の中で描画します。各フレームでは、まず前回のフレーム以降の入力とAPI呼び出しを適用し、フレームを構築・レンダリングしてから、そのフレームで発生したイベントを配信します。その結果、次のようになります。

- イベントがDOMの入力ハンドラーの中や、イベントの原因となったメソッド呼び出しの中で配信されることはありません。`chart.drawings.add(...)`はすぐにidを返しますが、それに対応する`drawingCreated`イベントは次のフレームの後に届きます。
- 一度に配信されるのは最大で1フレーム分のイベントです。前回のフレーム以降にクロスヘアが何度か動いた場合でも、ポインターイベントごとではなく、最新の位置を持つ`crosshairMove`が1回だけ届きます。`visibleRangeChanged`と`sync`も同様です。
- ブラウザーはバックグラウンドのタブで`requestAnimationFrame`を停止するため、イベントはタブが再び表示されるまで遅れます。

## ライフタイム

チャートはcanvasとwindowに登録したリスナーによって動作し続けますが、これらのリスナーはチャートへの弱参照しか持ちません。チャートがページ上にある間は`FerroChart`オブジェクトへの参照を保持してください。オブジェクトがガベージコレクションされると、チャートは描画もイベント送信も停止します。チャートを使い終わったら`destroy()`を呼んでください。すべてのイベントリスナー（あなたが登録したものとDOMのもの）を削除し、GPUリソースを解放します。その後オブジェクトは使用できません。

```ts
chart.destroy();
```

## イベントリファレンス

| イベント | ペイロード | 発火タイミング |
|---|---|---|
| `click` | `ClickEvent` | プロット内で押して離すまでの移動が3 CSS px以内だったとき |
| `crosshairMove` | `{ point: CrosshairPoint \| null }` | クロスヘアが別の足、ペイン、価格に移動したとき、またはプロットから出たとき |
| `visibleRangeChanged` | `{ from, to }` | 表示中の論理範囲が変わったとき |
| `needMoreHistory` | `{ barsBeforeStart }` | 表示の左端が最初の足から1画面分以内に近づいたとき |
| `drawingCreated` | `{ id }` | 描画が作成されたとき |
| `drawingModified` | `{ id }` | 描画が移動された、スタイルが変更された、またはパラメーターが変更されたとき |
| `drawingRemoved` | `{ id }` | 描画が削除されたとき |
| `drawingSelected` | `{ id: number \| null }` | 選択中の描画が変わったとき |
| `sync` | `{ message: SyncMessage }` | チャートが同期グループにメッセージを送ったとき |
| `drawingSync` | `{}` | 取り出し待ちの描画同期操作がチャートにあるとき |
| `warning` | `ChartWarning` | チャートが自動的に何かを調整したとき |
| `markerHover` | `MarkerHoverEvent \| null` | ポインターがマーカーまたは「+k」バッジに乗ったとき、または離れたとき |

### click

```ts
interface ClickEvent {
  x: number;              // CSS px from the canvas' top-left corner
  y: number;
  pane: number;           // pane id
  logicalIndex: number;   // fractional logical index at x
  time: number | null;    // UTC ms of the nearest bar, null beyond the data
  price: number;          // price at y on the pane's price scale
  target: ClickTarget | null;
}

type ClickTarget =
  | { type: 'drawing'; id: number }
  | { type: 'marker'; series: Series; index: number; id: number; marker: Marker }
  | { type: 'markerOverflow'; series: Series; first: number; count: number; id: number; bars: { from: number; to: number }; marker: Marker }
  | { type: 'series'; series: Series };
```

クリックとは、プロット内での押下と解放（マウス、ペン、タッチ）のうち、その間のポインターの移動が3 CSS px以内のものです。それ以上動いた場合はパンになります。描画の作成中はクリックは送られません。

`target`はポインターの下にあったもので、次の順で判定されます。

1. **描画**（本体またはハンドルのいずれか）：`{ type: 'drawing', id }`。描画を押すと、その描画が選択されます。
2. **マーカー**：`{ type: 'marker', series, index, id, marker }`。`index`は時刻順にソートした`series.markers()`での位置、`id`は独自のマーカーid（省略時は0）、`marker`は独自のフィールドも含めた、`setMarkers`に渡した元のオブジェクトです。折りたたみやクラスタリングの「+k」バッジは`{ type: 'markerOverflow', series, first, count, id, bars, marker }`を持ちます。`first`は描画されなかった最初のマーカーのソート後のインデックス、`count`はその側で描画されなかった数、`id`/`marker`は最初のマーカーを指し、`bars: { from, to }`は両端を含む足の論理インデックス範囲です。描画されなかったマーカーは、ソート済みリスト内で連続しているとは限りません。独自のデータには`id`/`marker`を使ってください。[マーカーの独自データ](markers.md#your-own-data-on-a-marker)と[クラスタリング](markers.md#clustering-across-bars)を参照してください。
3. **シリーズ**：ポインターがローソク足の実体やヒゲの上、ラインから4 CSS px以内、ヒストグラムのバーの内側、またはドットの上にあるとき`{ type: 'series', series }`。シリーズが重なっている場合は、最後に追加されたものが優先されます。
4. 何もない場所では`null`。

targetに含まれる`Series`オブジェクトは、既存のシリーズを指す新しいハンドルです。`===`ではなく`id`で比較してください。

```ts
chart.on('click', (e: ChartEventMap['click']) => {
  if (e.target?.type === 'series' && e.target.series.id === candles.id) {
    console.log('clicked the candles at', e.time);
  }
});
```

インジケーターの出力は通常のシリーズなので、インジケーターのラインをクリックすると、`id`が`indicator.outputs()`のいずれかと一致する`series`ターゲットが得られます。

### crosshairMove

```ts
interface CrosshairPoint {
  logicalIndex: number;   // the bar the crosshair is on (whole number)
  time: number | null;    // UTC ms, null beyond the data
  pane: number;           // pane under the pointer
  price: number;          // price at the crosshair; NaN for a synced crosshair
  bar: BarValues | null;  // OHLCV of the first OHLC series with a bar here
  x?: number;             // pointer position, CSS px; absent for a synced crosshair
  y?: number;
}

interface BarValues {
  open: number; high: number; low: number; close: number;
  volume: number | null;
}
```

ポインターがプロットから出たとき、またはクロスヘアが非表示のとき、`point`は`null`になります。イベントはpointの内容が変わったときにだけ送られ、同じ足・同じ価格の範囲内でポインターを動かしても繰り返し送られることはありません。

マグネットモードでは、`price`はスナップ後の価格です。そのインデックスに足を持つOHLCシリーズがない場合（たとえばラインシリーズしかないチャートや、データの欠損部分）、`bar`は`null`になります。クロスヘア位置の他のシリーズの値を読むには`series.dataAt(point.logicalIndex)`を使います。

[同期グループ](sync.md)に属するチャートは、他のチャートのクロスヘアも通知します。そのようなpointは`price: NaN`で、`pane`はメインペインに設定され、`x`と`y`はありません。

### visibleRangeChanged

`{ from: number, to: number }`はプロットの左端と右端の論理インデックスで、小数を含みます。パン、ズーム、リサイズ、および表示を変えるメソッド（`setVisibleLogicalRange`、`fitContent`、`scrollToRealtime`、`setBarSpacing`、`setRightOffset`）のいずれでも発生します。同じ値は`chart.visibleLogicalRange()`でいつでも取得できます。

### needMoreHistory

`{ barsBeforeStart: number }`は、最初の足と表示の左端との間の足の数です。表示がすでにデータより前の空白部分を映している場合は負の値になります。イベントは最初の足ごとに1回送られます。古いデータを追加した後、ユーザーが新しい先頭までスクロールすると再び発火することがあります。

```ts
let loading = false;
chart.on('needMoreHistory', async (e: ChartEventMap['needMoreHistory']) => {
  if (loading) return;
  loading = true;
  try {
    await loadOlderBars(e.barsBeforeStart); // your code: fetch older bars, then series.prepend(them)
  } finally {
    loading = false;
  }
});
```

データの読み込みについては[データ](data.md)を参照してください。

### 描画イベント

`drawingCreated`、`drawingModified`、`drawingRemoved`は描画の`id`を、`drawingSelected`は選択されたidまたは`null`を持ちます。

| イベント | 原因 |
|---|---|
| `drawingCreated` | ユーザーが描画を完了したとき、`drawings.add`、描画を（元のidで）復活させる元に戻す／やり直し、`loadState`（読み込んだ描画ごとに1回）、同期グループから受け取った描画 |
| `drawingModified` | 描画やハンドルのドラッグ、`setAnchors`、`setStyle`、`setParam`、それらの変更の元に戻す／やり直し、同期による変更 |
| `drawingRemoved` | Deleteキー、`drawings.remove`、作成の取り消し、`loadState`（置き換えられる描画）、ペインの削除、同期による削除 |
| `drawingSelected` | 描画または空白部分を押したとき、新しい描画の完了（その描画が選択されます）、`drawings.select`、選択中の描画の非表示または削除 |

描画の表示、非表示、ロック、重なり順の変更ではイベントは送られません。[描画ツール](drawings.md)を参照してください。

### sync

`{ message: SyncMessage }`は、チャートが[同期グループ](sync.md)にクロスヘアまたは表示範囲のメッセージを送ったときに送られます。

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };
```

同じページ上で同じ`SyncGroup`に属するチャートについては、何もする必要はありません。このイベントは、ワーカー内など別の場所にあるチャートへメッセージを転送するためのもので、転送先では`applySync(message)`で適用します。グループに属していないチャートや、その種類のメッセージを共有しないグループに属するチャートは`sync`イベントを送りません。

### drawingSync

`{}`。同期グループで描画を共有しているチャートに、ページがまだ取り出していない描画操作があるときに送られます。送られるのは1回だけで、`chart.takeDrawingSyncOps()`を呼んだ後にのみ再び送られます。[チャートの同期](sync.md)を参照してください。

### warning

チャートが自動的に何かを修正し、その原因となった呼び出しは成功しています。

```ts
type ChartWarning =
  | { kind: 'timeAxisMerged'; series: Series; count: number }
  | { kind: 'ohlcRepaired'; series: Series; count: number }
  | { kind: 'dataDropped'; series: Series }
  | { kind: 'other' };
```

| `kind` | 意味 |
|---|---|
| `'timeAxisMerged'` | `series`の時刻が時間軸の足と足の間にありました。軸はすべてのシリーズの時刻の和集合として再構築され、最初に挿入された足より後の論理インデックスがずれました。`count`本の足が挿入されました。 |
| `'ohlcRepaired'` | `series`のうち`count`本の足で、高値が他の価格より低い、または安値が他の価格より高かったため、範囲が広げられました。（JavaScript APIはこのような足を拒否するため、JavaScriptから操作するチャートではこの警告は送られません。） |
| `'dataDropped'` | `series`のデータを再構築された時間軸に移せなかった（メモリ不足）ため、データが消去されました。 |
| `'other'` | 後のバージョンで追加された種類です。 |

```ts
chart.on('warning', (w) => {
  if (w.kind === 'timeAxisMerged') console.warn(`time axis rebuilt: ${w.count} bars inserted by series ${w.series.id}`);
});
```

### markerHover

```ts
interface MarkerHoverEvent {
  series: Series;
  index: number;          // position in series.markers(); for a badge, the first marker not drawn
  id: number;             // your marker id, 0 if absent
  marker: Marker;         // original object; for a badge, the first hidden marker
  bars: { from: number; to: number }; // inclusive logical bar range
  overflow: number;       // k for a "+k" badge, 0 for a marker
  x: number;              // center, CSS px from the canvas' top-left corner
  y: number;
  size: number;           // diameter, CSS px
}
```

ポインターがマーカーから離れると、ペイロードは`null`になります。あるマーカーから別のマーカーへ直接移動した場合は、新しいマーカーだけが送られます。ポインターの下のマーカーはフレームの描画時に判定されるため、イベントは1フレームにつき最大1回です。描画は対象外で、イントロアニメーションの再生中はどのマーカーもホバー中とはみなされません。アバターやバッジのツールチップに使えます（[マーカー](markers.md)を参照）。

## 例：クロスヘアに追従する凡例

チャートの上に重ねたDOM要素に、クロスヘアの下にある足を表示し、ポインターが別の場所にあるときは最新の足を表示します。

```html
<div style="position: relative; height: 480px">
  <canvas id="chart" style="width: 100%; height: 100%"></canvas>
  <div id="legend" style="position: absolute; left: 12px; top: 8px;
       font: 12px sans-serif; pointer-events: none"></div>
</div>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart');
const legend = document.getElementById('legend');

if (!(canvas instanceof HTMLCanvasElement) || !(legend instanceof HTMLDivElement)) {
  throw new Error('Expected #chart canvas and #legend div');
}

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars); // your OHLC columns
const sma = chart.addIndicator(candles, 'sma', { n: 20 }).outputs()[0];

const fmt = (v: number | undefined) => (v === undefined || Number.isNaN(v) ? '–' : v.toFixed(2));

const show = (index: number) => {
  const bar = candles.dataAt(index);
  const avg = sma.dataAt(index);
  if (!bar || !('open' in bar)) {
    legend.textContent = '';
    return;
  }
  const time = chart.timeAtIndex(index);
  const date = time === null ? '' : new Date(time).toISOString().slice(0, 16).replace('T', ' ');
  legend.textContent =
    `${date}  O ${fmt(bar.open)}  H ${fmt(bar.high)}  L ${fmt(bar.low)}  C ${fmt(bar.close)}` +
    `  SMA 20 ${fmt(avg && 'value' in avg ? avg.value : undefined)}`;
};

function showLatest() {
  const range = chart.dataRange();
  if (range) show(range.to);
}

chart.on('crosshairMove', (e: ChartEventMap['crosshairMove']) => {
  if (e.point) show(e.point.logicalIndex);
  else showLatest();
});
showLatest();
```

`dataAt`は、OHLCシリーズでは`{ open, high, low, close, volume }`、それ以外では`{ value }`を返し、データがないか欠損部分の場合は`null`を返します。`null`でないことを確認してから、`'open' in bar`または`'value' in avg`で戻り値の型を絞り込んでフィールドを読み取ります。インジケーターの出力はウォームアップ期間の足では`NaN`です。リアルタイム更新で最新の足が変わったときは、ポインターがチャート上になければ`update`の後に`showLatest()`を呼んでください。

## 例：マーカーとシリーズのクリック

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

candles.setMarkers([
  { time: bars.time[120], position: 'belowBar', shape: 'arrowUp', color: '#26a69a', text: 'Buy' },
  { time: bars.time[180], position: 'aboveBar', shape: 'arrowDown', color: '#ef5350', text: 'Sell' },
]);

chart.on('click', (e: ChartEventMap['click']) => {
  const t = e.target;
  if (!t) {
    console.log(`empty space at ${e.price.toFixed(2)}`);
    return;
  }
  switch (t.type) {
    case 'marker': {
      const marker = t.marker;
      console.log(`marker "${marker.text}" at`, new Date(Number(marker.time)));
      break;
    }
    case 'series':
      if (t.series.id === candles.id) {
        const bar = candles.dataAt(Math.round(e.logicalIndex));
        console.log('candle', bar);
      }
      break;
    case 'drawing':
      console.log('drawing', t.id, chart.drawings.get(t.id)?.kind);
      break;
  }
});
```

マーカーのインデックスは、`setMarkers`に渡した順序ではなく、`series.markers()`が返す時刻順のリストを指します。後で見つけ直す必要がある場合は、独自のデータを時刻をキーにして保持してください。[マーカーと価格ライン](markers.md)を参照してください。
