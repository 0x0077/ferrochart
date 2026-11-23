<!-- source: 58a84755b2 -->
# 描画ツール

描画オブジェクトとは、ユーザーまたはコードによってチャート上に配置されるオブジェクトです。トレンドライン、矩形、フィボナッチ系ツール、ポジション、パターン、テキストなどがあります。各描画オブジェクトはデータ空間（時刻と価格）にアンカーを持つため、チャートをパン、ズーム、リサイズしても足に固定されたままです。作成、選択、ドラッグ、削除、アンドゥ/リドゥはFerroChartが処理します。ページ側はツールを有効にし、必要に応じて描画イベントに反応するだけです。

## Drawingsオブジェクト

描画関連のメソッドはすべて`chart.drawings`にあります。

```ts
const drawings = chart.drawings;

drawings.startTool('trend_line');   // the next clicks draw a trend line
const id = drawings.add({           // or add one from code
  kind: 'horizontal_line',
  anchors: [{ time: Date.UTC(2024, 0, 2), price: 101.5 }],
});
drawings.setStyle(id, { line: '#ef5350', lineWidth: 2 });
drawings.undo();
```

描画オブジェクトは数値のidで識別されます。idはチャート内で再利用されることはなく、アンドゥで描画オブジェクトが復活した場合は元のidが復元されます。

| メソッド / プロパティ | 機能 |
|---|---|
| `kinds()` | `add`と`startTool`が受け付ける種類（組み込みの29種類と登録済みのもの） |
| `startTool(kind)` / `cancelTool()` / `activeTool` | インタラクティブな作成 |
| `add(data, pane?)` | 描画オブジェクトを追加し、そのidを返す |
| `get(id)` | 描画オブジェクトの`DrawingData`（種類、アンカー、スタイル、パラメーター）。未知のidには`null` |
| `ids()` | すべてのid（下から上の順） |
| `paneOf(id)` | 描画オブジェクトのペインid |
| `setAnchors(id, anchors)` | アンカーを移動する |
| `setStyle(id, style)` | スタイルの一部のキーを変更する |
| `setParam(id, key, value)` | ツールのパラメーター（`text`、`levels`）を設定する |
| `select(id)` / `selected` | 選択 |
| `setVisible(id, visible)` / `isVisible(id)` | 表示・非表示 |
| `setLocked(id, locked)` / `isLocked(id)` | ドラッグに対するロック |
| `bringToFront(id)` / `sendToBack(id)` | 重なり順 |
| `remove(id)` | 描画オブジェクトを削除する |
| `undo()` / `redo()` / `canUndo` / `canRedo` / `clearHistory()` | 履歴 |
| `register(kind, tool)` | JavaScriptで書いた描画ツールを追加する |

idを受け取るメソッドは、未知のidに対して例外をthrowします（ただし`get`、`paneOf`、`isVisible`、`isLocked`は`null`または`undefined`を返します）。描画オブジェクトはチャートの状態と一緒に保存・復元されます（[状態の保存](state.md)を参照）。

## 組み込みの種類

種類の名前はsnake_caseの文字列です。

**ライン**

| 種類 | アンカー | 備考 |
|---|---|---|
| `trend_line` | 2 | アンカー間の線分 |
| `ray` | 2 | 最初のアンカーから2番目のアンカーを通ってプロットの端まで |
| `extended_line` | 2 | 両方のアンカーを通り、プロットの両端まで |
| `horizontal_line` | 1 | アンカーの価格でプロットを横断し、右端に価格ラベルを表示 |
| `horizontal_ray` | 1 | アンカーから右端まで、価格ラベル付き |
| `vertical_line` | 1 | アンカーの時刻でプロットを縦断 |
| `arrow` | 2 | 2番目のアンカーに矢じりが付いた線分 |

**図形とテキスト**

| 種類 | アンカー | 備考 |
|---|---|---|
| `parallel_channel` | 3 | 最初の2つのアンカーを通るラインと、3番目のアンカーを通るその平行線。塗りつぶしと破線の中央線付き |
| `rectangle` | 2 | 対角の2隅。塗りつぶしあり |
| `ellipse` | 2 | 2つのアンカーが作る矩形に内接。塗りつぶしあり |
| `text` | 1 | アンカーから始まるテキスト。パラメーター`text`（デフォルト`'Text'`）。fillが透明でない場合は`fill`色の背景上に描画 |
| `callout` | 2 | 2番目のアンカーにテキストボックスを置き、最初のアンカーへの引き出し線を引く。パラメーター`text`（デフォルト`'Callout'`）。ボックスはラインの色で塗られ、テキストは白 |
| `path` | 2以上 | ポリライン。1点ごとに1クリックし、ダブルクリックまたはEnterで終了 |
| `brush` | 2以上 | フリーハンドの線：押してドラッグし、離す。ハンドルはなく、全体として移動 |

**計測とポジション**

| 種類 | アンカー | 備考 |
|---|---|---|
| `price_range` | 2 | 網掛けした範囲に、最初のアンカーから2番目のアンカーまでの価格変化と変化率を表示 |
| `date_range` | 2 | 網掛けした範囲に、足の本数と期間を表示 |
| `long_position` | 3（2クリック） | エントリー（最初のアンカー）、利確目標（2番目）、損切り（3番目）。作成時、損切りはエントリーを中心に利確目標と対称な位置に置かれます。損益エリア、価格と変化率のラベル、リスクリワード比を表示 |
| `short_position` | 3（2クリック） | `long_position`と同様で、変化率は売りポジションとして計算 |

**フィボナッチとギャン**

| 種類 | アンカー | 備考 |
|---|---|---|
| `fib_retracement` | 2 | 各レベルについて`p2 + (p1 − p2) · level`にレベルラインを引き、`level (price)`のラベルと交互の網掛けバンドを表示。パラメーター`levels`（デフォルト`[0, 0.236, 0.382, 0.5, 0.618, 0.786, 1]`） |
| `fib_extension` | 3 | 3番目のアンカーから`p3 + (p2 − p1) · level`にレベルを描画。パラメーター`levels`（デフォルト`[0, 0.382, 0.618, 1, 1.272, 1.618, 2, 2.618]`） |
| `fib_time_zones` | 2 | フィボナッチ数F = 0, 1, 2, 3, 5, 8, …について、足`i1 + (i2 − i1) · F`の位置に垂直線 |
| `gann_fan` | 2 | 最初のアンカーから、2番目のアンカーへのラインの傾きの1/8〜8倍の傾きを持つ放射線。ラベルは`1x8`〜`8x1` |
| `pitchfork` | 3 | アンドリューのピッチフォーク：最初のアンカーから他の2つのアンカーの中点を通る中央線と、その2つを通る平行線。平行線の間は塗りつぶし |
| `schiff_pitchfork` | 3 | `pitchfork`と同様だが、起点を価格方向に2番目のアンカーへ半分だけ寄せる |
| `modified_schiff_pitchfork` | 3 | `pitchfork`と同様だが、起点を最初の2つのアンカーの中点に置く |

**パターンと波動**

| 種類 | アンカー | 備考 |
|---|---|---|
| `xabcd_pattern` | 5 | 点X、A、B、C、Dと各レッグ間のリトレースメント比率。XABとBCDの三角形は塗りつぶし |
| `head_and_shoulders` | 7 | 左肩、頭、右肩のラベルと、3番目と5番目のアンカーを通るネックライン。塗りつぶしあり |
| `elliott_impulse` | 6 | 0〜5のラベルが付いた波動 |
| `elliott_correction` | 4 | 0、A、B、Cのラベルが付いた波動 |

`p1`、`p2`、`p3`は順にアンカーの価格です。パターンと波動のラベルは、山では上に、谷では下に表示されます。

## インタラクティブな作成

```ts
chart.drawings.startTool('rectangle');
```

`startTool(kind)`はツールを有効にします。プロット上のカーソルが十字になり、次にいずれかのペインを押すとそこで描画が始まります。未知の種類を指定すると例外がthrowされます。ツールを有効にすると、作成中の描画はキャンセルされます。

描画の完了方法は種類によって異なります。

- **アンカー数が固定**（ほとんどの種類）：最初に押した位置に最初のアンカーが置かれ、次のアンカーはもう一度クリックするまでポインターに追従します。これを最後のアンカーが置かれるまで繰り返します。アンカーが1つの種類（`horizontal_line`、`horizontal_ray`、`vertical_line`、`text`）は最初に押した時点で完了します。最初の2つのアンカーは、押してドラッグし、離すことでも配置できます。
- **可変個数**（`path`）：アンカーごとに1クリックします。ダブルクリックまたはEnterで終了します（ダブルクリックによる重複点は除かれます）。終了には2つ以上のアンカーが必要です。
- **フリーハンド**（`brush`）：押してドラッグし、離します。3 CSS px移動するごとにアンカーが追加されます。アンカーが2つ未満のストロークは破棄されます。

アンカーの配置中、xは最も近い足にスナップし（最後の足より後の時刻は外挿されます）、クロスヘアがマグネットモードのときはyも最も近い始値・高値・安値・終値にスナップします。フリーハンドのアンカーはスナップしません。

描画が完了すると、その描画オブジェクトが選択され、`drawingCreated`イベントが送られ、**ツールは無効になります**。1回の`startTool`で作成できる描画オブジェクトは1つです。同じ種類を続けて描くには、`drawingCreated`リスナーの中でツールを再び有効にしてください（後述のツールバーの例を参照）。

| キー | 作成中の動作 |
|---|---|
| Escape | 作成中の描画をキャンセルする（アンカーが2つ以上ある`path`は代わりに完了する）、または有効なツールを無効にする |
| Enter | `path`を完了する |

`cancelTool()`はツールを無効にし、作成中の描画を破棄します。イベントは送られません。`activeTool`は有効なツールの種類、または作成中の描画の種類で、それ以外の場合は`undefined`です。

描画の作成中は`click`イベントは送られません。

## 選択と編集

ツールが有効でないとき：

- **描画オブジェクトを押す**と選択されます（`drawingSelected`）。ハンドルが表示されます。ほとんどの種類ではアンカーごとに1つ、ポジションではエントリー、利確目標、損切り、右端にハンドルがあり、水平線と垂直線ではプロットの中央に1つ、ブラシにはハンドルがありません。
- 描画オブジェクトを**ドラッグ**すると移動します。水平方向は足単位で、垂直方向はポインターに追従します。ハンドルをドラッグするとそのアンカーが移動します。ドラッグが終わると`drawingModified`が送られます。ドラッグ中にEscapeを押すと描画オブジェクトは元に戻ります。
- **何もない場所を押す**と選択が解除されます。
- ヒット判定は、選択中の描画オブジェクトのハンドル、選択中の描画オブジェクト、その他の描画オブジェクト（上から順）の順に行われます。
- 非表示の描画オブジェクトはヒットしません。矩形、楕円、チャネルは輪郭でヒットし、塗りが透明でない場合は内部のどこでもヒットします。

描画オブジェクトを押してもパンにはなりません。ロックされた描画オブジェクトを押すと、それが選択され、そのままチャートがパンします。

### キーボード

canvasはフォーカスがあるときにキーボード入力を受け取ります。押されたときにフォーカスされます。

| キー | 動作 |
|---|---|
| Delete | 選択中の描画オブジェクトを削除する（ロックされたものも含む） |
| Escape | 作成をキャンセルする、ツールを無効にする、または現在のドラッグを元に戻す |
| Enter | 作成中の`path`を完了する |
| Ctrl+Z / ⌘+Z | アンドゥ |
| Ctrl+Shift+Z / ⌘+Shift+Z、Ctrl+Y / ⌘+Y | リドゥ |

Backspaceは何もしません。別のキーで削除するには、自分でcanvasのイベントを監視してください。

```ts
canvas.addEventListener('keydown', (e) => {
  const id = chart.drawings.selected;
  if (e.key === 'Backspace' && id != null) chart.drawings.remove(id);
});
```

## コードからの追加

```ts
const id = chart.drawings.add({
  kind: 'fib_retracement',
  anchors: [
    { time: Date.UTC(2024, 0, 10), price: 120 },
    { time: Date.UTC(2024, 1, 20), price: 95 },
  ],
  style: { line: '#787b86', fill: 'rgba(120, 123, 134, 0.15)' },
  params: { levels: [0, 0.382, 0.5, 0.618, 1] },
});
```

```ts
interface DrawingData {
  kind: string;
  anchors: { time: number | bigint; price: number }[];
  style?: DrawingStyle;
  params?: Record<string, number | boolean | string | number[]>;
}
```

- `time`はUTCのミリ秒（整数）です。足の時刻である必要はありません。
- `price`はペインの最初の価格シリーズの単位で、有限の値である必要があります。
- アンカーの数は種類に合っている必要があります。ほとんどの種類は表のとおりちょうどその数、ポジションは2または3（2の場合、損切りは利確目標と対称な位置に追加されます）、`path`と`brush`は2〜4096です。
- `pane`のデフォルトはメインペインです。別のペインに描画するにはペインidを渡します。
- 省略したスタイルのキーは後述のデフォルト値になります。種類が使わないパラメーターのキーは無視されます。

未知の種類、アンカーの過不足、有限でない価格、未知のペイン、無効なスタイル、無効なパラメーターの場合、`add`は例外をthrowし、何も追加しません。コードから追加した描画オブジェクトは選択されません。

`get(id)`は、すべてのキーを含む保存形式を返します。`kind`、`anchors`、完全な`style`、そして種類のパラメーターを含む`params`（`text`と`callout`の`text`、フィボナッチ・リトレースメントとエクステンションの`levels`）です。これを`add`に渡すと描画オブジェクトをコピーできます。

```ts
const copy = chart.drawings.get(id);
if (copy) chart.drawings.add(copy, chart.drawings.paneOf(id));
```

`setAnchors(id, anchors)`は描画オブジェクトを移動します。描画オブジェクトが持つのと同じ数のアンカーが必要です（完成したポジションなら3）。

## スタイル

```ts
chart.drawings.setStyle(id, { line: '#26a69a', lineWidth: 2, lineDash: 'dashed' });
```

| キー | 型 | デフォルト | 用途 |
|---|---|---|---|
| `line` | `Color` | `#2962ff` | ラインと輪郭、範囲とポジションのラベル背景、コールアウトのボックス |
| `lineWidth` | `number` | `1` | CSS px、0より大きい値 |
| `lineDash` | `'solid' \| 'dashed' \| 'dotted'` | `'solid'` | ラインの破線パターン |
| `fill` | `Color` | 不透明度16%の`#2962ff` | 領域：チャネル、矩形、楕円、範囲、フィボナッチのバンド、ピッチフォーク、パターンの三角形、テキストの背景 |
| `text` | `Color` | `#2962ff` | 背景のないテキストとラベル |
| `fontSize` | `number` | `12` | CSS px、テキストとラベル |

`setStyle`は渡したキーだけを変更します。色に`null`は指定できません。塗りなしにするには`'transparent'`を使います。ポジションの損益エリアの色は緑と赤で固定です。

## パラメーター

```ts
chart.drawings.setParam(textId, 'text', 'Breakout');
chart.drawings.setParam(fibId, 'levels', [0, 0.5, 1, 1.618]);
```

| パラメーター | 種類 | 値 |
|---|---|---|
| `text` | `text`、`callout` | 文字列 |
| `levels` | `fib_retracement`、`fib_extension` | 最大24個の有限の数値 |

型が誤っている値は例外をthrowし、描画オブジェクトは変更されません。スタイルとパラメーターの変更は`drawingModified`を送り、アンドゥできます。

## 表示、ロック、重なり順

```ts
chart.drawings.setVisible(id, false);   // hidden drawings are not drawn or hit
chart.drawings.setLocked(id, true);     // can be selected and deleted, not dragged
chart.drawings.bringToFront(id);
chart.drawings.sendToBack(id);
console.log(chart.drawings.isVisible(id), chart.drawings.isLocked(id), chart.drawings.ids());
```

選択中の描画オブジェクトを非表示にすると、選択が解除されます。ロックしても、コードからの`setAnchors`、`setStyle`、`remove`は妨げられません。表示、ロック、重なり順の変更はイベントを送らず、アンドゥ履歴にも含まれませんが、チャートの状態と一緒に保存されます。

## 選択

```ts
chart.drawings.select(id);      // selects it and shows its handles
chart.drawings.select(null);    // clears the selection
const sel = chart.drawings.selected; // the selected id, or undefined
```

## 削除

```ts
chart.drawings.remove(id);
for (const d of chart.drawings.ids()) chart.drawings.remove(d); // remove all
```

ペインを削除すると、その描画オブジェクトも削除されます。描画オブジェクトがあった場合、アンドゥ履歴はクリアされます。

## アンドゥとリドゥ

ユーザーによるものかコードによるものかにかかわらず、次の変更が記録されます：追加、削除、移動（ドラッグ、`setAnchors`）、スタイルやパラメーターの変更。選択、表示、ロック、重なり順は記録されません。

```ts
chart.drawings.undo();          // false if there was nothing to undo
chart.drawings.redo();
if (chart.drawings.canUndo) { /* enable the undo button */ }
chart.drawings.clearHistory();
```

履歴は直近256件の変更を保持します。新しい変更を行うとリドゥのリストはクリアされます。`undo`と`redo`は作成中の描画をキャンセルします。状態を読み込む（`loadState`）と履歴はクリアされます。同期グループから受け取った描画オブジェクトは記録されません。

## 描画イベント

| イベント | ペイロード | タイミング |
|---|---|---|
| `drawingCreated` | `{ id }` | ユーザーによる描画の完了、コードからの追加、アンドゥ/リドゥによる復元、または読み込み |
| `drawingModified` | `{ id }` | 移動、スタイル変更、またはパラメーター変更 |
| `drawingRemoved` | `{ id }` | 削除 |
| `drawingSelected` | `{ id: number \| null }` | 選択の変更 |

イベントはメソッド呼び出しの中ではなく、変更が発生したフレームの後に届きます。[クロスヘアとイベント](crosshair-events.md)を参照してください。

```ts
chart.on('drawingCreated', (e: ChartEventMap['drawingCreated']) => {
  const d = chart.drawings.get(e.id);
  console.log('new', d?.kind, d?.anchors);
});
```

## 例：ツールバー

```html
<div id="toolbar">
  <button data-kind="trend_line">Trend</button>
  <button data-kind="horizontal_line">Horizontal</button>
  <button data-kind="rectangle">Rectangle</button>
  <button data-kind="fib_retracement">Fib</button>
  <button data-kind="long_position">Long</button>
  <button data-kind="path">Path</button>
  <button data-kind="brush">Brush</button>
  <button data-kind="text">Text</button>
  <label><input id="sticky" type="checkbox"> Keep drawing</label>
  <button id="lock">Lock</button>
  <button id="delete">Delete</button>
  <button id="undo">Undo</button>
  <button id="redo">Redo</button>
</div>
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
```

```ts
import { FerroChart, type ChartEventMap } from '@ferrochart/core';

const canvas = document.getElementById('chart') as HTMLCanvasElement;
const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(bars);

const d = chart.drawings;
const toolbar = document.getElementById('toolbar') as HTMLDivElement;
const sticky = document.getElementById('sticky') as HTMLInputElement;
const button = (id: string) => document.getElementById(id) as HTMLButtonElement;
let lastKind: string | null = null;

function refresh() {
  const active = d.activeTool ?? null;
  toolbar.querySelectorAll<HTMLButtonElement>('button[data-kind]').forEach((b) => {
    b.classList.toggle('active', b.dataset.kind === active);
  });
  const sel = d.selected;
  button('delete').disabled = sel == null;
  button('lock').disabled = sel == null;
  button('lock').textContent = sel != null && d.isLocked(sel) ? 'Unlock' : 'Lock';
  button('undo').disabled = !d.canUndo;
  button('redo').disabled = !d.canRedo;
}

toolbar.addEventListener('click', (e) => {
  const kind = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-kind]')?.dataset.kind;
  if (!kind) return;
  if (d.activeTool === kind) {
    d.cancelTool();
    lastKind = null;
  } else {
    d.startTool(kind);
    lastKind = kind;
  }
  canvas.focus(); // so that Escape and Enter reach the chart
  refresh();
});

button('delete').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.remove(sel);
};
button('lock').onclick = () => {
  const sel = d.selected;
  if (sel != null) d.setLocked(sel, !d.isLocked(sel));
  refresh();
};
button('undo').onclick = () => d.undo();
button('redo').onclick = () => d.redo();

chart.on('drawingCreated', (_e: ChartEventMap['drawingCreated']) => {
  if (sticky.checked && lastKind && d.activeTool == null) d.startTool(lastKind);
  refresh();
});
chart.on('drawingModified', refresh);
chart.on('drawingRemoved', refresh);
chart.on('drawingSelected', refresh);
// Escape disarms a tool without an event.
canvas.addEventListener('keyup', refresh);
refresh();
```

ボタンから実行したアンドゥとリドゥは、次のフレームの後に`drawingCreated`、`drawingModified`、`drawingRemoved`のいずれかを送り、それによってボタンの状態が更新されます。

## カスタムツール

`chart.drawings.register(kind, tool)`は、JavaScriptで書いた描画ツールを追加します。ツールは作成方法（`anchorCount`：クリック数、`{ open: min }`、または`'freehand'`）を指定し、`build(ctx)`から描画コマンドを返します。任意の`hitTest`で、押した位置が何にヒットするかを決めます。その種類は、このチャートの`add`、`startTool`、`loadState`で使用できます。組み込みの種類は置き換えられません。[プラグイン](plugins.md#drawing-tools)を参照してください。
