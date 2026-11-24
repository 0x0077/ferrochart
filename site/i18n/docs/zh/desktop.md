<!-- source: 5e934dc0cd -->
# 桌面应用（Rust + winit）

`ferrochart-winit` 负责打开窗口、用 wgpu 渲染，并处理鼠标、触控板和键盘输入。你只需提供一个 `Chart`，以及可选的回调。

```toml
[dependencies]
ferrochart-core = "0.5"
ferrochart-winit = "0.5"
```

## 显示数据的窗口

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

`show` 以默认标题和尺寸打开一个窗口，并在窗口关闭时返回。如需设置标题、尺寸或回调（实时数据、事件），请配合下文的 `ChartApp` 使用 `run(WindowOptions, chart, app)`。`run` 在窗口关闭时返回。图表只在有变化时（输入、新数据、惯性滚动）才重绘，因此空闲的图表不占用 CPU。

内置字体包含拉丁字母、数字和常用符号。要在标记文字、标签和标题中使用中文、日文、韩文或其他文字，请提供后备字体，按顺序查找（不会扫描系统字体；所有字体都没有的字符画成 `□`）：

```rust,ignore
let options = WindowOptions {
    fonts: vec![std::fs::read("/System/Library/Fonts/Hiragino Sans GB.ttc")?],
    ..WindowOptions::default()
};
ferrochart_winit::run(options, chart, app)?;
```

如果使用自己的事件循环，请把它们添加到渲染器：`renderer.text_engine_mut().add_fallback_font(data)`。

## 实时数据

实现 `ChartApp::update`。它在每次事件循环迭代时被调用；返回你希望下一次被调用的时间，使用与它接收到的相同的毫秒时钟：

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

`ChartApp::on_event` 接收十字光标、可见范围和历史数据事件；`ChartApp::on_frame` 接收每个渲染帧的 CPU 耗时。

## 演示

```bash
cargo run -p example-desktop --release
```

一千万根一分钟 K 线，外加每秒 100 个 tick 的模拟行情。拖动以平移（快速甩动可惯性滚动），滚轮或双指缩放以缩放，拖动坐标轴以缩放该轴，双击坐标轴以重置，方向键和 Home/End 用于导航。在触屏上也支持[触屏手势](interaction.md#touch)：winit 的触摸事件会转换为触摸指针。

## 自己的事件循环

如果你的应用已经拥有窗口（egui、游戏引擎），请使用 `ferrochart_winit::InputConverter` 把 winit 事件转换为图表输入，并自行驱动 `Chart::build_frame` 和 `WgpuRenderer::render`——见[渲染的工作原理](rendering.md)。
