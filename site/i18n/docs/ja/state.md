<!-- source: d3368c1d0b -->
# 保存と復元

`chart.saveState()`は、ユーザーが操作によって変更するチャートの部分を返します。ズーム、スクロール位置、ペインの高さ、価格スケールの設定、クロスヘアのモード、描画です。`chart.loadState(state)`でそれらを元に戻します。状態はJSON互換の単純なデータなので、`localStorage`、ファイル、サーバーに保存できます。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

const state: ChartState = chart.saveState();
localStorage.setItem('chart-layout', JSON.stringify(state));
```

## 状態に含まれるもの

| フィールド | 内容 |
|---|---|
| `version` | フォーマットのバージョン。現在は`1` |
| `barSpacing` | CSS px単位の足の間隔（ズームレベル） |
| `rightOffset` | 最後の足の右側の空白。足の本数単位（スクロール位置） |
| `crosshairMode` | `'normal'`、`'magnet'`または`'hidden'` |
| `panes` | ペインごとに1エントリ、上から下へ（後述） |
| `drawings` | 完成したすべての描画、下から上へ（後述） |

`panes`の各エントリ：

| フィールド | 内容 |
|---|---|
| `stretch` | 他のペインに対する相対的な高さ |
| `priceScale.mode` | `'normal'`、`'logarithmic'`、`'percentage'`または`'indexedTo100'` |
| `priceScale.inverted` | 高い価格を下に描画するかどうか |
| `priceScale.margins` | `{ top, bottom }`。ペインの高さに対する割合 |
| `priceScale.range` | 自動スケールがオフのとき（ユーザーが価格軸をドラッグした、または範囲を設定した）は`{ low, high }`。自動スケール中は`null` |

`drawings`の各エントリ：

| フィールド | 内容 |
|---|---|
| `pane` | 上から数えたペインのインデックス（0がメインペイン）。ペインidではありません |
| `visible` | 表示されているかどうか |
| `locked` | 編集できないようロックされているかどうか |
| `data` | `{ kind, anchors, style, params }`。`chart.drawings.get(id)`と同じです |

アンカーは時刻（UTC ms）と価格で保存されるため、データが変わったり過去データを追加で読み込んだりしても、描画は正しい足の上に残ります。

## 含まれないもの

状態が表すのは、ユーザーがチャートをどう配置したかであり、チャートが何を表示しているかではありません。次のものは保存**されません**。

- データ、シリーズとそのオプション
- インジケーター、バンド、マーカー、価格ライン
- レイヤープラグインと登録済みの描画ツール
- テーマ、レンダラー、その他の`FerroChart.create`のオプション
- 同期グループへの参加状態
- 描画のid、選択状態、選択中のツール、作成途中の描画、元に戻す履歴

これらはすべてアプリケーション自身が作成し、その上に状態を読み込みます。再作成に必要な情報（銘柄、時間足、インジケーターの一覧とパラメーター）は状態と一緒に保存してください。

## 状態の読み込み

`loadState`には、保存したときと同じ構成のチャートが必要です。特に**ペインの数が同じ**でなければなりません。ペインは位置で対応付けられるので、ペインを追加するシリーズやインジケーター（たとえばデフォルト配置の`'macd'`や`'rsi'`、または`addPane()`）を、読み込み前に同じ順序で作成してください。データも先に読み込んでおいてください。復元される描画は時刻で配置されます。

```ts
import { FerroChart, randomWalk, type ChartState } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;
declare const saved: ChartState;

const chart = await FerroChart.create(canvas);
const candles = chart.addCandlestickSeries();
candles.setData(randomWalk(1, 2_000, Date.UTC(2024, 0, 1), 60_000));
chart.addVolumeSeries(candles);          // same pane: no new pane
chart.addIndicator(candles, 'macd');     // adds a pane, as before saving

chart.loadState(saved);                  // two panes on both sides
```

読み込みで行われること：

- 足の間隔、右オフセット、クロスヘアのモードが設定されます。
- 各ペインにstretchと価格スケールの設定が適用されます。`range`付きで保存されたペインは、自動スケールをオフにしてその範囲を保ちます。`range: null`で保存されたペインは自動スケールに戻ります。
- 現在の描画はすべて削除され、保存された描画が追加されます。描画には新しいidが付き、チャートはそれらについて`drawingRemoved`と`drawingCreated`イベントを送ります。
- 元に戻す履歴は消去されます。

### JavaScriptで書いた描画ツール

`chart.drawings.register(kind, tool)`で登録した種類の描画は、その種類が登録されているチャートにしか読み込めません。`loadState`を呼ぶ前にツールを登録してください。[描画](drawings.md)と[プラグイン](plugins.md)を参照してください。

## エラー

状態が合わない場合、`loadState`は`Error`を投げ、チャートは変更されません。部分的に適用されることはありません。

| 原因 | メッセージの例 |
|---|---|
| オブジェクトでない | `state must be an object from saveState()` |
| 必須フィールドがない | `state: missing 'barSpacing'` |
| 不明な名前 | `state: unknown crosshair mode 'x'`, `state: unknown price scale mode 'x'` |
| より新しいフォーマットのバージョン | `state version is newer than supported` |
| ペインの数が異なる | `state has a different number of panes` |
| 足の間隔が有限でないか正でない、右オフセットが有限でない | `invalid bar spacing or right offset` |
| マージンが0〜1の範囲外（または合計が1以上）、範囲が有限かつ増加でない、または対数スケールで正でない | `invalid price scale state` |
| 存在しないペインインデックス上の描画 | `drawing on a missing pane` |
| チャートが構築できない描画（不明な種類、アンカーの数の誤り、無効なスタイルやパラメーター） | 描画によって異なります |

正の数でないペインの`stretch`は、エラーにはならず1に置き換えられます。

保存された状態が古くなっている可能性がある場合（たとえばアプリのデフォルトのインジケーターを変更した後）は、エラーを捕捉してデフォルトのレイアウトにフォールバックしてください。

```ts
import { FerroChart, type ChartState } from '@ferrochart/core';

declare const chart: FerroChart;

function restore(chart: FerroChart, json: string | null): boolean {
  if (json === null) return false;
  try {
    chart.loadState(JSON.parse(json) as ChartState);
    return true;
  } catch (e) {
    console.warn('saved layout ignored:', e);
    return false;
  }
}

restore(chart, localStorage.getItem('chart-layout'));
```

## 完全な例

次のページは、銘柄ごとに1つのレイアウトを`localStorage`に保存します。セットアップ関数は状態に含まれないもの（シリーズ、データ、インジケーター、その色）をすべて作成し、残りを状態が復元します。レイアウトは、ユーザーが描画を変更するたびと、ページが非表示になったときに保存されます。

```ts
import { FerroChart, randomWalk, type ChartState, type Series } from '@ferrochart/core';

declare const canvas: HTMLCanvasElement;

/** What the app needs to rebuild a chart; kept next to the chart state. */
interface Layout {
  symbol: string;
  indicators: { name: string; params: Record<string, number | string> }[];
  state: ChartState;
}

const key = (symbol: string) => `layout:${symbol}`;

function loadBars(symbol: string) {
  // Replace with your data source.
  const seed = [...symbol].reduce((s, c) => s + c.charCodeAt(0), 0);
  return randomWalk(seed, 5_000, Date.UTC(2024, 0, 1), 60_000);
}

async function openChart(symbol: string): Promise<FerroChart> {
  const chart = await FerroChart.create(canvas, { theme: 'dark' });

  // 1. Series and data (not part of the state).
  const candles: Series = chart.addCandlestickSeries();
  candles.setData(loadBars(symbol));
  chart.addVolumeSeries(candles);

  // 2. Indicators, in the order they were saved, so the panes match.
  const json = localStorage.getItem(key(symbol));
  const saved: Layout | null = json ? (JSON.parse(json) as Layout) : null;
  const indicators = saved?.indicators ?? [{ name: 'sma', params: { n: 50 } }, { name: 'rsi', params: { n: 14 } }];
  for (const { name, params } of indicators) {
    chart.addIndicator(candles, name, params);
  }

  // 3. The saved view, layout and drawings.
  if (saved) {
    try {
      chart.loadState(saved.state);
    } catch (e) {
      console.warn('saved layout ignored:', e);
    }
  }

  // Save after every drawing change and when the page is hidden.
  const save = () => {
    const layout: Layout = { symbol, indicators, state: chart.saveState() };
    localStorage.setItem(key(symbol), JSON.stringify(layout));
  };
  for (const event of ['drawingCreated', 'drawingModified', 'drawingRemoved'] as const) {
    chart.on(event, save);
  }
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') save();
  });
  return chart;
}

await openChart('BTCUSD');
```

この例の次の点は、実際の運用で重要です。

- インジケーターの一覧は状態と一緒に保存されます。ユーザーがインジケーターを追加または削除するとペインの数が変わり、古い状態は読み込めなくなります。
- `drawingModified`は、ユーザーが描画をドラッグしている間、最大でフレームごとに1回送られます。その頻度で小さな状態を`localStorage`に書き込むのは問題ありませんが、サーバーに保存する場合は呼び出しをデバウンスしてください。
- ズームとスクロール位置は描画イベントなしで変わるため、この例ではページが非表示になったときにも保存しています。もっと早く保存する必要がある場合は`visibleRangeChanged`を監視してください。

## 描画だけを保存する

描画だけを保存するには（たとえばペイン構成の異なる別のチャートにコピーするため）、状態ではなく描画APIから読み出します。

```ts
import { FerroChart, type DrawingData } from '@ferrochart/core';

declare const source: FerroChart;
declare const target: FerroChart;

const drawings: Required<DrawingData>[] = source.drawings
  .ids()
  .filter((id) => source.drawings.paneOf(id) === source.mainPane)
  .map((id) => source.drawings.get(id))
  .filter((d): d is Required<DrawingData> => d !== null);

for (const d of drawings) target.drawings.add(d);   // to the main pane
```

開いているチャート同士で描画を連動させたい場合は、[チャートの同期](sync.md)を参照してください。
