"""Tests of the Python binding: data input, options, indicators, PNG output.
Run with `maturin develop` then `pytest crates/ferrochart-py/tests`."""

from __future__ import annotations

import datetime as dt
import struct
from pathlib import Path

import numpy as np
import pytest

import ferrochart as fc

HOUR = 3_600_000
START = 1_700_000_000_000


def bars(n: int = 200, seed: int = 7) -> dict[str, np.ndarray]:
    return fc.random_walk(seed, n, START, HOUR)


def png_size(png: bytes) -> tuple[int, int]:
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    w, h = struct.unpack(">II", png[16:24])
    return w, h


def chart_with(data, kind: str = "candlestick") -> tuple[fc.Chart, fc.Series]:
    c = fc.Chart(width=400, height=300)
    s = c.add_series(kind)
    s.set_data(data)
    return c, s


# Data -----------------------------------------------------------------------


def test_random_walk_matches_the_engine():
    d = bars(5)
    assert d["time"].dtype == np.int64 and d["close"].dtype == np.float64
    assert list(np.diff(d["time"])) == [HOUR] * 4
    assert set(d) == {"time", "open", "high", "low", "close", "volume"}


def test_every_input_form_gives_the_same_chart():
    pd = pytest.importorskip("pandas")
    d = bars()
    reference = chart_with(d)[0].to_png(backend="cpu")
    df = pd.DataFrame(d)
    df["time"] = pd.to_datetime(df["time"], unit="ms")
    forms = {
        "datetime column": df,
        "datetime index": df.set_index("time"),
        "tz-aware index": df.set_index("time").tz_localize("UTC").tz_convert("Asia/Tokyo"),
        "capitalized": df.rename(columns=str.capitalize),
        "lists": {k: v.tolist() for k, v in d.items()},
        "strided": {k: np.repeat(v, 2)[::2] for k, v in d.items()},
        "int32 prices": {**d, "open": d["open"].astype(np.float32)},
        "datetime objects": {
            **d,
            "time": [dt.datetime.fromtimestamp(t / 1000, dt.timezone.utc) for t in d["time"]],
        },
    }
    pl = None
    try:
        import polars as pl
    except ImportError:
        pass
    if pl is not None:
        forms["polars"] = pl.DataFrame({**d, "time": pl.Series(d["time"]).cast(pl.Datetime("ms"))})
    for name, data in forms.items():
        c, s = chart_with(data)
        if name == "int32 prices":
            continue  # float32 rounding changes the opens
        assert c.data_range() == (0, len(d["time"]) - 1), name
        assert c.to_png(backend="cpu") == reference, name


def test_bad_data_is_reported():
    d = bars(10)
    c = fc.Chart()
    s = c.add_series("candlestick")
    with pytest.raises(ValueError, match="missing column"):
        s.set_data({k: v for k, v in d.items() if k != "close"})
    with pytest.raises(ValueError):
        s.set_data({**d, "time": d["time"][::-1].copy()})
    with pytest.raises(ValueError, match="whole milliseconds"):
        s.set_data({**d, "time": d["time"] + 0.5})
    with pytest.raises(TypeError, match="DataFrame or a dict"):
        s.set_data([1, 2, 3])
    with pytest.raises(ValueError):
        s.set_data({**d, "open": d["open"][:5]})


def test_value_series_take_time_value_and_colors():
    d = bars(50)
    c = fc.Chart(width=400, height=300)
    line = c.add_series("histogram", color="#2196f3")
    colors = ["#ff0000" if i % 2 else None for i in range(50)]
    line.set_data({"time": d["time"], "value": d["close"], "color": colors})
    assert c.data_range() == (0, 49)
    with pytest.raises(ValueError, match="missing column"):
        line.set_data(d)
    with pytest.raises(ValueError, match="not a CSS color"):
        line.set_data({"time": d["time"], "value": d["close"], "color": ["nope"] * 50})


def test_append_and_update_last():
    d = bars(20)
    c, s = chart_with({k: v[:10] for k, v in d.items()})
    s.append({k: v[10:15] for k, v in d.items()})
    assert c.data_range() == (0, 14)
    t = int(d["time"][14])
    s.update_last(t, open=1.0, high=3.0, low=0.5, close=2.0, volume=10)
    assert c.data_range() == (0, 14)
    s.update_last(np.datetime64(t + HOUR, "ms"), open=2.0, high=2.5, low=1.5, close=2.2, volume=5)
    assert c.data_range() == (0, 15)
    with pytest.raises(TypeError, match="close"):
        s.update_last(t + 2 * HOUR, open=1.0, high=1.0, low=1.0)
    with pytest.raises(ValueError):
        s.update_last(t - HOUR, open=1.0, high=1.0, low=1.0, close=1.0)
    line = c.add_series("line")
    line.set_data({"time": d["time"][:3], "value": [1.0, 2.0, 3.0]})
    line.update_last(dt.datetime.fromtimestamp(int(d["time"][3]) / 1000, dt.timezone.utc), 4.0)
    with pytest.raises(TypeError, match="value"):
        line.update_last(int(d["time"][4]))


# Options --------------------------------------------------------------------


def test_series_options_round_trip_and_unknown_names_fail():
    c = fc.Chart()
    s = c.add_series("line", color="#ff0000", line_width=2, line_style="dashed")
    o = s.options
    assert o["color"] == "#ff0000" and o["line_width"] == 2 and o["line_style"] == "dashed"
    assert o["down_color"] is None
    s.apply_options(color=None, step=True)
    assert s.options["color"] is None and s.options["step"] is True
    with pytest.raises(ValueError, match="unknown series option 'colour'"):
        c.add_series("line", colour="red")
    with pytest.raises(ValueError, match="not a CSS color"):
        s.apply_options(color="reddish")
    with pytest.raises(ValueError, match="True or False"):
        s.apply_options(step=1)
    with pytest.raises(ValueError, match="unknown series kind 'bars'"):
        c.add_series("bars")
    assert s.kind == "line" and s.pane == c.main_pane
    assert c.add_series("heikin_ashi").kind == "heikin_ashi"


def test_price_lines_markers_and_scales():
    d = bars(50)
    c, s = chart_with(d)
    line = s.create_price_line(100.0, color="#ff9800", title="target")
    assert line.options["price"] == 100.0 and line.options["title"] == "target"
    line.apply_options(price=101.5)
    assert line.options["price"] == 101.5
    line.remove()
    with pytest.raises(ValueError):
        line.options
    when = dt.datetime.fromtimestamp(int(d["time"][10]) / 1000, dt.timezone.utc)
    s.set_markers([{"time": when, "position": "below_bar", "shape": "arrow_up", "text": "buy"}])
    assert s.markers == [
        {"time": int(d["time"][10]), "position": "below_bar", "shape": "arrow_up", "color": None, "text": "buy",
         "size": None, "icon": None, "label": "", "border": None, "border_width": 2.0, "id": 0, "price": None}
    ]
    with pytest.raises(ValueError, match="'shape'"):
        s.set_markers([{"time": 0, "shape": "star"}])
    c.apply_price_scale_options(mode="indexed_to_100", inverted=True, margins=(0.1, 0.2))
    p = c.price_scale()
    assert p["mode"] == "indexed_to_100" and p["inverted"] and p["margins"] == pytest.approx((0.1, 0.2))
    with pytest.raises(ValueError, match="unknown mode"):
        c.apply_price_scale_options(mode="log")


def test_view_controls():
    c, _ = chart_with(bars(300))
    c.bar_spacing = 10
    assert c.bar_spacing == pytest.approx(10)
    c.set_visible_logical_range(100, 200)
    c.to_png(backend="cpu")  # the range waits for the layout of a frame
    lo, hi = c.visible_logical_range()
    assert lo == pytest.approx(100, abs=1) and hi == pytest.approx(200, abs=1)
    with pytest.raises(ValueError, match="from < to"):
        c.set_visible_logical_range(5, 5)
    c.right_offset = 5
    assert c.right_offset == 5
    c.crosshair_mode = "magnet"
    assert c.crosshair_mode == "magnet"
    with pytest.raises(ValueError):
        c.crosshair_mode = "sticky"
    c.fit_content()
    c.scroll_to_realtime()


# Indicators and transforms ----------------------------------------------------


def test_every_indicator_builds_with_defaults():
    c, s = chart_with(bars(300))
    names = fc.indicator_names()
    assert len(names) == 30 and "stoch_rsi" in names and "volume_ma" in names
    for name in names:
        ind = c.add_indicator(s, name)
        assert ind.outputs, name
    assert len(c.panes) > 5


def test_indicator_parameters_use_snake_case():
    c, s = chart_with(bars(300))
    k = c.add_indicator(s, "keltner", n=10, m=1.5, atr_n=5)
    assert len(k.outputs) == 3
    c.add_indicator(s, "sma", n=10, source="hl2", placement="new_pane")
    c.add_indicator(s, "vwap", anchor=dt.datetime(2023, 11, 15), utc_offset_ms=0)
    with pytest.raises(ValueError, match="unknown parameter 'atrN'"):
        c.add_indicator(s, "keltner", atrN=5)
    with pytest.raises(ValueError, match="'atr_n' must be a non-negative integer"):
        c.add_indicator(s, "supertrend", atr_n=2.5)
    with pytest.raises(ValueError, match="unknown indicator 'smaa'"):
        c.add_indicator(s, "smaa")
    with pytest.raises(ValueError, match="placement"):
        c.add_indicator(s, "sma", placement="below")
    panes = len(c.panes)
    macd = c.add_indicator(s, "macd", fast=8)
    assert len(c.panes) == panes + 1
    macd.remove()
    with pytest.raises(ValueError):
        macd.outputs


def test_transform_series():
    c = fc.Chart(width=400, height=300)
    renko = c.add_transform_series("renko", box_size=0.5, kind="ohlc", options={"color": "#00ff00"})
    renko.set_data(bars(500))
    assert renko.kind == "ohlc" and renko.options["color"] == "#00ff00"
    with pytest.raises(ValueError, match="unknown parameter 'box'"):
        fc.Chart().add_transform_series("renko", box=1.0)
    with pytest.raises(ValueError, match="needs one of 'box_size' and 'atr'"):
        fc.Chart().add_transform_series("renko")


# Output ---------------------------------------------------------------------


def test_to_png_sizes_and_backends(tmp_path):
    c, _ = chart_with(bars())
    assert png_size(c.to_png()) == (400, 300)
    assert png_size(c.to_png(width=640, height=360, scale=2)) == (1280, 720)
    assert c.size == (400, 300)  # unchanged
    cpu = c.to_png(backend="cpu")
    assert png_size(cpu) == (400, 300)
    path = tmp_path / "chart.png"
    assert c.to_png(path) is None
    assert path.read_bytes()[:4] == b"\x89PNG"
    with pytest.raises(ValueError, match="backend"):
        c.to_png(backend="metal")
    with pytest.raises(ValueError, match="scale"):
        c.to_png(scale=100)
    with pytest.raises(ValueError, match="out of range"):
        c.to_png(width=0)
    c.resize(320, 200)
    assert png_size(c.to_png(backend="cpu")) == (320, 200)


def test_errors_on_bad_construction():
    with pytest.raises(ValueError, match="theme"):
        fc.Chart(theme="blue")
    with pytest.raises(ValueError, match="width and height"):
        fc.Chart(width=0)


def test_removed_series_report_errors():
    c, s = chart_with(bars(10))
    s.remove()
    with pytest.raises(ValueError, match="removed"):
        s.kind
    assert "removed" in repr(s)


# Image markers and fallback fonts (r21) ---------------------------------------

CJK_FONT = Path(__file__).parents[2] / "ferrochart-render-cpu/tests/fonts/NotoSansSC-subset.otf"


def avatar(n: int = 32, rgb: tuple[int, int, int] = (230, 20, 20)) -> np.ndarray:
    a = np.zeros((n, n, 4), np.uint8)
    a[..., :3] = rgb
    a[..., 3] = 255
    return a


def test_icons_and_image_markers():
    c, s = chart_with(bars(60))
    red = c.add_icon(avatar())
    rgb = c.add_icon(avatar(40)[..., :3])  # RGB: opaque
    assert isinstance(red, int) and red != rgb
    t = int(bars(60)["time"][20])
    s.set_markers([
        {"time": t, "icon": red, "size": 30, "border": "#00ff00", "border_width": 3},
        {"time": t, "label": "DB", "color": "#2fbf71", "position": "below_bar"},
    ])
    s.apply_options(marker_stack_limit=3)
    m = s.markers
    assert m[0]["icon"] == red and m[0]["size"] == 30 and m[0]["border_width"] == 3
    assert m[1]["label"] == "DB" and m[1]["icon"] is None
    assert s.options["marker_stack_limit"] == 3
    with_icon = c.to_png(backend="cpu")
    c.remove_icon(red)
    assert c.to_png(backend="cpu") != with_icon  # the shape only now
    with pytest.raises(ValueError, match="unknown icon"):
        s.set_markers([{"time": t, "icon": red}])
    with pytest.raises(ValueError, match="not both"):
        s.set_markers([{"time": t, "icon": rgb, "label": "x"}])
    with pytest.raises(ValueError, match="marker_stack_limit"):
        s.apply_options(marker_stack_limit=0)
    with pytest.raises(ValueError, match="shape"):
        c.add_icon(np.zeros((4, 4), np.uint8))
    with pytest.raises(ValueError, match="1..=256"):
        c.add_icon(np.zeros((300, 2, 4), np.uint8))


def test_fallback_fonts_draw_cjk():
    d = bars(60)
    t = int(d["time"][30])
    pngs = []
    for fonts in ((), (CJK_FONT,), (CJK_FONT.read_bytes(),)):
        c = fc.Chart(width=400, height=300, fonts=fonts)
        s = c.add_series("candlestick")
        s.set_data(d)
        s.set_markers([{"time": t, "text": "买入"}])
        pngs.append(c.to_png(backend="cpu"))
    # Without the font the characters are boxes; with it (path or bytes) glyphs.
    assert pngs[0] != pngs[1] and pngs[1] == pngs[2]
    with pytest.raises(ValueError, match="font 0"):
        fc.Chart(fonts=[b"not a font"])


def test_marker_ids_and_at_price():
    c, s = chart_with(bars(60))
    t = int(bars(60)["time"][20])
    s.set_markers([
        {"time": t, "id": 7},
        {"time": t, "position": "at_price", "price": 101.5, "id": 2**63, "text": "fill"},
    ])
    m = s.markers
    assert (m[0]["id"], m[0]["price"]) == (7, None)
    assert (m[1]["position"], m[1]["price"], m[1]["id"]) == ("at_price", 101.5, 2**63)
    with pytest.raises(ValueError, match="finite price"):
        s.set_markers([{"time": t, "position": "at_price"}])
    with pytest.raises(ValueError, match="only atPrice"):
        s.set_markers([{"time": t, "price": 1.0}])
    with pytest.raises(ValueError, match="'id'"):
        s.set_markers([{"time": t, "id": -1}])


def test_marker_cluster_option():
    c, s = chart_with(bars(60))
    assert s.options["marker_cluster"] is None
    s.apply_options(marker_cluster=24)
    assert s.options["marker_cluster"] == 24
    s.apply_options(marker_cluster=None)
    assert s.options["marker_cluster"] is None
    with pytest.raises(ValueError, match="marker_cluster"):
        s.apply_options(marker_cluster=0)


def test_bar_flags():
    d = bars(60)
    c, s = chart_with(d)
    before = c.to_png(backend="cpu")
    t = d["time"][[10, 20, 30]]
    s.set_bar_flags(t, filled=[True, False, True], clipped=[False, True, True])
    assert c.to_png(backend="cpu") != before
    s.apply_options(filled_bar_color="#888888")
    assert s.options["filled_bar_color"] is not None
    with pytest.raises(ValueError, match="one value per time"):
        s.set_bar_flags(t, filled=[True])
    _, line = chart_with({"time": d["time"], "value": d["close"]}, "line")
    with pytest.raises(ValueError, match="OHLC"):
        line.set_bar_flags(t, filled=[True, True, True])
