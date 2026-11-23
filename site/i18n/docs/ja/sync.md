<!-- source: 37271b3d45 -->
# チャートの同期

同じ`SyncGroup`に参加したチャートは互いに追従します。1つのチャート上でクロスヘアを動かすと他のチャートにも表示され、1つをパンやズームすると他のチャートも同じ時間範囲にスクロールします。オプションで、1つのチャートで作成した描画を他のチャートにも表示できます。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const left: HTMLCanvasElement;
declare const right: HTMLCanvasElement;

const a = await FerroChart.create(left);
const b = await FerroChart.create(right);

const group = new SyncGroup();
a.joinSync(group);
b.joinSync(group);
```

`new SyncGroup()`にはWebAssemblyモジュールが必要です。モジュールは最初の`FerroChart.create`で読み込まれます。チャートより先にグループを作成するには、まず`await init()`を呼んでください（`import init from '@ferrochart/core'`）。

## 足の番号ではなく時刻

チャート間のメッセージはすべて**時刻**（UTC ms）を運び、論理インデックスは決して使いません。そのため、グループ内のチャートは異なる銘柄、異なる時間足、あるいは欠損の位置が異なるデータを表示していても構いません。

- 1分足チャートで10:37にあるクロスヘアは、1時間足チャートでは10:00の足（その時刻以前の最後の足）に表示されます。データの両端を越えた時刻は外挿されます。
- パンやズームの後、他のチャートは足の間隔の制限が許す範囲で、左端から右端まで同じ時間幅を表示します（右端は維持されます）。2時間分を表示している1分足チャートに1時間足チャートが追従すると、プロット全体で足2本分になってしまいますが、デフォルトの最大足間隔50 pxでは、同じ時刻で終わるより広い範囲が表示されます。

データのないチャートは受け取った範囲を無視します。

## 参加と離脱

### `chart.joinSync(group, options?)`

`group`に参加します。すでに別のグループに属していた場合は、そのグループから離脱します。次のフレームで、チャートはグループの現在のクロスヘアと表示範囲を引き継ぎます。

`SyncOptions`でチャートが共有するものを選びます。各オプションは双方向に作用します。`crosshair: false`のチャートは、他のチャートのクロスヘアを表示せず、自分のクロスヘアも送りません。

| オプション | 共有するもの | デフォルト |
|---|---|---|
| `crosshair` | クロスヘアの時刻 | `true` |
| `visibleRange` | 表示中の時間範囲（パンとズーム） | `true` |
| `drawings` | メインペインの描画 | `false` |

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const overview: FerroChart;
declare const detail: FerroChart;

// The overview shows the crosshair of the others but keeps its own zoom.
overview.joinSync(group, { visibleRange: false });
detail.joinSync(group);
```

チャートのオプションを変えるには、同じグループで`joinSync`をもう一度呼びます。

### `chart.leaveSync()`

グループから離脱します。このチャートが他のチャートに表示していたクロスヘアは消えます。`chart.destroy()`でもグループから離脱します。

### `chart.syncOptions()`

チャートが共有するものを、すべてのオプションを埋めた形で返します。グループに属していない場合は`null`です。

```ts
import { FerroChart } from '@ferrochart/core';

declare const chart: FerroChart;

const opts = chart.syncOptions();
if (opts === null) console.log('not synced');
else console.log(opts.crosshair, opts.visibleRange, opts.drawings);
```

## 何がいつ送られるか

- **クロスヘア。** ポインターがチャートのプロット上を動くと、チャートはクロスヘアの下の時刻を送ります。ポインターが出ると、クロスヘアが消えたことを送ります。他のチャートはその時刻にクロスヘアを表示します。そのとき他のチャートの`crosshairMove`イベントの`point`は`price: NaN`で、`x`/`y`がなく、`pane`はメインペインになります。
- **表示範囲。** ユーザーの操作（ドラッグ、ホイール、ピンチ、キー）またはコード（`setVisibleLogicalRange`、`setBarSpacing`、`fitContent`など）で表示が変わると、チャートは左端と右端の時刻を送ります。同じフレームでチャート自身が表示を変えた場合は、受け取った範囲よりその変更が優先されます。
- 受け取ったメッセージを適用しても再送はされないので、メッセージがチャート間で行き来し続けることはありません。

メッセージを送ったチャートは、グループ内の他のチャートのフレームを予約します。他のチャートはそのフレームを描画するときにメッセージを適用します。

## 描画の同期

`drawings: true`を指定すると、チャートの**メインペイン**上の描画が、描画を共有する他のチャートのメインペインにコピーされます。追加、移動、スタイル変更、表示、非表示、ロック、削除が対象です。アンカーは価格なので、同じ銘柄のチャート（たとえば複数の時間足）に使ってください。

```ts
import { FerroChart, SyncGroup } from '@ferrochart/core';

declare const group: SyncGroup;
declare const m5: FerroChart;
declare const h1: FerroChart;

m5.joinSync(group, { drawings: true });
h1.joinSync(group, { drawings: true });

// Appears on h1 at its next frame.
m5.drawings.add({
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 42_000 }],
});
```

詳細：

- `drawings: true`で参加したチャートは、まずグループがすでに持っている描画を引き継ぎ、それから自分の描画を共有します。
- 他のペインの描画は共有されません（他のペインはチャートごとに異なるインジケーターを表示している場合があります）。受け取った描画は常にメインペインに入ります。
- 受け取った変更は、受信側チャートの元に戻す履歴には追加されません。変更を行ったチャートでの元に戻す／やり直しは他の変更と同様に送られるため、最終的にすべてのチャートが同じ状態になります。
- 選択状態と作成途中の描画は共有されません。
- すべてのチャートがその描画の種類を知っている必要があります。JavaScriptで書いた描画ツール（`chart.drawings.register`）は、ページのすべてのチャートに対して1回登録されます。

## グループオブジェクトを共有しないチャート

`SyncGroup`は、1つのページ上で同じWebAssemblyモジュールのインスタンスが作成したチャート同士をつなぎます。別のウィンドウ、タブ、iframe、あるいはネイティブアプリケーションなど、他の場所にあるチャートは参加できません。そうした場合は、チャートが送信内容を通知するので、任意の転送手段（`postMessage`、`BroadcastChannel`、WebSocket）で転送してください。

送信側のチャートはグループに属している必要があります（1つだけのグループでも構いません）。メッセージを通知するのはグループに属するチャートだけです。

### クロスヘアと表示範囲

`sync`イベントは、このチャートがグループに送った各メッセージを配信します。受信側では`chart.applySync(message)`で適用します。

```ts
import { FerroChart, SyncGroup, type SyncMessage } from '@ferrochart/core';

declare const chart: FerroChart;

const channel = new BroadcastChannel('chart-sync');
const group = new SyncGroup();
chart.joinSync(group);

// Out: what this chart sent.
chart.on('sync', ({ message }: { message: SyncMessage }) => {
  channel.postMessage(message);
});

// In: what a chart in another window sent.
channel.onmessage = (e: MessageEvent<SyncMessage>) => {
  chart.applySync(e.data);
};
```

`SyncMessage`は単純なデータです。

```ts
type SyncMessage =
  | { type: 'crosshair'; time: number | null }       // null: the crosshair left
  | { type: 'visibleRange'; from: number; to: number }; // times at the plot edges, UTC ms
```

`applySync`は呼び出したチャートにだけ作用し、再送されることも、そのチャート自身のグループに渡されることもありません。受信側では、追従させたいすべてのチャートで呼んでください。

`applySync({ type: 'visibleRange', from, to })`は、単一のチャートで時間範囲を表示する手軽な方法としても使えます。

### 描画

描画の変更は`DrawingSyncOp`の値として送られます。各描画は、チャートをまたいで描画を識別する`key`を持ちます。

```ts
type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };
```

| メンバー | 説明 |
|---|---|
| `drawingSync`イベント | チャートが描画操作を送りました。送られるのは1回で、次は`takeDrawingSyncOps()`を呼んだ後に届きます |
| `takeDrawingSyncOps()` | 送信済みでまだ取り出されていない操作を、古い順に返します。保持されるのは最大1024件で、それより古いものは破棄されます |
| `applyDrawingSync(op)` | 他のチャートからの操作を1つ適用します（メインペイン、元に戻せない、再送されない） |
| `drawingSyncState()` | 後から参加するチャートのために、このチャートの同期対象の描画をすべて`'add'`操作として返します |
| `loadDrawingSyncState(ops)` | 同期対象の描画をそのようなリストに一致させます。追加と更新を行い、リストにない同期対象の描画を削除します |

キーはグループが割り当てます。グループの`origin`に2³² を掛け、カウンターを足したものです。複数のページが互いに描画を転送する場合は、`new SyncGroup(origin)`で各ページのグループに別々のoriginを与え、ページ間でキーが衝突しないようにしてください。originは0（デフォルト）から2²¹ − 1までの整数です。それより大きいと例外になり、これによりキーがJavaScriptの数値として正確に保たれます。

```ts
import { FerroChart, SyncGroup, type DrawingSyncOp } from '@ferrochart/core';

declare const chart: FerroChart;
declare const pageNumber: number; // different on every page, 0 to 2^21 - 1

type Msg =
  | { type: 'ops'; ops: DrawingSyncOp[] }
  | { type: 'hello' }
  | { type: 'state'; ops: DrawingSyncOp[] };

const channel = new BroadcastChannel('chart-drawings');
const group = new SyncGroup(pageNumber);
chart.joinSync(group, { drawings: true });

const send = (m: Msg) => channel.postMessage(m);

// Out: forward this chart's changes.
chart.on('drawingSync', () => send({ type: 'ops', ops: chart.takeDrawingSyncOps() }));

channel.onmessage = (e: MessageEvent<Msg>) => {
  const m = e.data;
  if (m.type === 'ops') {
    for (const op of m.ops) {
      try {
        chart.applyDrawingSync(op);
      } catch (err) {
        console.warn('drawing op skipped:', err);
      }
    }
  } else if (m.type === 'hello') {
    // A page that just opened asks for the current drawings.
    send({ type: 'state', ops: chart.drawingSyncState() });
  } else {
    chart.loadDrawingSyncState(m.ops);
  }
};

send({ type: 'hello' });
```

エラー：

- `applyDrawingSync`は、チャートが持っていないキーに対する`update`、`setVisible`、`setLocked`、`remove`と、構築できない描画（未登録の種類、誤ったアンカーやパラメーター）に対して例外を投げます。チャートがすでに持っているキーの`add`は、その描画を更新します。
- `loadDrawingSyncState`は、操作が`'add'`でない場合、キーが2回現れる場合、描画を構築できない場合に例外を投げ、チャートは変更されません。

## 例：マルチタイムフレームのレイアウト

1つの銘柄について、1分足、5分足、1時間足、日足の4つのチャートを並べます。日中の3つのチャートはクロスヘア、ズーム、描画を共有します。日足チャートは他のチャートのクロスヘアを表示しますが、独自の表示を保つので、全体を見渡すチャートのままです。

```html
<div class="grid">
  <canvas id="m1"></canvas><canvas id="m5"></canvas>
  <canvas id="h1"></canvas><canvas id="d1"></canvas>
</div>
<style>
  .grid { display: grid; grid-template: 1fr 1fr / 1fr 1fr; gap: 4px; height: 100vh; }
  .grid canvas { width: 100%; height: 100%; min-height: 0; }
</style>
```

```ts
import { FerroChart, SyncGroup, randomWalk, type SyncOptions } from '@ferrochart/core';

const MIN = 60_000;
const start = Date.UTC(2024, 0, 1);

const layout: { id: string; interval: number; sync: SyncOptions }[] = [
  { id: 'm1', interval: MIN, sync: { drawings: true } },
  { id: 'm5', interval: 5 * MIN, sync: { drawings: true } },
  { id: 'h1', interval: 60 * MIN, sync: { drawings: true } },
  { id: 'd1', interval: 1_440 * MIN, sync: { visibleRange: false } },
];

const group = new SyncGroup();
const charts: FerroChart[] = [];

for (const { id, interval, sync } of layout) {
  const canvas = document.getElementById(id) as HTMLCanvasElement;
  const chart = await FerroChart.create(canvas, { theme: 'dark' });
  const candles = chart.addCandlestickSeries();
  // Replace with your data; all four cover the same period.
  const bars = Math.round((90 * 1_440 * MIN) / interval);
  candles.setData(randomWalk(7, bars, start, interval));
  chart.addVolumeSeries(candles);
  chart.joinSync(group, sync);
  charts.push(chart);
}

// Show the last two days of the 1-minute chart; m5 and h1 follow it.
const range = charts[0].dataRange();
if (range) charts[0].setVisibleLogicalRange(range.to - 2 * 1_440, range.to);
charts[3].fitContent();
```

この例のデータはランダムなので、4つのチャートは同じ価格を表示しません。1つの銘柄の実データであれば揃います。

このレイアウトについての補足：

- `charts[0]`の`setVisibleLogicalRange`は、ユーザーのパンと同じように表示を変えるため、変更はグループに送られ、`m5`と`h1`が追従します。（`applySync`では`charts[0]`だけが変わります。適用されたメッセージは転送されないためです。）
- 日足チャートは`visibleRange: false`で参加します。他のチャートに追従することも動かすこともありませんが、クロスヘアは表示します。
- 同期されたチャートは、メッセージが届いたときか自身の内容が変わったときに再描画します。アイドル状態のチャートは再描画しません。

## Rust

Rustでは同じ機能が`Chart::join_sync(&group, SyncOptions)`、`Chart::leave_sync`、`Chart::apply_sync`、`ChartEvent::Sync`、`Chart::take_drawing_sync_ops`、`Chart::apply_drawing_sync`、`Chart::drawing_sync_state`、`Chart::load_drawing_sync_state`として提供され、`SyncGroup::new()`と`SyncGroup::with_origin(origin)`を使います。
