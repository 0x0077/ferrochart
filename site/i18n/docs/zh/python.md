<!-- source: acdcedb855 -->
# Python

`ferrochart` 包可以在 Python 中绘制图表，输出到原生窗口、Jupyter 或 PNG，使用 GPU（Metal、Vulkan、DirectX 12）渲染，并提供 CPU 回退。它接受 pandas 或 polars DataFrame，或由 NumPy 数组组成的 dict。

```bash
pip install ferrochart              # windows and PNG
pip install "ferrochart[jupyter]"   # also the Jupyter widget
```

每个平台一个 wheel，支持 Python 3.10 及以上版本。

## 快速上手

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

`time` 是一个 `datetime64` 列（无时区的时间视为 UTC）或整数毫秒；没有 `time` 列时使用 pandas 的 `DatetimeIndex`。折线、面积、基线、柱状图和散点系列接受 `time` 和 `value`，以及可选的逐点 `color`。

## 实时数据

```python
candles.update_last(t, open=o, high=h, low=l, close=c, volume=v)  # same time: update, later: new bar
candles.append(new_bars)
```

指标会跟随数据更新。图表显示期间所做的修改会立即绘制，包括来自其他线程的修改。

## 选项、指标与画线

选项和参数都是关键字参数；未知名称会引发 `ValueError`。

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

## 事件

```python
@chart.on("click")
def clicked(e):
    print(e["time"], e["price"], e["target"])

chart.on("crosshair_move", lambda e: print(e["point"]))
```

事件：`click`、`crosshair_move`、`visible_range_changed`、`need_more_history`、`drawing_created`、`drawing_modified`、`drawing_removed`、`drawing_selected`、`warning`。图表显示期间，回调在主线程上运行。

## 窗口

`chart.show()` 会阻塞直到窗口关闭，并且可以再次调用。它必须在主线程上运行。在 Linux 和 Windows 上，`chart.show(block=False)` 会从后台线程打开窗口并立即返回；此时事件回调在主线程上、在 Python 语句之间运行，或在你调用 `ferrochart.process_events()` 时运行。进程中第一次 `show` 决定采用哪种模式。

在窗口中：拖动以平移，滚轮或双指缩放以缩放，拖动坐标轴以缩放该轴，双击坐标轴以重置。启动某个工具后，点击放置锚点；Esc 取消，Delete 删除选中的画线，Ctrl/⌘+Z 撤销。

## Jupyter

作为单元格最后一个表达式的图表会显示为交互式 widget（需要 `ferrochart[jupyter]`；否则显示为 PNG）。之后的修改（例如新的 K 线）会同步到它的每一个视图。`chart.widget()` 返回该 widget，例如可以把它放进 ipywidgets 布局中。

## 绘图

`ferrochart.Plot` 可以在数值、类别或日期坐标轴上绘制折线、面积、散点、柱形、瀑布图和热力图。它与图表一样有三种显示方式：`plot.show()`、作为 notebook 单元格的最后一个表达式，或 `plot.to_png()`。

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

`set_data` 接受：带有 `x`、`y` 列以及可选 `y0`、`value`（按标记的颜色刻度着色）、`size`、`color`、`step`（瀑布图）和 `label` 列的 DataFrame 或 dict；pandas Series（x 取自其索引）；由 y 值组成的列表或数组；或关键字参数：`mark.set_data(x=xs, y=ys)`。`x` 中的日期时间配合 `plot.axis("x").apply_options(format="date")` 使用。事件：`hover`、`click`、`view_change`。
