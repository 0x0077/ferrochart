"""Tests of `ferrochart.Plot`: marks, data forms, options, axes,
rules, labels, events and output. Windows are in test_window.py."""

from __future__ import annotations

import datetime as dt
import io
import struct
import zlib

import numpy as np
import pytest

import ferrochart as fc


def png_size(png: bytes) -> tuple[int, int]:
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    return struct.unpack(">II", png[16:24])


def pixels(png: bytes) -> np.ndarray:
    """RGBA pixels of a PNG written by the engine (8-bit RGBA, no
    interlace)."""
    w, h = png_size(png)
    pos, data = 8, b""
    while pos < len(png):
        n, kind = struct.unpack(">I4s", png[pos:pos + 8])
        if kind == b"IDAT":
            data += png[pos + 8:pos + 8 + n]
        pos += 12 + n
    raw = zlib.decompress(data)
    color_type = png[25]
    bpp = 4 if color_type == 6 else 3
    stride = w * bpp
    out = np.zeros((h, stride), dtype=np.uint8)
    prev = np.zeros(stride, dtype=np.int32)
    for y in range(h):
        f = raw[y * (stride + 1)]
        line = np.frombuffer(raw, np.uint8, stride, y * (stride + 1) + 1).astype(np.int32)
        cur = np.zeros(stride, dtype=np.int32)
        for x in range(stride):
            a = cur[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 0:
                p = 0
            elif f == 1:
                p = a
            elif f == 2:
                p = b
            elif f == 3:
                p = (a + b) // 2
            else:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                p = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            cur[x] = (line[x] + p) & 0xFF
        out[y] = cur
        prev = cur
    return out.reshape(h, w, bpp)


def test_marks_take_every_data_form():
    p = fc.Plot(width=300, height=200)
    a = p.add_mark("line", [1, 3, 2])
    assert a.kind == "line" and p.marks == [a]
    b = p.add_mark("scatter", np.array([[0, 1], [1, 2], [2, 0.5]]))
    c = p.add_mark("bars")
    c.set_data(x=[0, 1], y=[1, 2], color=["#ff0000", None])
    d = p.add_mark("area", {"x": [0, 1, 2], "y": [1, 2, 1], "y0": [0, 0.5, 0], "other": [9, 9, 9]})
    with pytest.raises(TypeError, match="unknown column"):
        d.set_data(x=[0], y=[1], z=[2])
    with pytest.raises(ValueError, match="missing column 'y'"):
        d.set_data(x=[0, 1])
    with pytest.raises(ValueError):
        d.set_data(x=[0, 1], y=[1])  # different lengths
    with pytest.raises(TypeError, match="set_categories"):
        d.set_data(x=["a", "b"], y=[1, 2])
    assert {m.kind for m in p.marks} == {"line", "scatter", "bars", "area"}
    p.fit()
    p.to_png(backend="cpu")
    lo, hi = p.axis("right").view
    assert lo <= 0 and hi >= 3
    lo, hi = p.axis("x").view
    assert lo <= 0 and hi >= 2
    assert b != a and hash(b) == hash(p.marks[1])


def test_pandas_series_and_frames():
    pd = pytest.importorskip("pandas")
    p = fc.Plot()
    idx = pd.date_range("2024-01-01", periods=4, freq="D", tz="Europe/Berlin")
    s = pd.Series([1.0, None, 3.0, 2.0], index=idx)
    m = p.add_mark("line", s)
    p.axis("x").apply_options(format="date")
    p.to_png(backend="cpu")
    lo, hi = p.axis("x").view
    first = int(pd.Timestamp("2023-12-31T23:00:00Z").value // 1_000_000)
    assert lo <= first and hi >= first + 3 * 86_400_000
    df = pd.DataFrame({"y": [1, 2], "label": ["a", None]}, index=[10.0, 20.0])
    m.set_data(df)
    p.fit()
    p.to_png(backend="cpu")
    assert p.axis("x").view[0] <= 10


def test_options_round_trip_in_snake_case():
    p = fc.Plot(theme="dark")
    m = p.add_mark(
        "cells",
        color_scale={"stops": [(0, "#000000"), (1, "#ffffff")], "domain": (0, 10)},
        value_format={"type": "percent", "decimals": 1},
        cell_size=(1, 2),
        group=3,
        band=0.5,
    )
    o = m.options
    assert o["color_scale"] == {"stops": [(0.0, "#000000"), (1.0, "#ffffff")], "domain": (0.0, 10.0)}
    assert o["value_format"] == {"type": "percent", "decimals": 1}
    assert o["cell_size"] == (1.0, 2.0) and o["group"] == 3 and o["band"] == 0.5
    m.apply_options(group=None, band=None, color_scale="diverging", icon="arrow_down", color="#f00")
    o = m.options
    assert o["group"] is None and o["band"] is None and o["icon"] == "arrow_down"
    assert o["color_scale"]["center"] == 0.0 and o["color"] == "#ff0000"
    m.apply_options(color=None)
    assert m.options["color"] is None
    for bad, msg in [
        ({"colour": "red"}, "unknown mark option 'colour'"),
        ({"icon": "star"}, "'icon' must be"),
        ({"fill_alpha": 2}, "'fill_alpha' must be between 0 and 1"),
        ({"value_format": {"type": "fixed"}}, "'value_format' must be"),
        ({"color_scale": {"stops": [(1, "#000"), (0, "#fff")]}}, "'stops' must be"),
        ({"y_axis": "x"}, "'y_axis' must be 'left' or 'right'"),
        ({"group": -1}, "'group' must be"),
        ({"line_width": True}, "'line_width' must be a number"),
    ]:
        with pytest.raises(ValueError, match=msg):
            m.apply_options(**bad)
    with pytest.raises(ValueError, match="unknown mark kind 'pie'"):
        p.add_mark("pie")


def test_axes():
    p = fc.Plot()
    x = p.axis("x")
    x.apply_options(type="log", title="size", format={"type": "date", "utc_offset_minutes": 60})
    o = x.options
    assert o["type"] == "log" and o["title"] == "size"
    assert o["format"] == {"type": "date", "utc_offset_minutes": 60}
    x.apply_options(type="category", title=None)
    x.set_categories(["a", "b", 3])
    assert x.categories == ["a", "b", "3"] and x.options["title"] is None
    y = p.axis("right")
    y.apply_options(range=(0, 10), inverted=True)
    assert y.options["range"] == (0.0, 10.0)
    y.apply_options(range="auto", include_zero=True, margin=0.1)
    assert y.options["include_zero"] is True
    p.add_mark("line", [5, 6, 7])
    y.set_view(1, 2)
    p.to_png(backend="cpu")
    assert y.view == pytest.approx((1, 2))
    p.fit()
    with pytest.raises(ValueError, match="unknown axis"):
        p.axis("y")
    with pytest.raises(ValueError, match="unknown axis option 'kind'"):
        y.apply_options(kind="log")


def test_rules_and_labels():
    p = fc.Plot()
    r = p.add_rule(2.0, label="target", color="#00ff00")
    assert r.options["label"] == "target" and r.options["axis"] == "right"
    r.apply_options(to=3.0, label=None)
    assert r.options["to"] == 3.0 and r.options["label"] is None
    v = p.add_rule(dt.datetime(2024, 1, 1), axis="x")
    assert v.options["value"] == 1_704_067_200_000
    r.remove()
    with pytest.raises(ValueError, match="removed"):
        r.options
    lab = p.add_label(1, 2, "hi", icon="arrow_up", offset=(4, -2))
    lab.remove()
    with pytest.raises(ValueError):
        lab.remove()
    with pytest.raises(ValueError, match="unknown label option"):
        p.add_label(0, 0, size=3)


def test_update_last_and_append():
    p = fc.Plot()
    m = p.add_mark("line", {"x": [0, 1, 2], "y": [1, 2, 3]})
    m.append(x=[3, 4], y=[4, 5])
    m.update_last(4, 9.0)
    p.fit()
    p.to_png(backend="cpu")
    assert p.axis("right").view[1] >= 9
    m.remove()
    assert p.marks == []
    with pytest.raises(ValueError, match="removed"):
        m.kind


def test_to_png_draws_the_marks():
    p = fc.Plot(width=120, height=80)
    p.add_mark("bars", [1, 1, 1], color="#ff0000", band=1)
    p.axis("right").apply_options(visible=False, range=(0, 1))
    p.axis("x").apply_options(visible=False)
    png = p.to_png(backend="cpu")
    assert png_size(png) == (120, 80)
    px = pixels(png)
    red = (px[..., 0] > 200) & (px[..., 1] < 60) & (px[..., 2] < 60)
    assert red.mean() > 0.5
    assert png_size(p.to_png(width=60, height=40, scale=2, backend="cpu")) == (120, 80)
    assert p.size == (120, 80)


def test_events_go_to_callbacks_with_marks():
    p = fc.Plot()
    m = p.add_mark("scatter", [1, 2])
    seen = []
    p.on("click", seen.append)

    @p.on("hover")
    def boom(e):
        raise RuntimeError("callback error")

    p._emit("click", {"hit": {"mark": m.id, "index": 1, "x": 1.0, "y": 2.0,
                              "position": {"x": 3.0, "y": 4.0}}, "x": 1.0, "y": 2.0})
    p._emit("hover", {"hit": None})  # printed, not raised
    assert seen[0]["hit"]["mark"] == m
    p.off("click")
    p._emit("click", {"hit": None, "x": 0, "y": 0})
    assert len(seen) == 1
    with pytest.raises(ValueError, match="unknown event"):
        p.on("crosshair_move", print)


def test_bad_construction():
    with pytest.raises(ValueError, match="theme"):
        fc.Plot(theme="blue")
    with pytest.raises(ValueError, match="width and height"):
        fc.Plot(width=0)
    assert "Plot 800×500" in repr(fc.Plot())
