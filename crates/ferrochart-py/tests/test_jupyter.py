"""Tests of the Jupyter display: the column buffer format, the
recorded operations, and the widget's messages (front end: the browser test
tests/web/jupyter.mjs)."""

from __future__ import annotations

import numpy as np
import pytest

import ferrochart as fc
from ferrochart import _jupyter

HOUR = 3_600_000


@pytest.fixture
def recording(monkeypatch):
    monkeypatch.setattr(_jupyter, "RECORD", True)


def test_column_buffers_round_trip_and_stay_aligned():
    cols = {
        "time": np.arange(5, dtype=np.int64) * HOUR,
        "value": np.linspace(0, 1, 5),
        "color": np.array([0, 0xFF0000FF, 0, 1, 2], dtype=np.uint32),  # 20 bytes: padded
        "close": np.array([1.5, 2, 3, 4, 5], dtype=">f8"),  # big-endian input
    }
    buf = _jupyter.encode_columns(cols)
    assert buf[:4] == b"FCCB" and len(buf) % 8 == 0
    back = _jupyter.decode_columns(buf)
    assert list(back) == list(cols)
    for k, v in cols.items():
        np.testing.assert_array_equal(back[k], v)
    # Every column starts 8-byte aligned.
    pos = 16 + 16 * len(cols)
    for v in back.values():
        assert pos % 8 == 0
        pos += len(v) * v.itemsize + (-len(v) * v.itemsize % 8)
    with pytest.raises(ValueError, match="rows"):
        _jupyter.encode_columns({"a": np.zeros(2), "b": np.zeros(3)})
    with pytest.raises(TypeError, match="dtype"):
        _jupyter.encode_columns({"a": np.zeros(2, dtype=np.float32)})


def test_css_colors_become_rgba():
    assert list(_jupyter.css_to_u32(["#ff0000", None, "#0f08", "rgba(0, 0, 255, 0.5)", "transparent"])) == [
        0xFF0000FF, 0, 0x00FF0088, 0x0000FF80, 0,
    ]


def test_charts_outside_a_kernel_record_nothing():
    c = fc.Chart()
    c.add_series("line")
    assert c._rec is None
    with pytest.raises(RuntimeError, match="outside a Jupyter kernel"):
        c.widget()
    bundle = c._repr_mimebundle_()
    assert bundle["image/png"][:4] == b"\x89PNG"


def test_operations_are_recorded_in_js_terms(recording):
    c = fc.Chart(theme="dark", width=500, height=300)
    s = c.add_series("heikin_ashi", down_color="#ff0000", line_width=2)
    d = fc.random_walk(1, 50, 0, HOUR)
    s.set_data(d)
    v = c.add_volume_series(s)
    ind = c.add_indicator(s, "stoch_rsi", rsi_n=10, placement="new_pane")
    c.apply_price_scale_options(mode="indexed_to_100", margins=(0.1, 0.2))
    s.set_markers([{"time": int(d["time"][3]), "position": "above_bar", "shape": "arrow_down"}])
    line = s.create_price_line(100.0, axis_label_visible=False)
    line.apply_options(title="x")
    c.set_visible_logical_range(10, 40)
    ops = [op for _, op, _ in c._rec.ops]
    names = [op["op"] for op in ops]
    assert names == [
        "addSeries", "setData", "addVolume", "addIndicator", "priceScale",
        "setMarkers", "createPriceLine", "applyPriceLine", "setVisibleRange",
    ]
    assert ops[0] == {"op": "addSeries", "id": s.id, "kind": "heikinAshi",
                      "options": {"downColor": "#ff0000", "lineWidth": 2}, "pane": None}
    assert ops[2]["source"] == s.id and ops[2]["id"] == v.id
    assert ops[3]["name"] == "stochRsi" and ops[3]["params"] == {"rsiN": 10}
    assert ops[3]["placement"] == "newPane" and ops[3]["outputs"] == [o.id for o in ind.outputs]
    assert ops[4]["options"] == {"mode": "indexedTo100", "margins": {"top": 0.1, "bottom": 0.2}}
    assert ops[5]["markers"][0]["position"] == "aboveBar" and ops[5]["markers"][0]["shape"] == "arrowDown"
    assert ops[6]["options"] == {"price": 100.0, "axisLabelVisible": False}
    data = _jupyter.decode_columns(c._rec.ops[1][2])
    np.testing.assert_array_equal(data["close"], d["close"])
    assert data["time"].dtype == np.int64


def test_recorded_data_is_compacted(recording):
    c = fc.Chart()
    s = c.add_series("candlestick")
    d = fc.random_walk(1, 20, 0, HOUR)
    s.set_data({k: x[:10] for k, x in d.items()})
    s.append({k: x[10:15] for k, x in d.items()})
    t = int(d["time"][14])
    for close in (1.0, 2.0, 3.0):
        s.update_last(t, open=1, high=4, low=0.5, close=close, volume=1)
    names = [op["op"] for _, op, _ in c._rec.ops]
    assert names == ["addSeries", "setData", "append", "update"]
    assert c._rec.ops[-1][1]["bar"]["close"] == 3.0
    s.set_data(d)  # new data replaces the older data operations
    assert [op["op"] for _, op, _ in c._rec.ops] == ["addSeries", "setData"]
    seq, ops, buffers = c._rec.snapshot()
    assert seq == 7 and ops[1]["buffer"] == 0 and len(buffers) == 1
    line = c.add_series("line")
    line.set_data({"time": d["time"], "value": d["close"], "color": ["#ff0000"] + [None] * 19})
    colors = _jupyter.decode_columns(c._rec.ops[-1][2])["color"]
    assert colors[0] == 0xFF0000FF and colors[1] == 0


def test_live_updates_fold_into_one_set_data(recording):
    c = fc.Chart()
    s = c.add_series("candlestick")
    line = c.add_series("line")
    d = fc.random_walk(2, 300, 0, HOUR)
    s.set_data({k: x[:10] for k, x in d.items()})
    colors = ["#ff0000"] + [None] * 9
    line.set_data({"time": d["time"][:10], "value": d["close"][:10], "color": colors})
    for i in range(10, 300):
        t = int(d["time"][i])
        for close in (d["open"][i], d["close"][i]):
            s.update_last(t, open=d["open"][i], high=d["high"][i], low=d["low"][i],
                          close=close, volume=d["volume"][i])
        line.update_last(t, d["open"][i])
        line.update_last(t, d["close"][i])
        if i % 50 == 0:
            s.append({k: x[i + 1 : i + 2] for k, x in d.items()})
            line.append({"time": d["time"][i + 1 : i + 2], "value": d["close"][i + 1 : i + 2]})
            s.update_last(int(d["time"][i + 1]), open=d["open"][i + 1], high=d["high"][i + 1],
                          low=d["low"][i + 1], close=d["close"][i + 1], volume=d["volume"][i + 1])
            line.update_last(int(d["time"][i + 1]), d["close"][i + 1])
    # Index 51, 101, … came twice (append, then the loop): the same bar.
    ops = c._rec.ops
    assert len(ops) < 2 + 2 * (_jupyter.FOLD_AFTER + 2)
    names = [op["op"] for _, op, _ in ops]
    assert names[:4] == ["addSeries", "addSeries", "setData", "setData"]
    replay: dict[int, dict[str, np.ndarray]] = {}
    for _, op, buf in ops:
        if op["op"] == "setData":
            replay[op["series"]] = {k: v.copy() for k, v in _jupyter.decode_columns(buf).items()}
        elif op["op"] == "append":
            more = _jupyter.decode_columns(buf)
            cur = replay[op["series"]]
            replay[op["series"]] = {k: np.concatenate([v, more[k]]) for k, v in cur.items()}
        elif op["op"] == "update":
            replay[op["series"]] = _jupyter._apply_update(replay[op["series"]], op)
    candles = replay[s.id]
    for k in ("time", "open", "high", "low", "close", "volume"):
        np.testing.assert_array_equal(candles[k], d[k], err_msg=k)
    values = replay[line.id]
    np.testing.assert_array_equal(values["time"], d["time"])
    np.testing.assert_array_equal(values["value"], d["close"])
    assert values["color"][0] == 0xFF0000FF and not values["color"][1:].any()


def test_live_plot_updates_fold(recording):
    p = fc.Plot()
    m = p.add_mark("line", [0.0])
    for i in range(1, 200):
        m.append(x=[i], y=[0.0])
        m.update_last(i, i * 2.0)
    ops = p._rec.ops
    assert len(ops) <= _jupyter.FOLD_AFTER + 2
    first = next(op for _, op, _ in ops if op["op"] == "setData")
    assert first["mark"] == m.id


@pytest.fixture
def widget(recording):
    pytest.importorskip("anywidget")
    if not _jupyter.has_frontend():
        pytest.skip("no Jupyter front end (run scripts/build-py-static.sh)")
    c = fc.Chart(width=500, height=300)
    s = c.add_series("candlestick")
    s.set_data(fc.random_walk(1, 50, 0, HOUR))
    w = c.widget()
    sent: list[tuple[dict, list]] = []
    w.send = lambda content, buffers=None: sent.append((content, list(buffers or [])))
    return c, s, w, sent


def test_widget_answers_views_and_broadcasts_changes(widget):
    c, s, w, sent = widget
    assert c.widget() is w and (w.width, w.height, w.theme) == (500, 300, "light")
    assert w.options == {}
    assert "__wbg_init" in w._esm and "export default" in w._esm
    w._on_msg(w, {"type": "init", "view": "v1", "wasm": True}, [])
    reply, buffers = sent.pop()
    assert reply["type"] == "snapshot" and reply["to"] == "v1" and reply["seq"] == 2
    assert [op["op"] for op in reply["ops"]] == ["addSeries", "setData"]
    assert reply["wasm"] == 1 and buffers[1][:4] == b"\0asm"
    w._on_msg(w, {"type": "init", "view": "v2", "wasm": False}, [])
    reply, buffers = sent.pop()
    assert "wasm" not in reply and len(buffers) == 1
    s.update_last(49 * HOUR, open=1, high=2, low=0.5, close=1.5, volume=3)
    live, bufs = sent.pop()
    assert live == {"type": "ops", "seq": 3, "ops": [{"op": "update", "series": s.id,
                    "bar": {"time": 49 * HOUR, "open": 1.0, "high": 2.0, "low": 0.5, "close": 1.5, "volume": 3.0}}]}
    s.append({k: v[-1:] + 0 for k, v in fc.random_walk(1, 51, 0, HOUR).items()})
    live, bufs = sent.pop()
    assert live["ops"][0] == {"op": "append", "series": s.id, "buffer": 0} and len(bufs) == 1


def test_widget_forwards_events_with_python_names(widget):
    c, s, w, _ = widget
    got: list[dict] = []
    assert w.events == []
    c.on("click", got.append)
    assert w.events == ["click"]
    w._on_msg(w, {"type": "event", "name": "click", "payload": {
        "x": 10, "y": 20, "pane": 0, "logicalIndex": 3.2, "time": 3 * HOUR, "price": 1.5,
        "target": {"type": "series", "series": s.id}}}, [])
    assert got and got[0]["logical_index"] == 3.2 and got[0]["target"]["series"] == s
    c.off("click")
    assert w.events == []
    bundle = c._repr_mimebundle_()
    data = bundle[0] if isinstance(bundle, tuple) else bundle
    assert "application/vnd.jupyter.widget-view+json" in data


def test_plot_operations_are_recorded_in_js_terms(recording):
    p = fc.Plot(theme="dark", width=400, height=250, crosshair=True)
    m = p.add_mark("scatter", {"x": [0, 1], "y": [1, 2], "color": ["#ff0000", None], "label": ["a", None]},
                   icon="arrow_up", y_axis="left", color_scale={"stops": [(0, "#000"), (1, "#fff")]},
                   value_format={"type": "date", "utc_offset_minutes": 60})
    p.axis("x").apply_options(type="category", include_zero=True, range=(0, 5))
    p.axis("x").set_categories(["a", "b"])
    r = p.add_rule(1.5, label_background=False)
    lab = p.add_label(0, 1, "hi", y_axis="left", offset=(2, 3))
    m.apply_options(point_size=8)
    p.axis("right").set_view(0, 3)
    p.fit()
    r.remove()
    lab.remove()
    ops = [op for _, op, _ in p._rec.ops]
    assert [op["op"] for op in ops] == [
        "addMark", "setData", "axis", "categories", "addRule", "addLabel",
        "markOptions", "setView", "fit", "removeRule", "removeLabel",
    ]
    assert ops[0] == {"op": "addMark", "id": m.id, "kind": "scatter", "options": {
        "icon": "arrowUp", "yAxis": "left", "colorScale": {"stops": [[0, "#000"], [1, "#fff"]]},
        "valueFormat": {"type": "date", "utcOffsetMinutes": 60}}}
    assert ops[1]["mark"] == m.id and ops[1]["label"] == ["a", None] and "color" not in ops[1]
    cols = _jupyter.decode_columns(p._rec.ops[1][2])
    assert list(cols) == ["x", "y", "color"] and cols["color"][0] == 0xFF0000FF
    assert ops[2]["options"] == {"type": "category", "includeZero": True, "range": [0, 5]}
    assert ops[4]["options"] == {"axis": "right", "value": 1.5, "labelBackground": False}
    assert ops[5]["options"] == {"x": 0, "y": 1, "text": "hi", "yAxis": "left", "offset": [2, 3]}
    assert ops[7] == {"op": "setView", "axis": "right", "lo": 0.0, "hi": 3.0}


def test_plot_data_is_compacted(recording):
    p = fc.Plot()
    m = p.add_mark("line", [1, 2, 3])
    m.append(x=[3], y=[4])
    m.update_last(3, 5)
    m.update_last(3, 6, color="#00ff00")
    assert [op["op"] for _, op, _ in p._rec.ops] == ["addMark", "setData", "append", "update"]
    assert p._rec.ops[-1][1] == {"op": "update", "mark": m.id, "point": {"x": 3.0, "y": 6.0, "color": "#00ff00"}}
    m.set_data([7, 8])
    assert [op["op"] for _, op, _ in p._rec.ops] == ["addMark", "setData"]


def test_plot_widget_and_events(recording):
    pytest.importorskip("anywidget")
    if not _jupyter.has_frontend():
        pytest.skip("no Jupyter front end (run scripts/build-py-static.sh)")
    p = fc.Plot(width=400, height=250, hover_highlight=False)
    m = p.add_mark("bars", [1, 2])
    w = p.widget()
    assert (w.kind, w.width, w.height) == ("plot", 400, 250)
    assert w.options == {"crosshair": False, "hoverHighlight": False, "attributionLogo": True}
    assert fc.Plot(attribution_logo=False)._options["attributionLogo"] is False
    sent: list[tuple[dict, list]] = []
    w.send = lambda content, buffers=None: sent.append((content, list(buffers or [])))
    w._on_msg(w, {"type": "init", "view": "v", "wasm": False}, [])
    reply, _ = sent.pop()
    assert [op["op"] for op in reply["ops"]] == ["addMark", "setData"]
    got: list[dict] = []
    p.on("view_change", got.append)
    p.on("click", got.append)
    assert w.events == ["click", "view_change"]
    w._on_msg(w, {"type": "event", "name": "click", "payload": {
        "hit": {"mark": m.id, "index": 1, "x": 1, "y": 2, "position": {"x": 5, "y": 6}}, "x": 1, "y": 2}}, [])
    assert got[0]["hit"]["mark"] == m and got[0]["hit"]["position"] == {"x": 5, "y": 6}


def test_icons_are_recorded_by_name(recording):
    c = fc.Chart()
    s = c.add_series("candlestick")
    s.set_data(fc.random_walk(1, 20, 0, HOUR))
    img = np.zeros((8, 4, 3), np.uint8)
    icon = c.add_icon(img)
    s.set_markers([{"time": 0, "icon": icon, "border_width": 1}])
    c.remove_icon(icon)
    _, ops, bufs = c._rec.snapshot()
    reg = next(o for o in ops if o["op"] == "registerIcon")
    assert reg["name"] == str(icon) and (reg["width"], reg["height"]) == (4, 8)
    # RGB becomes RGBA for ImageData.
    assert len(bufs[reg["buffer"]]) == 4 * 8 * 4 and bufs[reg["buffer"]][3] == 255
    marker = next(o for o in ops if o["op"] == "setMarkers")["markers"][0]
    assert marker["icon"] == str(icon) and marker["borderWidth"] == 1
    assert ops[-1] == {"op": "unregisterIcon", "name": str(icon)}


def test_look_options_are_recorded_camel_cased(recording):
    c = fc.Chart(theme={"base": "dark", "up_wick": "#00ff00"}, preset="sparkline", width=200, height=60)
    c.apply_options(crosshair_style={"dot": {"radius": 3}}, price_axis=True)
    ops = [op for _, op, _ in c._rec.ops if op["op"] == "chartOptions"]
    assert ops[0]["options"] == {"preset": "sparkline", "theme": {"base": "dark", "upWick": "#00ff00"}}
    assert ops[1]["options"] == {"crosshairStyle": {"dot": {"radius": 3}}, "priceAxis": True}
    assert c._theme == "dark"
    assert fc.Chart(attribution_logo=False)._options == {"attributionLogo": False}
