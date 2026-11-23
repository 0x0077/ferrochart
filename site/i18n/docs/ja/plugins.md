<!-- source: 69f10455ac -->
# プラグイン

FerroChartにはチャートを拡張する方法が5つあります。いずれもフレームに描画コマンドを書き込むため、すべてのレンダラー（WebGPU、WebGL2、Vulkan/Metal/DX12、CPUレンダラー）で動作し、組み込みのシリーズと同じようにスクリーンショットやゴールデンイメージにも現れます。

| 拡張 | 用途 | Rust | JavaScript |
|---|---|---|---|
| レイヤープラグイン | ペインやチャート全体への追加描画：ウォーターマーク、売買マーク、セッションの背景色 | `LayerPlugin` | `chart.addLayer` |
| カスタムシリーズ | 自身のデータを保持して描画するシリーズ：プロファイル、ヒートマップ、レンジボックス | `CustomSeries` | — |
| ネイティブGPUシリーズ | カスタムシリーズの種類に対するGPU実装（コンピュートシェーダー、大規模データ） | `GpuSeries` | — |
| シリーズ変換 | 足から計算される新しいOHLC風シリーズ：Renko系のチャート | `SeriesTransform` | — |
| 描画ツール | アンカー、ハンドル、ヒットテストを持つ、ユーザーが描くオブジェクト | `DrawingTool` | `chart.drawings.register` |

実行可能なRustのサンプルは`examples/plugins`（`cargo run -p example-plugins
--release`）です。ウォーターマーク、売買マーク、シミュレートされたフィードで更新されるカスタムシリーズが含まれています。オーダーフローのクレート（`ferrochart-orderflow`、[オーダーフロー](orderflow.md)を参照）は最大のプラグインで、Rustのすべての拡張ポイントを使っています。

## すべてのプラグインが従うルール

- **毎フレーム、アロケーションなしで描画する。** `build`は、ユーザーがパンやズームをしている間も含めて毎フレーム呼び出されます。渡された`FrameBuilder`に書き込み（そのプールは容量を保持します）、独自の作業用バッファーはプラグイン内に持ってクリアして再利用してください。表示されている部分だけを描画します。表示範囲は引数で渡されます。
- **パニックしない。** プラグインはチャートのフレーム処理の中で実行されます。代わりに早期リターンしてください。
- **座標は、Rustでは物理ピクセル**（`Px`、`PointPx`、`RectPx`）、JavaScriptではCSSピクセルです。Rustでは`ctx.px(Css(…))`でCSSからサイズを変換します。
- **色はスタイルかオプションから取る。** `ctx.style()`（テーマに基づいて解決されたシリーズの色）か、オプションとして受け取った色を使ってください。そうすればプラグインがライトテーマとダークテーマに追従します。
- **変更をチャートに伝える。** 内容が変わるレイヤーは`invalidate_layer`を呼びます。カスタムシリーズは`update_custom`を通してのみデータを変更します。

## レイヤープラグイン

レイヤーは1つのペイン（そのプロット領域）またはチャート全体に、4つの深さのいずれかで描画します。

| `LayerPosition` | 描画位置 |
|---|---|
| `Background` | グリッドの上、他のすべての背後 |
| `BelowSeries` | シリーズの下 |
| `AboveSeries` | シリーズとマーカーの上、描画オブジェクトとラベルの下 |
| `Overlay` | オーバーレイレイヤー上、クロスヘアの下 |

```rust
use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::TextAlign;
use ferrochart_core::geom::{Css, PointPx, Rgba};
use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};

struct Watermark(&'static str);

impl LayerPlugin for Watermark {
    fn position(&self) -> LayerPosition {
        LayerPosition::Background
    }

    fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
        let r = ctx.rect();
        let center = PointPx { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
        let label = Label {
            font: ctx.font(Css(48.0)),
            color: Rgba::hex(0x80_8080).with_alpha(0x40),
            align: TextAlign::Center,
            bg: None,
        };
        out.text(center, self.0, label);
    }
}

// None: the whole chart; Some(pane): that pane's plot.
let id = chart.add_layer(None, Box::new(Watermark("BTCUSD")))?;
```

ペイン上では、`ctx.coords()`がそのペインの時刻と価格の変換（`x_of`、`y_of`、`time_at`、`price_at`、`px`、`plot`、価格のフォーマット）を提供します。サンプルの`TradeMarks`のように、`time_at(plot.x)`と`time_at(plot.right())`と二分探索を使って、データのうち表示されている部分を見つけてください。

レイヤー自身のデータが変わったら、次のフレームで再描画されるように`chart.invalidate_layer(id)`を呼びます。`chart.remove_layer(id)`でレイヤーを削除します。

### JavaScriptの場合

```js
const id = chart.addLayer({
  pane: 0,                  // omit for the whole chart
  position: 'aboveSeries',  // 'background' | 'belowSeries' | 'aboveSeries' | 'overlay'
  build(ctx) {
    const x = ctx.timeToX(entryTime);
    const y = ctx.priceToY(entryPrice);
    if (x === null || y === null) return [];
    return [
      { type: 'line', x1: ctx.rect.x, y1: y, x2: x, y2: y, color: '#2962FF', dash: 'dashed' },
      { type: 'text', x: x + 4, y, text: ctx.formatPrice(entryPrice), color: '#2962FF', align: 'left' },
    ];
  },
});
chart.invalidateLayer(id); // after the plugin's own data changed
chart.removeLayer(id);
```

`build(ctx)`は、canvasの左上隅を原点とするCSSピクセル単位のコマンドの配列を返します。コマンドは`rect`、`line`、`polyline`、`polygon`、`text`、`icon`です（各フィールドは`PluginCommand`型に記載されています）。`ctx`の関数（`timeToX`、`priceToY`、`xToTime`、`yToPrice`、`textWidth`、`formatPrice`）は呼び出し中にのみ動作します。例外をthrowしたり無効なコマンドを返したりしたプラグインは、そのフレームでは何も描画しません（不正なコマンドより前のコマンドは残ります）。最初のエラーは`console.warn`で出力されます。

JavaScriptのレイヤーはフレームごとに1回、wasmの境界を越えて呼び出されるため、コマンド数は表示されている分（数十万ではなく数百程度）に抑えてください。大規模データには、Rustでカスタムシリーズを書いてください。

## カスタムシリーズ

カスタムシリーズは自身のデータを保持します。チャートはそのシリーズに時刻と価格の極値を問い合わせ、時間軸上に配置し、価格軸をそれに合わせてスケールし、表示されているポイントを描画させます。

```rust
use ferrochart_core::chart::SeriesOptions;
use ferrochart_core::data::{DataChange, Timestamp};
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::geom::RectPx;
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};

/// A vertical bar from `lo` to `hi` per point.
#[derive(Default)]
struct Ranges {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl Ranges {
    fn push(&mut self, t: Timestamp, lo: f64, hi: f64) -> DataChange {
        self.time.push(t);
        self.range.push((lo, hi));
        let _ = self.index.push(lo, hi);
        DataChange::Append { count: 1 }
    }
}

impl CustomSeries for Ranges {
    fn kind(&self) -> &'static str {
        "example.ranges"
    }
    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }
    fn times(&self) -> &[Timestamp] {
        &self.time
    }
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        let w = (ctx.bar_spacing().0 * 0.6).max(1.0);
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) =
                (ctx.x_of_point(i), ctx.coords().y_of(hi), ctx.coords().y_of(lo))
            else {
                continue;
            };
            out.rect(RectPx { x: x - w / 2.0, y: y0, w, h: (y1 - y0).max(1.0) }, ctx.style().up);
        }
    }
}

let id = chart.add_custom_series(chart.main_pane(), Box::new(Ranges::default()), SeriesOptions::default())?;
chart.update_custom(id, |r: &mut Ranges| r.push(60_000, 1.0, 2.0))?;
let ranges: Option<&Ranges> = chart.custom_series::<Ranges>(id);
```

### 契約

| メソッド | 呼び出されるタイミング | 守るべきこと |
|---|---|---|
| `kind` | 追加時 | `"myorg.profile"`のような一意の名前を返す。ネイティブGPU実装はこれで照合されます |
| `axis_mode` | 追加時 | `Slots`または`Overlay`（後述）を返す |
| `times` | 変更のたび | 狭義単調増加の時刻（UTC ms）を返す。ポイント`i`は`times()[i]` |
| `extremes(from, to)` | 毎フレーム | ポイント`from..to`の価格範囲をO(log n)で返す。`MinMaxIndex`を使います |
| `build(ctx, from, to, out)` | 毎フレーム | ポイント`from..to`（表示されているものと両側に1つずつ）をアロケーションなしで描画する |
| `last_value` | 毎フレーム | 最終値ラベルの値（`None`：ラベルなし） |
| `crosshair_value(i)` | クロスヘアの移動時 | ポイント`i`について表示する値 |
| `native_params` | 毎フレーム（ネイティブパスのみ） | GPU実装向けに最大8個のパラメーターを書き込む |

**軸モード。** `Slots`：組み込みシリーズの足と同じように、各ポイントが時間軸の1スロットになります（ローソク足ごとのフットプリント列など）。`Overlay`：ポイントは時刻に応じて既存のスロットの間に配置され、スロットを追加しません（1分足チャート上の1秒ごとのヒートマップ列など）。

**データの変更。** プラグインの変更は、何が変わったかを返す`chart.update_custom(id, |s: &mut MySeries| { …; change })`を通してのみ行います。

| `DataChange` | 意味 |
|---|---|
| `UpdateLast` | 最後のポイントが変わったが、その時刻は変わっていない（アロケーションなし：リアルタイムのパス） |
| `Append { count }` | 末尾に`count`個のポイントが追加された（`count: 0`：変更なし） |
| `Prepend { count }` | 先頭に`count`個のポイントが追加された（履歴） |
| `Replace` | それ以外のすべて |

データと一致しない変更（個数の誤り、最後の時刻の移動）は`Replace`として適用されます。そのため、誤った申告は時間を無駄にするだけで、チャートを壊すことはありません。時刻が狭義単調増加でなくなった場合は`TimeNotIncreasing`が返され、有効な変更が行われるまでシリーズには何も表示されません。

**極値。** `MinMaxIndex`は範囲の最小値・最大値のクエリにO(log n)で答え、`push` / `set_last`で最新の状態に保たれます。シリーズごとに1つ使い、データを変更するのと同じクロージャの中で更新してください。

**描画。** `ctx.x_of_point(i)`はポイント`i`の中心です（`SeriesOptions::shift`を反映）。`ctx.coords()`は価格と時刻を変換し、フォントを提供し、価格をフォーマットします。`ctx.bar_spacing()`はスロット間の距離で、`ctx.style()`はシリーズの色を保持します。`FrameBuilder`は矩形、線、ポリライン、ポリゴン、テキスト、アイコンを書き込みます。整数ピクセルにスナップした矩形は、どのデバイスピクセル比でもシャープに見えます。

カスタムシリーズは変換シリーズとチャートを共有できません（どちらも時間軸の構築方法を決めるためです）。

## ネイティブGPUシリーズ

ポータブルな`build`パスはどこでも動作します。シリーズのデータが多すぎる場合（たとえば100万セルのヒートマップ）、代わりにネイティブ実装でGPU上に描画できます。その場合、フレームにはそのシリーズのプリミティブの代わりに`DrawCmd::Custom`が1つだけ入ります。

```rust
use ferrochart_render_wgpu::{GpuSeries, GpuSeriesCtx};
use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::CustomDraw;

struct MySeriesGpu { /* pipelines, per-series buffers */ }

impl GpuSeries for MySeriesGpu {
    fn kind(&self) -> &'static str {
        "myorg.profile" // the CustomSeries::kind it draws
    }
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        // Upload what changed since the last frame:
        //   data.changes_since(draw.series, synced_generation)
        // Read the plugin: data.custom(draw.series), downcast through &dyn Any.
        // Compute work may be submitted to ctx.queue here.
    }
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw) {
        // Set the pipeline and bind groups and draw; the scissor is already
        // the pane's plot. Do not change the scissor or viewport.
    }
}

renderer.register_series(Box::new(MySeriesGpu { /* … */ }));
// Before each frame (the winit and web hosts do this for you):
chart.set_native_kinds(renderer.native_kinds());
```

`ferrochart-winit`では、GPUレンダラーが作成されるたびに呼ばれる`ChartApp::on_renderer`の中で登録します。

ネイティブ実装のルール：

- **ポータブルパスと同じ出力にする。** レイアウト、しきい値、色の範囲はプラグイン（コア側）で計算し、`native_params`と`data.custom`を通して渡します。シェーダーはそれを適用するだけです。両方のパスを同じゴールデンイメージでテストしてください。
- **デバイスを確認する。** WebGL2にはコンピュートシェーダーもストレージバッファーもありません。必要な機能をデバイスがサポートしている場合にだけ登録してください。登録されていない種類は自動的にポータブルパスを使います。
- **インクリメンタルにアップロードする。** `CustomDraw::generation`と`data.changes_since`で何が変わったかがわかります。リアルタイム更新は通常、最後のポイントにしか触れません。

`ferrochart_orderflow::register_gpu`は完全な実装例です。コンピュートシェーダーがヒートマップのセルをピクセルごとに集計し、1枚のクアッドで色を付けます。

## シリーズ変換

変換は、組み込みのRenko、レンジバー、Kagi、ポイント・アンド・フィギュアと同じように、メインシリーズの足を新しいOHLC風のシリーズに変換します。入力足ごとに1回呼び出され、`peek`はまだ形成中の最後の足から何が生成されるかを示します。

| メソッド | 守るべきこと |
|---|---|
| `name` | `"renko"`のような名前を返す |
| `output_kind` | `Candlestick`、`OhlcBar`、`Kagi`、`PointFigure`のいずれかを返す |
| `push(bar, out)` | `bar`を確定し、それによって完成する要素を`out`に書き込む |
| `peek(bar, out)` | 状態を変えずに、`bar`によって完成する要素と、まだ形成中の要素を書き込む |
| `reset` | すべての入力を破棄する |
| `box_size` | ポイント・アンド・フィギュアのみ：1ボックスの価格の高さ |

```rust
let id = chart.add_transform_series(pane, Box::new(MyTransform::new()), SeriesOptions::default())?;
chart.set_data(id, SeriesData::Ohlc(bars))?;   // input bars
chart.update_last(id, tick)?;                   // live input
```

変換シリーズのx軸は時間ではなく自身の出力なので、他の時間ベースのシリーズとチャートを共有できません。

## 描画ツール

描画ツールは、ユーザーがマウスで配置するオブジェクトです。アンカー（時刻、価格）、編集用のハンドル、ヒットテスト、スタイル、そしてアンドゥ/リドゥや状態ファイルのための保存形式を持ちます。

### Rustの場合

`DrawingTool`（`kind`、`anchors`、`anchors_mut`、`anchor_count`、`build`、`hit_test`、`handles`、`style`、`save`、任意で`move_handle`）を実装し、保存形式から再構築するファクトリーを登録します。

```rust
fn my_tool(data: &DrawingData) -> ferrochart_core::Result<Box<dyn DrawingTool>> {
    Ok(Box::new(MyTool::from_data(data)?))
}

chart.drawings().register("myorg.zone", my_tool);
```

`AnchorCount`で作成方法が決まります。固定回数のクリック、ダブルクリックまたはEnterで終了する可変個数、またはフリーハンドです。組み込みツールは`ferrochart_core::drawing::BUILTIN_KINDS`に列挙されており、よい参考になります。

### JavaScriptの場合

```js
chart.drawings.register('myorg.zone', {
  anchorCount: 2, // or { open: 3 } or 'freehand'
  build(ctx) {
    const [a, b] = ctx.anchors;
    if (a.x === null || b.x === null) return [];
    return [{
      type: 'rect',
      x: Math.min(a.x, b.x), y: Math.min(a.y, b.y),
      width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y),
      color: ctx.style.fill ?? 'rgba(41, 98, 255, 0.2)',
    }];
  },
  // Optional; default: hit inside the drawn bounds.
  hitTest(ctx, point) {
    return false; // true: the drawing; a number: that anchor's handle
  },
});
chart.drawings.startTool('myorg.zone');
```

JavaScriptの種類は、`drawings.register`を呼び出したチャートで（`add`、`startTool`、`loadState`に）使用できます。必要なチャートごとに登録してください。同じ種類を再度登録すると、ページ内のすべてのチャートでその実装が置き換えられます。組み込みの種類は置き換えられません。

## プラグインのテスト

- **ゴールデンイメージ。** `ferrochart-render-cpu`（決定的で、GPU不要）で固定のシーンを描画し、保存済みのPNGと比較します。ネイティブ実装がある場合は`ferrochart-render-wgpu`でも描画し、両方のパスを同じ画像と比較します。
- **アロケーションなし。** 数フレームのウォームアップの後、パン、ズーム、クロスヘアの移動（`build_frame`）と、最後のポイントの更新の間のアロケーション回数を数えます。回数はゼロであるべきです。カウント用アロケーターについては`ferrochart-orderflow/tests/zero_alloc.rs`を参照してください。
- **ヘッドレスのフレーム。** `ferrochart_core::testing::MonoMeasurer`（`testing`フィーチャー）を使うと、フォントやGPUなしでユニットテストから`chart.build_frame`を呼び出せます。
