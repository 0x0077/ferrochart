# ferrochart

Fast financial charts for Python, written in Rust. Renders on the GPU (Metal,
Vulkan, DirectX 12) with a CPU fallback, in a native window, in Jupyter or to
PNG.

- Millions of bars: pans and zooms at full frame rate.
- Candlestick, OHLC, Heikin-Ashi, line, area, baseline, histogram, points and
  volume series; Renko, range bars, Kagi and point & figure; multiple panes;
  price lines and markers; normal, logarithmic, percentage and indexed price
  scales.
- 30 built-in indicators (`ferrochart.indicator_names()`), updated
  incrementally.
- 29 drawing tools (trend lines, channels, Fibonacci tools, pitchforks, Gann
  fans, patterns, Elliott waves, paths, brushes, texts…) with undo and redo;
  save and restore the view, layout and drawings as JSON.
- Takes pandas or polars DataFrames, or dicts of NumPy arrays.

```bash
pip install ferrochart              # windows and PNG
pip install "ferrochart[jupyter]"   # also the Jupyter widget
```

One wheel per platform serves Python 3.10 and later.

## Quick start

```python
import ferrochart as fc

chart = fc.Chart(theme="dark", width=1200, height=700)
candles = chart.add_series("candlestick")
candles.set_data(df)                 # columns time, open, high, low, close[, volume]
chart.add_volume_series(candles)
chart.add_indicator(candles, "ema", n=20)
chart.add_indicator(candles, "macd")  # in a new pane

chart.show()                         # a window; returns when it is closed
chart.to_png("chart.png")            # offscreen, GPU or CPU
chart                                # in Jupyter: an interactive chart
```

`time` is a `datetime64` column (naive times are UTC) or integer
milliseconds; a pandas `DatetimeIndex` is used when there is no `time` column.
Line, area, baseline, histogram and points series take `time` and `value`,
and optionally `color` per point.

## Real-time data

```python
candles.update_last(t, open=o, high=h, low=l, close=c, volume=v)  # same time: update, later: new bar
candles.append(new_bars)
```

Indicators follow the data. Changes made while the chart is shown are drawn at
once, including changes from other threads.

## Options, indicators and drawings

Options and parameters are keyword arguments; unknown names raise
`ValueError`.

```python
line = chart.add_series("line", color="#ff9800", line_width=2)
candles.apply_options(down_color="#e91e63")
candles.create_price_line(101.5, title="target", line_style="dashed")
candles.set_markers([{"time": t, "position": "below_bar", "shape": "arrow_up", "text": "buy"}])
chart.apply_price_scale_options(mode="logarithmic")

rsi = chart.add_indicator(candles, "rsi", n=14)
boll = chart.add_indicator(candles, "boll", n=20, k=2)

bricks = fc.Chart()                  # a transform series is the only series of its chart
renko = bricks.add_transform_series("renko", box_size=2.0)
renko.set_data(df)                   # OHLC bars in, Renko bricks drawn

d = chart.drawings
trend = d.add("trend_line", [(t0, 100.0), (t1, 120.0)], style={"line": "#e91e63", "line_width": 2})
d.add("fib_retracement", [(t0, 90.0), (t1, 130.0)], params={"levels": [0, 0.382, 0.618, 1]})
d.start_tool("rectangle")            # the next clicks in the window draw one
d.undo()

import json
saved = json.dumps(chart.save_state())   # view, panes, price scales, drawings
chart.load_state(json.loads(saved))      # on a chart set up the same way
```

## Events

```python
@chart.on("click")
def clicked(e):
    print(e["time"], e["price"], e["target"])

chart.on("crosshair_move", lambda e: print(e["point"]))
```

Events: `click`, `crosshair_move`, `visible_range_changed`,
`need_more_history`, `drawing_created`, `drawing_modified`, `drawing_removed`,
`drawing_selected`, `warning`. Callbacks run on the main thread while the
chart is shown.

## Windows

`chart.show()` blocks until the window is closed and can be called again. It
must run on the main thread. On Linux and Windows, `chart.show(block=False)`
opens the window from a background thread and returns at once; event callbacks
then run on the main thread between Python statements, or when you call
`ferrochart.process_events()`. The first `show` of a process fixes the mode.

In the window: drag to pan, wheel or pinch to zoom, drag an axis to scale it,
double-click an axis to reset it. With a tool started, click to place anchors;
Esc cancels, Delete removes the selected drawing, Ctrl/⌘+Z undoes.

## Jupyter

A chart that is the last expression of a cell is shown as an interactive
widget (with `ferrochart[jupyter]`; a PNG otherwise). Later changes, such as
new bars, reach every view of it. `chart.widget()` returns the widget, for
example to put it in an ipywidgets layout.

## Plots

`ferrochart.Plot` draws lines, areas, scatter, bars, waterfalls and heatmaps
on numeric, category or date axes. It is shown the same three ways as a chart:
`plot.show()`, the last expression of a notebook cell, or `plot.to_png()`.

```python
import ferrochart as fc

plot = fc.Plot(width=640, height=400)
plot.axis("x").apply_options(type="category")
plot.axis("x").set_categories(["north", "south", "east"])
plot.add_mark("bars", [4, 6, 5], color="#2196f3")
plot.add_mark("line", {"x": [0, 1, 2], "y": [3, 5.5, 4.5]}, line_width=2)
plot.add_rule(5, label="target", line_style="dashed")
plot.add_label(1, 6, "best", icon="arrow_down")

@plot.on("click")
def clicked(e):
    if e["hit"]:
        print(e["hit"]["mark"], e["hit"]["index"], e["hit"]["y"])

plot.to_png("plot.png")
```

`set_data` takes a DataFrame or dict with columns `x`, `y` and optional `y0`,
`value` (colored by the mark's color scale), `size`, `color`, `step`
(waterfalls) and `label`; a pandas Series (x from its index); a list or array
of y values; or keywords: `mark.set_data(x=xs, y=ys)`. Datetimes in `x` work
with `plot.axis("x").apply_options(format="date")`. Events: `hover`, `click`,
`view_change`.

## License

Apache-2.0.
