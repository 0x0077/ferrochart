<!-- source: acdcedb855 -->
# Python

`ferrochart` 패키지는 Python에서 네이티브 창, Jupyter, PNG로 차트를 그립니다.
GPU(Metal, Vulkan, DirectX 12)를 사용하며 CPU 대체 경로가 있습니다. pandas나
polars DataFrame, 또는 NumPy 배열의 dict를 받습니다.

```bash
pip install ferrochart              # windows and PNG
pip install "ferrochart[jupyter]"   # also the Jupyter widget
```

플랫폼마다 휠 하나가 Python 3.10 이상을 지원합니다.

## 빠른 시작


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

`time`은 `datetime64` 열(타임존 없는 시간은 UTC) 또는 정수 밀리초입니다. `time`
열이 없으면 pandas `DatetimeIndex`를 사용합니다. 라인, 영역, 베이스라인,
히스토그램, 포인트 시리즈는 `time`과 `value`, 그리고 선택적으로 점별 `color`를
받습니다.

## 실시간 데이터

```python
candles.update_last(t, open=o, high=h, low=l, close=c, volume=v)  # same time: update, later: new bar
candles.append(new_bars)
```

지표는 데이터를 따라갑니다. 차트가 표시되는 동안의 변경은 다른 스레드에서 온
변경을 포함해 즉시 그려집니다.

## 옵션, 지표, 드로잉

옵션과 매개변수는 키워드 인수이며, 알 수 없는 이름은 `ValueError`를
발생시킵니다.

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

## 이벤트

```python
@chart.on("click")
def clicked(e):
    print(e["time"], e["price"], e["target"])

chart.on("crosshair_move", lambda e: print(e["point"]))
```

이벤트: `click`, `crosshair_move`, `visible_range_changed`,
`need_more_history`, `drawing_created`, `drawing_modified`, `drawing_removed`,
`drawing_selected`, `warning`. 콜백은 차트가 표시되는 동안 메인 스레드에서
실행됩니다.

## 창

`chart.show()`는 창이 닫힐 때까지 블록되며, 다시 호출할 수 있습니다. 메인
스레드에서 실행해야 합니다. Linux와 Windows에서는 `chart.show(block=False)`가
백그라운드 스레드에서 창을 열고 즉시 반환합니다. 이때 이벤트 콜백은 Python 문장
사이에, 또는 `ferrochart.process_events()`를 호출할 때 메인 스레드에서
실행됩니다. 프로세스에서 처음 호출한 `show`가 모드를 결정합니다.

창에서는 드래그로 이동하고, 휠이나 핀치로 확대·축소하고, 축을 드래그해 스케일을
조정하고, 축을 더블클릭해 초기화합니다. 도구를 시작한 상태에서는 클릭으로 앵커를
배치합니다. Esc는 취소, Delete는 선택한 드로잉 제거, Ctrl/⌘+Z는 실행 취소입니다.

## Jupyter

셀의 마지막 식인 차트는 인터랙티브 위젯으로 표시됩니다(`ferrochart[jupyter]`가
있을 때, 없으면 PNG). 이후의 변경(예: 새 봉)은 그 차트의 모든 뷰에 반영됩니다.
`chart.widget()`은 위젯을 반환하므로, 예를 들어 ipywidgets 레이아웃에 넣을 수
있습니다.

## 플롯

`ferrochart.Plot`은 숫자, 카테고리, 날짜 축 위에 선, 영역, 산점도, 막대, 폭포,
히트맵을 그립니다. 차트와 같은 세 가지 방법으로 표시합니다. `plot.show()`, 노트북
셀의 마지막 식, `plot.to_png()`입니다.

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

`set_data`는 `x`, `y` 열과 선택 사항인 `y0`, `value`(마크의 색상 스케일로
색칠), `size`, `color`, `step`(폭포), `label` 열을 가진 DataFrame이나 dict,
pandas Series(x는 인덱스), y 값의 리스트나 배열, 또는 키워드
(`mark.set_data(x=xs, y=ys)`)를 받습니다. `x`의 datetime은
`plot.axis("x").apply_options(format="date")`와 함께 동작합니다. 이벤트:
`hover`, `click`, `view_change`.
