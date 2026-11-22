"""Tests of drawings and saved state in Python."""

from __future__ import annotations

import datetime as dt
import json

import numpy as np
import pytest

import ferrochart as fc
from ferrochart import _jupyter

HOUR = 3_600_000


def chart() -> tuple[fc.Chart, fc.Series, dict]:
    c = fc.Chart(width=600, height=400)
    s = c.add_series("candlestick")
    d = fc.random_walk(1, 100, 0, HOUR)
    s.set_data(d)
    return c, s, d


def test_add_get_and_edit_drawings():
    c, _, d = chart()
    dr = c.drawings
    assert dr is c.drawings and len(dr) == 0
    assert "trend_line" in dr.kinds() and "fib_retracement" in dr.kinds()
    t0, t1 = int(d["time"][10]), int(d["time"][60])
    line = dr.add("trend_line", [(t0, 100.0), {"time": t1, "price": 120.0}],
                  style={"line": "#e91e63", "line_width": 2, "line_dash": "dashed"})
    fib = dr.add("fib_retracement", [(t0, 90), (t1, 130)], params={"levels": np.array([0, 0.5, 1])})
    assert dr.ids() == [line, fib] and list(dr) == [line, fib]
    got = dr.get(line)
    assert got["kind"] == "trend_line"
    assert got["anchors"] == [{"time": t0, "price": 100.0}, {"time": t1, "price": 120.0}]
    assert got["style"]["line"] == "#e91e63" and got["style"]["line_dash"] == "dashed"
    assert got["style"]["line_width"] == 2.0
    assert dr.get(fib)["params"] == {"levels": [0.0, 0.5, 1.0]}
    assert dr.get(12345) is None and dr.pane_of(line) == c.main_pane

    dr.set_style(line, line="#00ff00")
    assert dr.get(line)["style"]["line"] == "#00ff00" and dr.get(line)["style"]["line_width"] == 2.0
    dr.set_anchors(line, [(t0, 1.0), (t1, 2.0)])
    assert [a["price"] for a in dr.get(line)["anchors"]] == [1.0, 2.0]
    dr.set_param(fib, "levels", (0, 1))
    assert dr.get(fib)["params"]["levels"] == [0.0, 1.0]
    dr.set_visible(line, False)
    dr.set_locked(fib, True)
    assert dr.is_visible(line) is False and dr.is_locked(fib) is True and dr.is_visible(999) is None
    dr.send_to_back(fib)
    assert dr.ids() == [fib, line]
    dr.bring_to_front(fib)
    assert dr.ids() == [line, fib]
    dr.select(line)
    assert dr.selected == line
    dr.select(None)
    assert dr.selected is None

    # A saved form adds an equal drawing.
    copy = dr.add(dr.get(fib))
    assert dr.get(copy) == dr.get(fib)
    dr.remove(copy)
    assert copy not in dr.ids()


def test_datetimes_and_text_params():
    c, _, d = chart()
    when = dt.datetime(1970, 1, 2, 3, tzinfo=dt.timezone.utc)
    tid = c.drawings.add("text", [(when, 105.0)], params={"text": "breakout"})
    got = c.drawings.get(tid)
    assert got["anchors"][0]["time"] == 27 * HOUR and got["params"] == {"text": "breakout"}
    c.drawings.set_param(tid, "text", "later")
    assert c.drawings.get(tid)["params"]["text"] == "later"
    c.drawings.add("horizontal_line", [(np.datetime64("1970-01-01T05:00"), 100)])


def test_undo_redo_and_tools():
    c, _, d = chart()
    dr = c.drawings
    assert not dr.can_undo and dr.undo() is False
    a = dr.add("horizontal_line", [(0, 100.0)])
    dr.remove(a)
    assert dr.ids() == [] and dr.can_undo
    assert dr.undo() and dr.ids() == [a]  # the drawing keeps its id
    assert dr.undo() and dr.ids() == []
    assert dr.can_redo and dr.redo() and dr.ids() == [a]
    assert dr.active_tool is None
    dr.start_tool("rectangle")
    assert dr.active_tool == "rectangle"
    dr.cancel_tool()
    assert dr.active_tool is None


@pytest.mark.parametrize(
    ("call", "match"),
    [
        (lambda dr: dr.add("no_such_tool", [(0, 1)]), "no_such_tool"),
        (lambda dr: dr.add("trend_line", [(0, 1)]), "anchor"),
        (lambda dr: dr.add("trend_line", [(0, 1), (1, 2)], style={"colour": "red"}), "colour"),
        (lambda dr: dr.add("trend_line", [(0, 1), (1, 2)], style={"line": "nope"}), "CSS color"),
        (lambda dr: dr.add("trend_line", [(0, 1), (1, 2)], style={"line_width": -1}), "positive"),
        (lambda dr: dr.add("trend_line", [(0, 1), (1, 2)], style={"line_dash": "wavy"}), "line_dash"),
        (lambda dr: dr.add("trend_line", [{"time": 0}, (1, 2)]), "price"),
        (lambda dr: dr.add("trend_line", [(0, float("nan")), (1, 2)]), "finite"),
        (lambda dr: dr.add({"kind": "trend_line", "anchors": [], "extra": 1}), "extra"),
        (lambda dr: dr.remove(77), "drawing"),
        (lambda dr: dr.set_style(77, line="#fff"), "unknown drawing"),
        (lambda dr: dr.start_tool("nope"), "nope"),
    ],
)
def test_errors_are_value_errors(call, match):
    c, _, _ = chart()
    with pytest.raises(ValueError, match=match):
        call(c.drawings)


def test_state_round_trips_through_json():
    c, s, d = chart()
    pane = c.add_pane(0.5)
    c.bar_spacing = 9
    c.right_offset = 3
    c.crosshair_mode = "magnet"
    c.apply_price_scale_options(mode="indexed_to_100", margins=(0.1, 0.2))
    c.apply_price_scale_options(pane, inverted=True, range=(1, 2))
    a = c.drawings.add("trend_line", [(0, 100.0), (HOUR * 50, 110.0)], style={"line": "#ff000080"})
    c.drawings.add("text", [(HOUR, 95.0)], pane=pane, params={"text": "hi"})
    c.drawings.set_locked(a, True)
    state = c.save_state()
    assert state["bar_spacing"] == 9 and state["right_offset"] == 3 and state["crosshair_mode"] == "magnet"
    assert state["panes"][0]["price_scale"]["mode"] == "indexed_to_100"
    assert state["panes"][1]["price_scale"]["range"] == [1.0, 2.0]
    assert [x["pane"] for x in state["drawings"]] == [0, 1] and state["drawings"][0]["locked"]
    assert state["drawings"][0]["data"]["style"]["line"] == "rgba(255, 0, 0, 0.502)"
    text = json.dumps(state)  # JSON-compatible

    c2 = fc.Chart(width=600, height=400)
    c2.add_series("candlestick").set_data(d)
    c2.add_pane(1.0)
    c2.load_state(json.loads(text))
    assert c2.save_state() == state
    assert c2.bar_spacing == 9 and c2.crosshair_mode == "magnet"
    assert not c2.drawings.can_undo
    # Rendering works after loading.
    assert c2.to_png(backend="cpu")[:4] == b"\x89PNG"


def test_bad_states_leave_the_chart_unchanged():
    c, _, _ = chart()
    c.drawings.add("horizontal_line", [(0, 100.0)])
    before = c.save_state()
    two_panes = {**before, "panes": before["panes"] * 2}
    with pytest.raises(ValueError):
        c.load_state(two_panes)
    with pytest.raises(ValueError, match="state key"):
        c.load_state({**before, "zoom": 1})
    with pytest.raises(ValueError, match="crosshair"):
        c.load_state({**before, "crosshair_mode": "sideways"})
    with pytest.raises(ValueError, match="margins"):
        bad = json.loads(json.dumps(before))
        bad["panes"][0]["price_scale"]["margins"] = [0.1]
        c.load_state(bad)
    bad = json.loads(json.dumps(before))
    bad["drawings"].append({"pane": 0, "visible": True, "locked": False,
                            "data": {"kind": "nope", "anchors": [{"time": 0, "price": 1}]}})
    with pytest.raises(ValueError):
        c.load_state(bad)
    assert c.save_state() == before


def test_drawing_operations_are_recorded(monkeypatch):
    monkeypatch.setattr(_jupyter, "RECORD", True)
    c, _, d = chart()
    dr = c.drawings
    a = dr.add("trend_line", [(0, 1.0), (HOUR, 2.0)], style={"line_width": 3, "font_size": 14})
    b = dr.add("fib_retracement", [(0, 1), (HOUR, 2)], params={"levels": np.array([0.0, 1.0])})
    dr.set_style(a, line_dash="dotted")
    dr.set_param(b, "levels", np.array([0.5]))
    dr.set_anchors(a, [(dt.datetime(1970, 1, 1, 2), 3.0), (HOUR * 3, 4.0)])
    dr.set_visible(a, False)
    dr.select(a)
    dr.undo()
    dr.remove(b)
    state = c.save_state()
    c.load_state(state)
    ops = [op for _, op, _ in c._rec.ops if op["op"] not in ("addSeries", "setData")]
    assert [o["op"] for o in ops] == [
        "addDrawing", "addDrawing", "drawingStyle", "drawingParam", "drawingAnchors",
        "drawingVisible", "selectDrawing", "undoDrawing", "removeDrawing", "loadState",
    ]
    assert ops[0] == {"op": "addDrawing", "id": a, "pane": None, "data": {
        "kind": "trend_line", "anchors": [{"time": 0, "price": 1.0}, {"time": HOUR, "price": 2.0}],
        "style": {"lineWidth": 3, "fontSize": 14}}}
    assert ops[1]["data"]["params"] == {"levels": [0.0, 1.0]}
    assert ops[2]["style"] == {"lineDash": "dotted"} and ops[3]["value"] == [0.5]
    assert ops[4]["anchors"][0] == {"time": 2 * HOUR, "price": 3.0}
    js = ops[-1]["state"]
    assert set(js) == {"version", "barSpacing", "rightOffset", "crosshairMode", "panes", "drawings"}
    assert js["panes"][0]["priceScale"]["margins"].keys() == {"top", "bottom"}
    assert ops[-1]["drawingIds"] == dr.ids() and len(js["drawings"]) == 1
    json.dumps(ops)  # all plain values
