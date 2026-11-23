<!-- source: 4bfbed7d9a -->
# Python

`ferrochart` 套件讓你從 Python 在原生視窗、Jupyter 中繪製圖表或輸出為 PNG，使用 GPU（Metal、Vulkan、DirectX 12）並具備 CPU 備援。它接受 pandas 或 polars DataFrame，或由 NumPy 陣列組成的 dict。

```bash
pip install ferrochart              # windows and PNG
pip install "ferrochart[jupyter]"   # also the Jupyter widget
```

每個平台一個 wheel，支援 Python 3.10 及更新版本。

## 快速入門


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

`time` 是 `datetime64` 欄（無時區的時間視為 UTC）或整數毫秒；沒有 `time` 欄時，會使用 pandas 的 `DatetimeIndex`。折線、面積、基準線、直方圖與點系列接受 `time` 與 `value`，並可選擇性地提供逐點的 `color`。

## 即時資料

```python
candles.update_last(t, open=o, high=h, low=l, close=c, volume=v)  # same time: update, later: new bar
candles.append(new_bars)
```

指標會跟隨資料更新。圖表顯示期間所做的變更會立即繪出，包括來自其他執行緒的變更。

## 選項、指標與繪圖

選項與參數以關鍵字引數傳入；未知的名稱會引發 `ValueError`。

```python
line = chart.add_series("line", color="#ff9800", line_width=2)
candles.apply_options(down_color="#e91e63")
candles.create_price_line(101.5, title="target", line_style="dashed")
candles.set_markers([{"time": t, "position": "below_bar", "shape": "arrow_up", "text": "buy"}])
whale = chart.add_icon(avatar)       # a (height, width, 4) uint8 array; RGB works too
candles.set_markers([{"time": t, "icon": whale, "size": 24, "border": "#2fbf71"},
                     {"time": t, "label": "DB", "position": "below_bar"}])
candles.apply_options(marker_stack_limit=4)   # 3 markers per bar side, then "+k"
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

事件：`click`、`crosshair_move`、`visible_range_changed`、`need_more_history`、`drawing_created`、`drawing_modified`、`drawing_removed`、`drawing_selected`、`warning`、`marker_hover`（一個包含 `series`、`index`、`overflow`、`x`、`y`、`size` 的 dict，指標離開標記時為 `None`）。圖表顯示期間，回呼函式在主執行緒上執行。

## 中文、日文與韓文文字

內建字型涵蓋拉丁字母、數字與常用符號。若要顯示其他文字，請傳入字型檔案（路徑或位元組），依序查找；所有字型都沒有的字元會畫成 `□`：

```python
chart = fc.Chart(fonts=["/System/Library/Fonts/Hiragino Sans GB.ttc"])  # macOS
chart = fc.Chart(fonts=["NotoSansSC-Regular.otf"])                     # any system
```

這些字型由 `show()` 與 `to_png()` 使用。在 Jupyter 中，瀏覽器以其本身的字型繪製，不需要 `fonts`。參見[標記](markers.md)。

## 視窗

`chart.show()` 會阻塞直到視窗關閉，之後可再次呼叫。它必須在主執行緒上執行。在 Linux 與 Windows 上，`chart.show(block=False)` 會從背景執行緒開啟視窗並立即返回；此時事件回呼會在主執行緒上、於 Python 陳述式之間執行，或在你呼叫 `ferrochart.process_events()` 時執行。行程中第一次 `show` 會決定採用哪種模式。

在視窗中：拖曳以平移，滾輪或雙指縮放以縮放，拖曳座標軸以調整其刻度，雙擊座標軸以重設。啟動工具後，點擊以放置錨點；Esc 取消，Delete 移除選取的繪圖，Ctrl/⌘+Z 復原。

## Jupyter

作為儲存格最後一個運算式的圖表，會以互動式 widget 顯示（需安裝 `ferrochart[jupyter]`；否則顯示 PNG）。之後的變更（例如新的 K 棒）會反映到它的每一個檢視上。`chart.widget()` 會傳回該 widget，例如可放進 ipywidgets 版面中。

## Plots

`ferrochart.Plot` 可在數值、類別或日期座標軸上繪製折線、面積、散佈圖、長條圖、瀑布圖與熱力圖。它的顯示方式與圖表相同，有三種：`plot.show()`、作為 notebook 儲存格的最後一個運算式，或 `plot.to_png()`。

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

`set_data` 接受具有 `x`、`y` 欄以及選用的 `y0`、`value`（依標記的色階著色）、`size`、`color`、`step`（瀑布圖）與 `label` 欄的 DataFrame 或 dict；pandas Series（x 取自其索引）；y 值的串列或陣列；或關鍵字引數：`mark.set_data(x=xs, y=ys)`。`x` 中的日期時間需搭配 `plot.axis("x").apply_options(format="date")` 使用。事件：`hover`、`click`、`view_change`。
