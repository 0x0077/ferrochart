<!-- source: 118e97bde8 -->
# オーダーフロー

`ferrochart-orderflow`は、約定と板から構築する3つのシリーズを追加します。これらは[カスタムシリーズ](plugins.md#custom-series)なので、時間軸、価格軸、クロスヘア、ペインをローソク足と共有します。

| シリーズ | 表示内容 | 入力 |
|---|---|---|
| **フットプリント** | ローソク足ごとに、各価格レベルでの売り出来高と買い出来高 | 約定 |
| **ヒートマップ** | 時間列と価格レベルごとの板の厚み（または約定出来高） | 板のスナップショットまたは約定 |
| **TPO**（マーケットプロファイル） | セッションごとに、どの期間がどの価格で取引されたか。POC、バリューエリア、イニシャルバランス | ローソク足 |

3つともすべてのレンダラーで動作します。WebGPUとネイティブGPUでは、ヒートマップはコンピュートシェーダーでセルを集計します。WebGL2とCPUレンダラーではポータブルな処理経路を使い、出力は同じです。

## 共通のルール

- **価格レベル。** 価格は`step = tickSize × rowSize`のグリッドに丸められます。レベルは`round(price / step)`で、0.5は切り上げです。1つのレベルは中心価格の周囲`±step / 2`を占めます。
- **時間列**は、その期間（フットプリントの時間足、ヒートマップの列の長さ）のUTCでの倍数から始まります。
- **約定**は同じ長さのカラムで表します。時刻（UTC ms、非減少）、価格、サイズ（≥ 0。ゼロは無視されます）、および省略可能なサイドです。バッチは最初に検査され、無効ならチャートは変わりません。最後の列に入る約定はその列を更新し、それより後の約定は新しい列を開きます。最後の列より前の約定は拒否されます。
- **アグレッサーサイド。** 買いと売りは与えられたとおりに使われます。不明なサイドはティックルールに従います。直前の約定価格より上なら買い、下なら売り、同じなら直前の約定のサイドです（最初の約定は買いとみなします）。

## フットプリント

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 200 });
const candles = chart.addCandlestickSeries();
const footprint = chart.addFootprint({
  tickSize: 0.25,
  timeframeMs: 60_000,   // the candles' period
  rowSize: 4,            // ticks per row
  mode: 'bidAsk',        // 'bidAsk' | 'delta' | 'volume'
});
footprint.appendTrades({
  time: new Float64Array([...]),
  price: new Float64Array([...]),
  size: new Float64Array([...]),
  side: new Int8Array([...]), // 1 buy, -1 sell, 0 unknown; optional
});
```

描画される内容は足の間隔によって変わります。

| 足の間隔（CSS px） | 描画内容 |
|---|---|
| < 12 | なし（ローソク足は表示されたまま） |
| 12 – 60 | 濃淡を付けたセル |
| ≥ 60 | セルと数値 |

デフォルトの`maxBarSpacing`は50なので、数値が見えるまでズームインしたい場合は、より大きな値でチャートを作成してください。

- **モード。** `bidAsk`：左半分に売り出来高、右半分に買い出来高。`delta`：買い − 売りで、符号によって色分けされます。`volume`：合計。濃淡の強さは同じ列内の最大値に対する相対値です。
- **インバランス**は斜めに比較します。あるレベルの買いが1つ下のレベルの売りの`imbalanceRatio`倍以上なら買いのインバランスです（右端にバー）。あるレベルの売りが1つ上のレベルの買いのratio倍以上なら売りのインバランスです（左端にバー）。デフォルトの比率は3で、`null`にするとインバランスはオフになります。
- **POC。** `showPoc`（デフォルト）では、各列で出来高が最も多いレベルが枠で囲まれます。
- 色はデフォルトでテーマの陽線／陰線／ライン／テキストの色になります。`colors: { buy, sell, volume, text, poc }`で上書きできます。

## ヒートマップ

```js
const book = chart.addHeatmap({
  source: 'depth',       // or 'trades' (fed with appendTrades)
  tickSize: 0.25,
  columnMs: 1_000,       // may be finer than the candles
});
book.appendDepth({ time: Date.now(), prices: bookPrices, sizes: bookSizes });
```

- 列はローソク足のスロットの間に時刻で配置されるため、1秒の列を1分足のローソク足の下に置けます。列がスロットを追加することはありません。
- **板。** 1つのスナップショット内では、同じレベルに入る隣接した価格は合算されます。1つの列内では、各レベルはスナップショット全体での最大サイズを保持します。
- **約定。** サイズは列とレベルごとに合算されます。サイドは使われません。
- **色。** 小さい値から大きい値への5つのカラーストップ（`colors`、アルファ付き）を、`log`（デフォルト）または`linear`のスケールで使います。範囲はデフォルトでは表示中のセルの`{ percentile: 0.99 }`で、パン中に色がちらつかないよう量子化されます。`{ lo, hi }`で固定することもできます。
- 板はローソク足よりはるかに広い範囲に及ぶため、ヒートマップはデフォルトでは自動スケールに参加しません。含めるには`autoScale: true`を設定してください。
- ピクセル行あたり8つより多くのレベルが表示される場合は、何も描画されません。ズームインするか、より大きな`rowSize`を使ってください。

## TPO

```js
const chart = await FerroChart.create(canvas, { maxBarSpacing: 600 });
const tpo = chart.addTpo({
  tickSize: 0.25,
  rowSize: 4,
  periodMs: 30 * 60_000,          // one letter per period (default 30 min)
  sessionOffsetMs: 14.5 * 3_600_000, // New York cash session: 14:30 UTC…
  sessionMs: 6.5 * 3_600_000,        // …for 6.5 hours
});
tpo.appendBars(oneMinuteBars);     // time, high and low are used
```

- **セッション**はUTCの1日に1回、`sessionOffsetMs`に始まり、`sessionMs`の間続きます（1期間から1日まで。デフォルトはUTCの1日）。各セッションは時間軸の1スロットです。どのセッションにも入らない足はスキップされます。
- **期間**にはA–Z、続いてa–zの文字が付きます。足は、その開始時刻が入る期間に属します。リアルタイムの足を再送すると、その期間の範囲が広がります。
- セッションごとの**統計**：
  - **POC**：TPOが最も多いレベル。同数の場合はセッションの中央に最も近いレベル、さらに同じなら低い方のレベルになります。
  - **バリューエリア**：TPOの`valueArea`（デフォルト70%）。POCから、TPOの多い側へ2レベルずつ広げていきます（同数の場合は上へ）。
  - **イニシャルバランス**：最初の2期間のレンジ。
- **描画。** 表示中のすべてのセッションで最も幅の広い行が読める文字サイズに収まる場合は文字、そうでなければヒストグラムで描画します。どちらにするかはフレームごとに1回決まります。イニシャルバランスは左側のバーで表され、バリューエリア外の行は薄く表示されます。

## Rust

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_orderflow::{Footprint, FootprintOptions, Side, TickColumns, append_trades};

let fp = Footprint::new(FootprintOptions::new(0.25, 60_000, &theme))?;
let id = chart.add_custom_series(chart.main_pane(), Box::new(fp), SeriesOptions::default())?;
append_trades(&mut chart, id, &TickColumns {
    time, price, size,
    side: Vec::new(), // all unknown: tick rule
})?;
```

| シリーズ | オプション | データ |
|---|---|---|
| `Footprint` | `FootprintOptions::new(tick_size, timeframe_ms, &theme)` | `append_trades` |
| `Heatmap` | `HeatmapOptions::new(tick_size, column_ms, HeatmapSource::Depth \| Trades)` | `append_depth(&DepthSnapshot)` / `append_trades` |
| `Tpo` | `TpoOptions::new(tick_size, &theme)` | `append_bars(&OhlcColumns)` |

フットプリントの数値とTPOの文字を表示するには`ChartOptions::max_bar_spacing`を設定してください。`ferrochart-render-wgpu`を使う場合は、`ferrochart_orderflow::register_gpu(&ctx,
&mut renderer)`を1回呼びます（`ferrochart-winit`では`ChartApp::on_renderer`の中で）。戻り値は、デバイスがネイティブのヒートマップに対応しているかどうかです。これはデフォルトのフィーチャー`gpu`が提供し、このフィーチャーがなければクレートはwgpuに依存しません。

セッションの統計は`Tpo::profile(i)`（POC、バリューエリア、イニシャルバランス）から、生のセルは`Footprint::grid()`と`Heatmap::grid()`から取得できます。

## パフォーマンス

1,000,000件の約定に対する1分足フットプリントと1秒の約定ヒートマップ（Apple M4 Max、1600×900 CSS px、2倍、ネイティブヒートマップ）の1フレームあたりの時間：

| シナリオ | p50 | p99 |
|---|---|---|
| パン、フットプリントの数値 | 0.47 ms | 1.6 ms |
| パン、フットプリントのセル | 0.65 ms | 1.6 ms |
| ズーム、フットプリント + ヒートマップ | 0.78 ms | 3.3 ms |
| パン、すべて表示 | 3.9 ms | 6.8 ms |
| パン、TPOの文字 | 0.28 ms | 1.1 ms |

最後の列にすでにある価格でのリアルタイム約定、最後の列への板のスナップショット、最後のTPOセッションでのリアルタイムの足は、メモリを一切確保しません。新しい価格レベルと新しい列は、ローソク足の追加と同様にメモリを確保します。

`cargo bench -p ferrochart-bench --bench orderflow`で再現できます。
