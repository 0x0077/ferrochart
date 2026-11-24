# Desktop (Rust + winit)

`ferrochart-winit` opens a window, renders with wgpu and handles mouse,
trackpad and keyboard input. You give it a `Chart` and, optionally, callbacks.

```toml
[dependencies]
ferrochart-core = "0.5"
ferrochart-winit = "0.5"
```

## A window with data

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

`show` opens a window with the default title and size and returns when it is
closed. For a title, a size or callbacks (live data, events), use
`run(WindowOptions, chart, app)` with a `ChartApp`, below. `run` returns when
the window is closed. The chart redraws only when something
changed (input, new data, kinetic scrolling), so an idle chart uses no CPU.

The bundled font covers Latin letters, digits and common symbols. For Chinese,
Japanese, Korean or other scripts in marker texts, labels and titles, give
fallback fonts, searched in order (no system fonts are scanned; a character
that no font has is drawn as `□`):

```rust,ignore
let options = WindowOptions {
    fonts: vec![std::fs::read("/System/Library/Fonts/Hiragino Sans GB.ttc")?],
    ..WindowOptions::default()
};
ferrochart_winit::run(options, chart, app)?;
```

With your own event loop, add them to the renderer:
`renderer.text_engine_mut().add_fallback_font(data)`.

## Live data

Implement `ChartApp::update`. It is called on every event-loop iteration; return
when you want to be called next, on the same millisecond clock it receives:

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

`ChartApp::on_event` receives crosshair, visible-range and history events;
`ChartApp::on_frame` receives the CPU time of each rendered frame.

## The demo

```bash
cargo run -p example-desktop --release
```

Ten million one-minute bars with a simulated feed of 100 ticks per second.
Drag to pan (flick for kinetic scrolling), wheel or pinch to zoom, drag an axis
to scale it, double-click an axis to reset it, arrow keys and Home/End to
navigate. On a touch screen the [touch gestures](interaction.md#touch) work
too: winit's touch events become touch pointers.

## Your own event loop

If your app already owns the window (egui, a game engine), use
`ferrochart_winit::InputConverter` to turn winit events into chart input, and
drive `Chart::build_frame` and `WgpuRenderer::render` yourself — see
[How rendering works](rendering.md).
