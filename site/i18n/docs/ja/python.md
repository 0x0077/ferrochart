<!-- source: acdcedb855 -->
# Python

`ferrochart`パッケージは、Pythonからネイティブウィンドウ、Jupyter、またはPNGにチャートを描画します。描画にはGPU（Metal、Vulkan、DirectX 12）を使い、CPUへのフォールバックもあります。pandasやpolarsのDataFrame、またはNumPy配列のdictを受け取ります。

```bash
pip install ferrochart              # windows and PNG
pip install "ferrochart[jupyter]"   # also the Jupyter widget
```

プラットフォームごとに1つのwheelでPython 3.10以降に対応します。

## クイックスタート


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

`time`は`datetime64`のカラム（タイムゾーンなしの時刻はUTCとみなします）または整数のミリ秒です。`time`カラムがない場合はpandasの`DatetimeIndex`が使われます。ライン、エリア、ベースライン、ヒストグラム、ポイントのシリーズは`time`と`value`、および省略可能なポイントごとの`color`を受け取ります。

## リアルタイムデータ

```python
candles.update_last(t, open=o, high=h, low=l, close=c, volume=v)  # same time: update, later: new bar
candles.append(new_bars)
```

インジケーターはデータに追従します。チャートの表示中に行った変更は、他のスレッドからの変更も含め、すぐに描画されます。

## オプション、インジケーター、描画

オプションとパラメーターはキーワード引数です。不明な名前は`ValueError`を送出します。

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

## イベント

```python
@chart.on("click")
def clicked(e):
    print(e["time"], e["price"], e["target"])

chart.on("crosshair_move", lambda e: print(e["point"]))
```

イベント：`click`、`crosshair_move`、`visible_range_changed`、`need_more_history`、`drawing_created`、`drawing_modified`、`drawing_removed`、`drawing_selected`、`warning`。コールバックはチャートの表示中にメインスレッドで実行されます。

## ウィンドウ

`chart.show()`はウィンドウが閉じられるまでブロックし、何度でも呼べます。メインスレッドで実行する必要があります。LinuxとWindowsでは、`chart.show(block=False)`がバックグラウンドスレッドからウィンドウを開いてすぐに戻ります。その場合、イベントのコールバックはPythonの文と文の間か、`ferrochart.process_events()`を呼んだときにメインスレッドで実行されます。プロセスで最初の`show`がモードを決めます。

ウィンドウ内では、ドラッグでパン、ホイールまたはピンチでズーム、軸のドラッグでスケール変更、軸のダブルクリックでリセットします。ツールを開始した状態では、クリックでアンカーを配置します。Escで取り消し、Deleteで選択中の描画を削除、Ctrl/⌘+Zで元に戻します。

## Jupyter

セルの最後の式になったチャートは、インタラクティブなウィジェットとして表示されます（`ferrochart[jupyter]`がある場合。ない場合はPNG）。新しい足など後からの変更は、そのチャートのすべての表示に反映されます。`chart.widget()`はウィジェットを返すので、たとえばipywidgetsのレイアウトに配置できます。

## プロット

`ferrochart.Plot`は、数値、カテゴリー、日付の軸上に、ライン、エリア、散布図、棒グラフ、ウォーターフォール、ヒートマップを描画します。表示方法はチャートと同じ3通りです：`plot.show()`、ノートブックのセルの最後の式、`plot.to_png()`。

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

`set_data`が受け取るのは、カラム`x`、`y`と省略可能な`y0`、`value`（マークのカラースケールで色付け）、`size`、`color`、`step`（ウォーターフォール）、`label`を持つDataFrameまたはdict、pandasのSeries（xはインデックスから）、y値のリストまたは配列、あるいはキーワード引数（`mark.set_data(x=xs, y=ys)`）です。`x`の日時は`plot.axis("x").apply_options(format="date")`で扱えます。イベント：`hover`、`click`、`view_change`。
