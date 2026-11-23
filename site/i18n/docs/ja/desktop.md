<!-- source: 0c5fc454f5 -->
# デスクトップ（Rust + winit）

`ferrochart-winit`はウィンドウを開き、wgpuでレンダリングし、マウス、トラックパッド、キーボードの入力を処理します。渡すのは`Chart`と、必要に応じてコールバックです。

```toml
[dependencies]
ferrochart-core = "0.1"
ferrochart-winit = "0.1"
```

## データを表示するウィンドウ

```rust,ignore
use ferrochart_core::data::OhlcColumns;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_indicators::named::ChartExt; // add_indicator_by_name

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut chart = Chart::new(ChartOptions::default());
    let candles = chart.add_candlestick()?;
    chart.add_volume(candles)?;
    chart.set_data(candles, OhlcColumns {
        time: vec![/* UTC ms */],
        open: vec![], high: vec![], low: vec![], close: vec![],
        volume: Some(vec![]),
    })?;
    chart.add_indicator_by_name(candles, "ema", &[("n", 20.into())])?;
    ferrochart_winit::show(chart)?;
    Ok(())
}
```

`show`はデフォルトのタイトルとサイズでウィンドウを開き、閉じられると戻ります。タイトル、サイズ、コールバック（リアルタイムデータ、イベント）を指定するには、後述の`ChartApp`とともに`run(WindowOptions, chart, app)`を使います。`run`はウィンドウが閉じられると戻ります。チャートは何かが変わったとき（入力、新しいデータ、慣性スクロール）にだけ再描画するので、アイドル状態のチャートはCPUを使いません。

## リアルタイムデータ

`ChartApp::update`を実装します。これはイベントループの反復ごとに呼ばれます。次に呼んでほしい時刻を、受け取ったものと同じミリ秒の時計で返してください。

```rust,ignore
use ferrochart_core::Chart;
use ferrochart_core::data::{BarUpdate, SeriesId};
use ferrochart_winit::ChartApp;

struct Feed { series: SeriesId, bar: BarUpdate, next: f64 }

impl ChartApp for Feed {
    fn update(&mut self, chart: &mut Chart, now_ms: f64) -> Option<f64> {
        if now_ms >= self.next {
            self.bar.close += 0.01;
            self.bar.high = self.bar.high.max(self.bar.close);
            let _ = chart.update_last(self.series, self.bar);
            self.next = now_ms + 10.0; // 100 updates per second
        }
        Some(self.next)
    }
}
```

`ChartApp::on_event`はクロスヘア、表示範囲、履歴のイベントを受け取ります。`ChartApp::on_frame`はレンダリングされた各フレームのCPU時間を受け取ります。

## デモ

```bash
cargo run -p example-desktop --release
```

1000万本の1分足と、毎秒100ティックの模擬フィードです。ドラッグでパン（はじくと慣性スクロール）、ホイールまたはピンチでズーム、軸のドラッグでスケール変更、軸のダブルクリックでリセット、矢印キーとHome/Endで移動します。

## 独自のイベントループ

アプリがすでにウィンドウを所有している場合（egui、ゲームエンジン）は、`ferrochart_winit::InputConverter`でwinitのイベントをチャートの入力に変換し、`Chart::build_frame`と`WgpuRenderer::render`を自分で駆動してください。[レンダリングの仕組み](rendering.md)を参照してください。
