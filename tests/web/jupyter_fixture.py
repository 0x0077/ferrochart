"""Fixture of the Jupyter front-end browser test (tests/web/jupyter.mjs):
builds a chart and a plot with the Python package (operations recorded as
in a kernel), captures the widgets' messages to a view and renders the same
chart and plot with the CPU renderer as reference images (the plot's in
OUT_DIR/plot).

    python tests/web/jupyter_fixture.py OUT_DIR
"""

import json
import sys
from pathlib import Path

import ferrochart as fc
from ferrochart import _jupyter

HOUR = 3_600_000

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
_jupyter.RECORD = True

chart = fc.Chart(width=500, height=300)
d = fc.random_walk(5, 160, 1_700_000_000_000, HOUR)
s = chart.add_series("candlestick", down_color="#e91e63")
s.set_data({k: v[:150] for k, v in d.items()})
chart.add_volume_series(s)
chart.add_indicator(s, "ema", n=12)
chart.add_indicator(s, "rsi", placement="new_pane")
line = chart.add_series("line", color="#ff9800", line_width=2)
line.set_data({"time": d["time"][:150], "value": d["close"][:150] + 3,
               "color": ["#2196f3" if i % 20 < 10 else None for i in range(150)]})
s.set_markers([{"time": int(d["time"][120]), "position": "below_bar", "shape": "arrow_up", "text": "buy"}])
s.create_price_line(float(d["close"][100]), color="#4caf50", title="level")
chart.apply_price_scale_options(margins=(0.15, 0.15))
chart.set_visible_logical_range(40, 160)
t = lambda i: int(d["time"][i])  # noqa: E731
dr = chart.drawings
trend = dr.add("trend_line", [(t(60), float(d["low"][60])), (t(130), float(d["high"][130]))],
               style={"line": "#9c27b0", "line_width": 2})
fib = dr.add("fib_retracement", [(t(80), float(d["low"][80])), (t(110), float(d["high"][110]))],
             params={"levels": [0, 0.382, 0.618, 1]})
note = dr.add("text", [(t(70), float(d["high"][70]) + 1)], params={"text": "note"})
dr.set_style(fib, line_dash="dashed")

widget = chart.widget()
sent = []
widget.send = lambda content, buffers=None: sent.append((content, list(buffers or [])))
widget._on_msg(widget, {"type": "init", "view": "VIEW", "wasm": True}, [])
chart.to_png(out / "before.png", backend="cpu")
range_before = chart.visible_logical_range()

# Live changes after the view is up: an update of the last bar, new bars.
last = 149
s.update_last(int(d["time"][last]), open=float(d["open"][last]), high=float(d["high"][last]) + 1,
              low=float(d["low"][last]), close=float(d["close"][last]) + 0.8, volume=float(d["volume"][last]))
s.append({k: v[150:] for k, v in d.items()})
# Drawing changes, an undo, and a state reload (ids map again afterwards).
dr.set_anchors(trend, [(t(50), float(d["low"][50])), (t(140), float(d["high"][140]))])
dr.remove(note)
box = dr.add("rectangle", [(t(120), float(d["low"][125])), (t(135), float(d["high"][130]))],
             style={"fill": "rgba(255, 152, 0, 0.25)", "line": "#ff9800"})
dr.undo()  # the rectangle goes again
state = chart.save_state()
dr.add("horizontal_line", [(t(100), float(d["close"][100]))])  # dropped by the reload
chart.load_state(state)
dr.set_param(dr.ids()[-1], "levels", [0, 0.5, 1])  # the reloaded Fibonacci retracement
dr.set_visible(dr.ids()[0], True)
chart.to_png(out / "after.png", backend="cpu")
range_after = chart.visible_logical_range()

messages = []
for i, (content, buffers) in enumerate(sent):
    names = []
    for j, b in enumerate(buffers):
        name = f"m{i}b{j}.bin"
        (out / name).write_bytes(b)
        names.append(name)
    messages.append({"msg": content, "buffers": names})
(out / "messages.json").write_text(json.dumps({"messages": messages, "before": range_before, "after": range_after}))
(out / "widget.mjs").write_text(widget._esm)
print(f"{len(messages)} messages")


def save(sent, out, extra):
    out.mkdir(parents=True, exist_ok=True)
    messages = []
    for i, (content, buffers) in enumerate(sent):
        names = []
        for j, b in enumerate(buffers):
            name = f"m{i}b{j}.bin"
            (out / name).write_bytes(b)
            names.append(name)
        messages.append({"msg": content, "buffers": names})
    (out / "messages.json").write_text(json.dumps({"messages": messages, **extra}))
    return messages


# A plot: bars on a category axis, a line, a scatter with per-point
# colors and labels, a rule and a label; then live changes.
plot = fc.Plot(width=500, height=300)
bars = plot.add_mark("bars", [4.0, 6.0, 5.0], color="#2196f3")
plot.axis("x").apply_options(type="category")
plot.axis("x").set_categories(["north", "south", "east"])
plot.axis("right").apply_options(include_zero=True, format={"type": "fixed", "decimals": 1})
line = plot.add_mark("line", {"x": [0, 1, 2], "y": [3.0, 5.5, 4.5]}, color="#ff9800", line_width=2)
dots = plot.add_mark("scatter", {"x": [0, 1, 2], "y": [1.0, 2.0, 1.5],
                                 "color": ["#e91e63", None, "#4caf50"], "label": ["a", None, "c"]},
                     point_size=10)
rule = plot.add_rule(5.0, label="target", line_style="dashed")
note = plot.add_label(2, 5.0, "note", icon="arrow_down", color="#9c27b0")
pwidget = plot.widget()
psent = []
pwidget.send = lambda content, buffers=None: psent.append((content, list(buffers or [])))
pwidget._on_msg(pwidget, {"type": "init", "view": "VIEW", "wasm": True}, [])
pout = out / "plot"
pout.mkdir(parents=True, exist_ok=True)
plot.to_png(pout / "before.png", backend="cpu")
# Bar 1 is under the plot's center: its position for the hover check.
mid = {"mark": bars.id, "index": 1}

line.append(x=[3], y=[6.5])
line.update_last(3, 7.0)
bars.append(x=[3], y=[3.0])
plot.axis("x").set_categories(["north", "south", "east", "west"])
dots.apply_options(color="#3f51b5", point_size=6)
rule.apply_options(value=6.0, to=6.5, label=None)
note.remove()
plot.add_label(3, 7.0, "new", icon="circle")
plot.axis("right").apply_options(range=(0, 8))
plot.to_png(pout / "after.png", backend="cpu")
plot.fit()  # a viewChange in the browser
pm = save(psent, pout, {"hit": mid})
print(f"{len(pm)} plot messages")
