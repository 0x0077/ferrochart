<!-- source: ebac567ea9 -->
# 桌面（Rust + winit）

`ferrochart-winit` 會開啟視窗、以 wgpu 渲染，並處理滑鼠、觸控板與鍵盤輸入。你提供一個 `Chart`，也可以選擇性地提供回呼。

```toml
[dependencies]
ferrochart-core = "0.6"
ferrochart-winit = "0.6"
```

## 帶有資料的視窗

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

`show` 會以預設標題與尺寸開啟視窗，並在視窗關閉時返回。若需要標題、尺寸或回呼（即時資料、事件），請搭配下方的 `ChartApp` 使用 `run(WindowOptions, chart, app)`。`run` 在視窗關閉時返回。圖表只有在有所變化時（輸入、新資料、慣性捲動）才會重繪，因此閒置的圖表不占用 CPU。

內建字型涵蓋拉丁字母、數字與常用符號。若標記文字、標籤與標題中有中文、日文、韓文或其他文字，請提供備援字型，依序查找（不會掃描系統字型；所有字型都沒有的字元會畫成 `□`）：

```rust,ignore
let options = WindowOptions {
    fonts: vec![std::fs::read("/System/Library/Fonts/Hiragino Sans GB.ttc")?],
    ..WindowOptions::default()
};
ferrochart_winit::run(options, chart, app)?;
```

若使用自己的事件迴圈，請將字型加入渲染器：`renderer.text_engine_mut().add_fallback_font(data)`。

## 即時資料

實作 `ChartApp::update`。它在每次事件迴圈迭代時被呼叫；請傳回你希望下次被呼叫的時間，使用與它所接收之時間相同的毫秒時鐘：

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

`ChartApp::on_event` 接收十字游標、可見範圍與歷史資料事件；`ChartApp::on_frame` 接收每個已渲染影格的 CPU 時間。

## 示範程式

```bash
cargo run -p example-desktop --release
```

一千萬根一分鐘 K 棒，搭配每秒 100 筆 tick 的模擬資料源。拖曳以平移（快速甩動可慣性捲動），滾輪或雙指縮放以縮放，拖曳座標軸以調整其刻度，雙擊座標軸以重設，使用方向鍵與 Home/End 瀏覽。在觸控螢幕上也可使用[觸控手勢](interaction.md#touch)：winit 的觸控事件會轉換為觸控指標。

## 自行管理事件迴圈

若你的應用程式已經擁有視窗（egui、遊戲引擎），請使用 `ferrochart_winit::InputConverter` 把 winit 事件轉換為圖表輸入，並自行驅動 `Chart::build_frame` 與 `WgpuRenderer::render`——參見[渲染的運作方式](rendering.md)。
