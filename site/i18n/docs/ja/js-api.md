<!-- source: 8cc76ebe6c -->
# JavaScript APIの概要

パッケージ`@ferrochart/core`。このページでは、モジュールの読み込みと、API全体に共通する規約を扱います。すべてのクラス、メソッド、型を網羅した完全なリファレンスは、パッケージのTypeScript宣言から生成されています：[JavaScript APIリファレンス](https://ferrochart.com/docs/api)。

## 読み込み

```ts
import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
```

`FerroChart.create`と`FerroPlot.create`は、最初に呼ばれたときにWebAssemblyモジュールを読み込みます。同時に呼ばれた場合は1回の読み込みを共有します。チャートを作成する前にモジュールが必要な場合（`randomWalk`、`new SyncGroup()`、`indicatorNames()`）や、`.wasm`ファイルを別のURLから読み込む場合は、デフォルトエクスポートの`init`を自分で呼んでください。

```ts
import init, { SyncGroup } from '@ferrochart/core';

await init();                                     // next to the module (default)
await init({ module_or_path: '/assets/ferrochart_wasm_bg.wasm' });
const group = new SyncGroup();
```

`init`は何度でも呼べます。最初の読み込みの後はすぐに戻ります。

### TypeScript

型宣言が同梱されています。宣言は`Symbol.dispose`を使うので、`compilerOptions.lib`に`"ESNext.Disposable"`を追加するか、よく使われる`skipLibCheck: true`を維持してください。

### バンドラーとフレームワーク

パッケージはESモジュールで、自身のURLを基準に`.wasm`ファイルを取得します（`new URL('…_bg.wasm', import.meta.url)`）。これはVite、webpack 5、Rollup、esbuildが解釈できます。チャートはブラウザーでのみ作成してください。Reactではエフェクトの中で、Vueでは`onMounted`の中で作成し、コンポーネントのサーバーレンダリングはスキップします（[ReactとVue](frameworks.md)を参照）。

## クラス

| クラス | 作成方法 | 概要 |
|---|---|---|
| [`FerroChart`](https://ferrochart.com/docs/api/FerroChart) | `FerroChart.create(canvas, options?)` | `<canvas>`上の金融チャート。ペイン、シリーズ、スケール、クロスヘア、描画 |
| [`Series`](https://ferrochart.com/docs/api/Series) | `chart.addSeries`、`addCandlestickSeries`、`addVolumeSeries`、`addTransformSeries`、オーダーフローの`add*` | 1つのデータシリーズ |
| [`Indicator`](https://ferrochart.com/docs/api/Indicator) | `chart.addIndicator` | インジケーターとその出力シリーズ |
| [`PriceLine`](https://ferrochart.com/docs/api/PriceLine) | `series.createPriceLine` | ある価格の水平線 |
| [`Drawings`](https://ferrochart.com/docs/api/Drawings) | `chart.drawings` | チャートの描画と描画ツール |
| [`SyncGroup`](https://ferrochart.com/docs/api/SyncGroup) | `new SyncGroup()` | 複数のチャートのクロスヘア、表示範囲、描画を連動させます |
| [`FerroPlot`](https://ferrochart.com/docs/api/FerroPlot) | `FerroPlot.create(canvas, options?)` | 汎用のx/yプロット（数値、カテゴリー、日付） |
| [`Mark`](https://ferrochart.com/docs/api/Mark), [`Axis`](https://ferrochart.com/docs/api/Axis), [`Rule`](https://ferrochart.com/docs/api/Rule), [`Label`](https://ferrochart.com/docs/api/Label) | `FerroPlot`のメソッド | プロットの構成要素 |

## 規約

### 時刻

時刻は **UTCミリ秒**です。カラムデータ（`setData`の`time`）は`number[]`、`Float64Array`、`BigInt64Array`を受け付けます。それ以外で単一の時刻を渡す場所（行、`update`、マーカー、描画のアンカー、`indexOfTime`）では、`number`、`bigint`、`Date`のいずれも使えます。時刻は狭義単調増加でなければなりません。イベントとゲッターは時刻を`number`（データの範囲外では`null`）で返します。時間軸のラベルはUTCです。

### 論理インデックス

足は**論理インデックス**で指定します。初期データの最初の足が0、次が1、という具合です。後から先頭に追加された足には負のインデックスが付くため、一度付いたインデックスは変わりません。表示範囲、クロスヘア、変換メソッド（`coordinateToLogical`、`timeAtIndex`、`indexOfTime`）はこのインデックスを使います。小数の値は足の中心と中心の間の位置です。

### 座標

位置は、DOMイベントと同様に、canvasの左上隅からの **CSSピクセル**です。canvasのサイズはCSSで指定します。チャートはバッキングストアをCSSサイズ × `devicePixelRatio`に設定し、サイズの変化にも自動で追従します。

### 色

`Color`はCSSのカラー文字列です：`#rgb`、`#rrggbb`、`#rrggbbaa`、`rgb()`、`rgba()`、または`'transparent'`。オプションでは、`null`はテーマのデフォルトを意味します。

### オプションオブジェクト

オプションは単純なオブジェクトです。`applyOptions`などのメソッドは、渡したキーだけを変更し、渡さなかったキーは値を保ちます。`series.options()`などのゲッターはすべてのキーを返します（`Required<…>`）。

### エラー

メソッドは、問題を示すメッセージ付きの`Error`を投げます（不明なインジケーター名、無効なパラメーター、長さの異なるカラム、時刻が増加していない足、不明なidなど）。チャートは変更されません。要求したGPU APIが利用できない場合、`FerroChart.create`はrejectされます。Canvas 2Dへのフォールバックはなく、`renderer: 'auto'`はWebGPU、次にWebGL2を試します。

### イベント

`chart.on(name, callback)`で購読し、`chart.off(name, callback)`で購読を解除します。コールバックの引数はイベント名から型付けされます。イベントは入力ハンドラーの中ではなく、それを生じたフレームの後に配信されます。コールバックが投げた例外は（`reportError`を通じて）コンソールに報告され、他のコールバックはそのまま実行されます。[クロスヘアとイベント](crosshair-events.md)を参照してください。

### ライフタイムとメモリ

チャートはcanvasとwindowを監視し、自分で再描画します。ページ上にある間はチャートへの参照を保持し、取り除くときは`chart.destroy()`を呼んでください。リスナーが削除され、GPUメモリが解放されます。`destroy`せずに手放したチャートはガベージコレクションされ、応答しなくなることがあります。

APIのすべてのオブジェクトはWebAssemblyのメモリを保持しています。チャートが返すオブジェクト（`Series`、`Indicator`、`PriceLine`）はチャートとともに解放されます。`free()`または`using`宣言で、個別に早く解放できます。シリーズの削除（`remove()`）はこれとは別で、シリーズをチャートから取り除く操作です。

### canvasで処理される入力

ポインター（マウス、ペン、タッチ）、ホイール、キーボード（canvasには`tabindex="0"`が付きます）、ダブルクリック、トラックパッドのピンチ（ChromeとFirefoxではCtrl+ホイール、Safariではジェスチャーイベント）です。canvasのスタイルには`touch-action: none`が設定されます。各ジェスチャーの動作は[ジェスチャーとキーボード](interaction.md)を参照してください。

## テストデータ

`randomWalk(seed, n, startMs, intervalMs)`は決定的なOHLCカラムを返します。Rustのジェネレーター`ferrochart_core::testing::random_walk`と同一の結果です。サンプルとデモで使われています。

```ts
import init, { randomWalk } from '@ferrochart/core';

await init();
const bars = randomWalk(42, 10_000, Date.UTC(2024, 0, 1), 60_000);
```
