"""The public `Chart`, `Series`, `Indicator`, `PriceLine` and `Drawings`
classes. Options and parameters are keyword arguments with
the snake_case names of the Rust API and `docs/book/src/indicators.md`; the native
module checks them (unknown names are errors)."""

from __future__ import annotations

import os
import threading
import traceback
import weakref
from typing import Any, Callable, Iterable, Mapping

from . import _data, _jupyter
from . import _native
from ._jupyter import camel

#: Accepted `kind`s of `Chart.add_series`.
SERIES_KINDS = (
    "candlestick",
    "ohlc",
    "heikin_ashi",
    "line",
    "area",
    "baseline",
    "histogram",
    "points",
)


#: Events of `Chart.on`.
EVENTS = (
    "click",
    "crosshair_move",
    "visible_range_changed",
    "need_more_history",
    "drawing_created",
    "drawing_modified",
    "drawing_removed",
    "drawing_selected",
    "warning",
)


def _dispatch(ref: "weakref.ref[Chart]", name: str, payload: dict[str, Any]) -> None:
    """The native event handler of a chart (holds it weakly: no cycle)."""
    chart = ref()
    if chart is not None:
        chart._emit(name, payload)


def _params(params: Mapping[str, Any]) -> dict[str, Any]:
    """Indicator/transform parameters, datetimes as ms."""
    return {k: _data.time_ms(v) if _data.is_time(v) else v for k, v in params.items()}


class Chart:
    """A financial chart.

    >>> import ferrochart as fc
    >>> chart = fc.Chart(theme="dark", width=800, height=450)
    >>> candles = chart.add_series("candlestick")
    >>> candles.set_data(fc.random_walk(1, 500))
    >>> sma = chart.add_indicator(candles, "sma", n=50)
    >>> png = chart.to_png()   # PNG bytes; chart.to_png("chart.png") writes a file

    `attribution_logo` shows a small FerroChart logo at the bottom-left of
    the chart in Jupyter only; native windows and `to_png` never draw it.
    """

    def __init__(
        self,
        theme: str = "light",
        width: float = 1200,
        height: float = 720,
        *,
        attribution_logo: bool = True,
    ) -> None:
        self._native = _native.Chart(theme, float(width), float(height))
        self._theme = theme
        # Creation options of the Jupyter front end.
        self._options = {"attributionLogo": bool(attribution_logo)}
        # Operations for the Jupyter front end (in a kernel only).
        self._rec = _jupyter.Recorder() if _jupyter.recording() else None
        self._next_line = 0
        self._drawings: Drawings | None = None
        self._handlers: dict[str, list[Callable[[dict[str, Any]], Any]]] = {}
        ref = weakref.ref(self)
        self._native.set_event_handler(lambda name, payload: _dispatch(ref, name, payload))

    def __repr__(self) -> str:
        w, h = self.size
        return f"<ferrochart.Chart {w:g}×{h:g}, {len(self.panes)} pane(s)>"

    def _record(self, op: dict[str, Any], buffer: bytes | None = None) -> None:
        if self._rec is not None:
            self._rec.record(op, buffer)

    # Series and panes -----------------------------------------------------

    def add_series(self, kind: str = "candlestick", pane: int | None = None, **options: Any) -> Series:
        """Adds a series of `kind` (see `SERIES_KINDS`) to `pane` (default:
        the main pane). Options: `color`, `down_color`, `top_fill`,
        `bottom_fill` (CSS colors, None for the theme), `line_width`,
        `line_style` ('solid', 'dashed', 'dotted'), `step`, `base_value`,
        `last_value_visible`, `price_line_visible`, `shift`,
        `price_precision` (0 to 12 decimals; None, the default, is automatic:
        at least 2 decimals and 4 significant digits, with subscript zeros
        for small prices such as 0.0₄1234)."""
        s = Series(self, self._native.add_series(kind, options, pane))
        self._record({"op": "addSeries", "id": s.id, "kind": camel(kind),
                      "options": _jupyter.series_options(options), "pane": pane})
        return s

    def add_volume_series(self, source: Series, pane: int | None = None) -> Series:
        """A volume histogram of the `volume` column of `source` (default: in
        the bottom quarter of the main pane)."""
        s = Series(self, self._native.add_volume_series(source._native, pane))
        self._record({"op": "addVolume", "id": s.id, "source": source.id, "pane": pane})
        return s

    def add_transform_series(
        self,
        name: str,
        pane: int | None = None,
        options: Mapping[str, Any] | None = None,
        **params: Any,
    ) -> Series:
        """Adds a series drawing a transform of the OHLC bars it is given:
        'renko' (`box_size` or `atr`, `kind`), 'range_bars' (`range`, `kind`),
        'kagi' (`reversal` or `reversal_percent`) or 'point_figure'
        (`box_size`, `reversal`). `options` are series options."""
        p, o = _params(params), dict(options or {})
        s = Series(self, self._native.add_transform_series(name, p, o, pane))
        self._record({"op": "addTransform", "id": s.id, "name": camel(name),
                      "params": _jupyter.params(p), "options": _jupyter.series_options(o), "pane": pane})
        return s

    @property
    def main_pane(self) -> int:
        """Id of the top pane."""
        return self._native.main_pane

    @property
    def panes(self) -> list[int]:
        """Pane ids, top to bottom."""
        return self._native.panes

    def add_pane(self, stretch: float = 1.0) -> int:
        """Adds a pane below the others (`stretch`: height relative to the
        other panes); returns its id."""
        pane = self._native.add_pane(float(stretch))
        self._record({"op": "addPane", "id": pane, "stretch": float(stretch)})
        return pane

    def remove_pane(self, pane: int) -> None:
        self._native.remove_pane(pane)
        self._record({"op": "removePane", "pane": pane})

    def add_indicator(
        self, series: Series, name: str, placement: str | int | None = None, **params: Any
    ) -> Indicator:
        """Attaches indicator `name` (see `indicator_names()`) computed from
        `series`, with the parameters of `docs/book/src/indicators.md` (missing ones take
        the defaults): `add_indicator(s, "macd", fast=12, slow=26)`.
        `placement`: None (the indicator's default), 'overlay', 'new_pane',
        'volume' or a pane id."""
        p = _params(params)
        ind = Indicator(self, self._native.add_indicator(series._native, name, p, placement))
        self._record({"op": "addIndicator", "id": ind.id, "source": series.id, "name": camel(name),
                      "params": _jupyter.params(p), "placement": _jupyter.placement(placement),
                      "outputs": [s.id for s in ind.outputs]})
        return ind

    # Price scale and view -------------------------------------------------

    def price_scale(self, pane: int | None = None) -> dict[str, Any]:
        """Options of the price scale of `pane` (default: the main pane)."""
        return self._native.price_scale(pane)

    def apply_price_scale_options(self, pane: int | None = None, **options: Any) -> None:
        """`mode` ('normal', 'logarithmic', 'percentage', 'indexed_to_100'),
        `auto_scale`, `inverted`, `margins` (top, bottom), `range` (low,
        high)."""
        self._native.apply_price_scale_options(options, pane)
        self._record({"op": "priceScale", "options": _jupyter.price_scale_options(options), "pane": pane})

    def fit_content(self) -> None:
        """Shows all data."""
        self._native.fit_content()
        self._record({"op": "fitContent"})

    def scroll_to_realtime(self) -> None:
        """Scrolls to the last bar."""
        self._native.scroll_to_realtime()
        self._record({"op": "scrollToRealtime"})

    @property
    def bar_spacing(self) -> float:
        """Distance between bars, CSS px."""
        return self._native.bar_spacing

    @bar_spacing.setter
    def bar_spacing(self, spacing: float) -> None:
        self._native.set_bar_spacing(float(spacing))
        self._record({"op": "setBarSpacing", "value": float(spacing)})

    @property
    def right_offset(self) -> float:
        """Empty space right of the last bar, in bars."""
        return self._native.right_offset

    @right_offset.setter
    def right_offset(self, bars: float) -> None:
        self._native.set_right_offset(float(bars))
        self._record({"op": "setRightOffset", "value": float(bars)})

    @property
    def crosshair_mode(self) -> str:
        """'normal', 'magnet' or 'hidden'."""
        return self._native.crosshair_mode

    @crosshair_mode.setter
    def crosshair_mode(self, mode: str) -> None:
        self._native.set_crosshair_mode(mode)
        self._record({"op": "setCrosshairMode", "value": mode})

    def data_range(self) -> tuple[int, int] | None:
        """Logical indices of the first and last bar, None without data."""
        return self._native.data_range()

    def visible_logical_range(self) -> tuple[float, float]:
        """Logical indices at the plot edges (fractional)."""
        return self._native.visible_logical_range()

    def set_visible_logical_range(self, start: float, end: float) -> None:
        """Shows logical indices `start..end` across the plot. Like
        `fit_content`, it is applied when the chart is next laid out (the
        next `to_png` or window frame), so `visible_logical_range()` reports
        it after that."""
        self._native.set_visible_logical_range(float(start), float(end))
        self._record({"op": "setVisibleRange", "from": float(start), "to": float(end)})

    # Drawings and state -----------------------------------------------------

    @property
    def drawings(self) -> Drawings:
        """The drawings of the chart (trend lines, Fibonacci tools, texts…)."""
        if self._drawings is None:
            self._drawings = Drawings(self, self._native.drawings)
        return self._drawings

    def save_state(self) -> dict[str, Any]:
        """The view (bar spacing, right offset, crosshair mode), the panes'
        heights and price scales, and the drawings, as a JSON-compatible
        dict. Data, series and indicators are not included: rebuild those,
        then `load_state`."""
        state: dict[str, Any] = self._native.save_state()
        return state

    def load_state(self, state: Mapping[str, Any]) -> None:
        """Loads a dict from `save_state` into a chart with the same panes.
        The drawings replace the current ones and the drawing undo history is
        cleared. Raises ValueError, leaving the chart unchanged, if the state
        does not fit."""
        self._native.load_state(dict(state))
        if self._rec is not None:
            self._record({"op": "loadState", "state": _jupyter.state(self._native.save_state()),
                          "drawingIds": self.drawings.ids()})

    # Events -----------------------------------------------------------------

    def on(self, event: str, callback: Callable[[dict[str, Any]], Any] | None = None) -> Any:
        """Calls `callback(payload)` on `event` (see `EVENTS`) while the chart
        is shown. Usable as a decorator: `@chart.on("click")`.

        Payloads (dicts): `click`: `x`, `y` (CSS px), `pane`, `logical_index`,
        `time` (ms or None), `price`, `target` (None or a dict with `type`
        'series', 'marker' or 'drawing' and `series` / `index` / `id`);
        `crosshair_move`: `point` (None or `logical_index`, `time`, `pane`,
        `price`, `bar`); `visible_range_changed`: `from`, `to`;
        `need_more_history`: `bars_before_start`; `drawing_*`: `id`.

        With `show()` callbacks run on the main thread during `show`; with
        `show(block=False)` they are queued and run on the main thread
        between bytecodes, or by `ferrochart.process_events()`. An exception
        in a callback is printed and does not close the window."""
        if event not in EVENTS:
            raise ValueError(f"unknown event '{event}' (expected one of: {', '.join(EVENTS)})")

        def register(cb: Callable[[dict[str, Any]], Any]) -> Callable[[dict[str, Any]], Any]:
            if not callable(cb):
                raise TypeError("the callback must be callable")
            self._handlers.setdefault(event, []).append(cb)
            self._sync_events()
            return cb

        return register if callback is None else register(callback)

    def off(self, event: str, callback: Callable[[dict[str, Any]], Any] | None = None) -> None:
        """Removes `callback` from `event`, or every callback of it."""
        if callback is None:
            self._handlers.pop(event, None)
        elif callback in self._handlers.get(event, []):
            self._handlers[event].remove(callback)
        self._sync_events()

    def _sync_events(self) -> None:
        if self._rec is not None and self._rec.widget is not None:
            self._rec.widget.events = sorted(e for e, cbs in self._handlers.items() if cbs)

    def _series_of(self, sid: int) -> Series | int:
        native = self._native.series_by_id(sid)
        return sid if native is None else Series(self, native)

    def _emit(self, name: str, payload: dict[str, Any]) -> None:
        handlers = list(self._handlers.get(name, ()))
        if not handlers:
            return
        target = payload.get("target")
        if isinstance(target, dict) and "series" in target:
            target["series"] = self._series_of(target["series"])
        for cb in handlers:
            try:
                cb(payload)
            except Exception:
                traceback.print_exc()

    # Jupyter ----------------------------------------------------------------

    def widget(self) -> Any:
        """The Jupyter widget showing this chart (anywidget; one per chart,
        every display of it is a view). Notebooks show it for a chart that is
        the last expression of a cell. Needs a chart created in a Jupyter
        kernel and `pip install anywidget`."""
        if self._rec is None:
            raise RuntimeError(
                "this chart was created outside a Jupyter kernel, so it cannot be "
                "rebuilt in the browser; create it in the notebook"
            )
        if not _jupyter.has_frontend():
            raise RuntimeError("this build of ferrochart has no Jupyter front end")
        if self._rec.widget is None:
            self._rec.widget = _jupyter.widget_class()(self, options=self._options)
            self._sync_events()
        return self._rec.widget

    def _repr_mimebundle_(self, include: Any = None, exclude: Any = None) -> Any:
        # The widget's bundle (a (data, metadata) pair), or a PNG image.
        if self._rec is not None and _jupyter.has_frontend():
            try:
                return self.widget()._repr_mimebundle_(include=include, exclude=exclude)
            except ImportError:
                pass
        png = self.to_png()
        return {"image/png": png} if png is not None else {}

    # Output ---------------------------------------------------------------

    def show(
        self,
        block: bool = True,
        title: str = "FerroChart",
        width: float | None = None,
        height: float | None = None,
    ) -> None:
        """Opens a native window showing the chart (default size: the
        chart's).

        `show()` blocks until the window is closed and can be called again
        later (in the same script or notebook); it must run on the main
        thread. `show(block=False)` (Linux and Windows) opens the window from
        a background thread and returns at once. The first call of a process
        fixes the mode. Changes made to the chart while it is shown (from
        callbacks or other threads) are drawn."""
        self._show(block, title, width, height, None)

    def _show(self, block: bool, title: str, width: float | None, height: float | None,
              close_after: float | None) -> None:
        if block and threading.current_thread() is not threading.main_thread():
            raise RuntimeError(
                "show() must be called from the main thread "
                "(show(block=False) works from any thread on Linux and Windows)"
            )
        self._native.show(
            block,
            title,
            None if width is None else float(width),
            None if height is None else float(height),
            close_after,
        )

    @property
    def shown(self) -> bool:
        """Whether the chart is in a window."""
        return self._native.shown


    @property
    def size(self) -> tuple[float, float]:
        """`(width, height)` in CSS px."""
        return self._native.size

    def resize(self, width: float, height: float) -> None:
        self._native.resize(float(width), float(height))
        self._record({"op": "resize", "width": float(width), "height": float(height)})

    def to_png(
        self,
        path: str | os.PathLike[str] | None = None,
        width: float | None = None,
        height: float | None = None,
        *,
        scale: float = 1.0,
        backend: str = "auto",
    ) -> bytes | None:
        """Renders the chart offscreen as PNG: `width` × `height` CSS px
        (default: the chart's size) at `scale` pixels per CSS px. `backend`:
        'auto' (GPU, CPU without a GPU adapter), 'gpu' or 'cpu'. Writes
        `path`, or returns the bytes without one."""
        png, _ = self._native.render_png(
            None if width is None else float(width),
            None if height is None else float(height),
            float(scale),
            backend,
        )
        if path is None:
            return png
        with open(path, "wb") as f:
            f.write(png)
        return None


class Series:
    """A series of a `Chart` (from `Chart.add_series` and friends)."""

    def __init__(self, chart: Chart, native: Any) -> None:
        self._chart = chart
        self._native = native

    def __repr__(self) -> str:
        try:
            return f"<ferrochart.Series {self.id} {self.kind}>"
        except ValueError:
            return f"<ferrochart.Series {self.id} (removed)>"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Series) and self._native == other._native

    def __hash__(self) -> int:
        return hash(self._native)

    @property
    def id(self) -> int:
        return self._native.id

    @property
    def kind(self) -> str:
        return self._native.kind

    @property
    def pane(self) -> int:
        return self._native.pane

    def _put(self, data: Any, append: bool) -> None:
        if self._native.is_value:
            c = _data.value_columns(data)
            self._native.set_values(c["time"], c["value"], c.get("color"), append)
            if self._chart._rec is not None and c.get("color") is not None:
                c = {**c, "color": _jupyter.css_to_u32(c["color"])}
        else:
            c = _data.ohlc_columns(data)
            self._native.set_ohlc(
                c["time"], c["open"], c["high"], c["low"], c["close"], c.get("volume"), append
            )
        if self._chart._rec is not None:
            op = "append" if append else "setData"
            self._chart._record({"op": op, "series": self.id}, _jupyter.encode_columns(c))

    def set_data(self, data: Any) -> None:
        """Replaces all data. `data` is a pandas or polars DataFrame or a dict
        of arrays: `time`, `open`, `high`, `low`, `close`, optional `volume`
        for OHLC series; `time`, `value`, optional `color` for the others.
        `time` is datetime64 (naive: UTC) or integer milliseconds; a pandas
        DatetimeIndex is used when there is no `time` column."""
        self._put(data, append=False)

    def append(self, data: Any) -> None:
        """Appends bars or points after the last one (same forms as
        `set_data`)."""
        self._put(data, append=True)

    def update_last(
        self,
        time: Any,
        value: float | None = None,
        *,
        open: float | None = None,
        high: float | None = None,
        low: float | None = None,
        close: float | None = None,
        volume: float | None = None,
    ) -> None:
        """Updates the last bar (same time) or appends one (later time):
        `update_last(t, open=…, high=…, low=…, close=…, volume=…)` for OHLC
        series, `update_last(t, value)` for the others."""
        t = _data.time_ms(time)
        if self._native.is_value:
            if value is None:
                raise TypeError("update_last(time, value) needs a value for this series")
            v = float(value)
            self._native.update_last(t, v, v, v, v, None)
            self._chart._record({"op": "update", "series": self.id, "bar": {"time": t, "value": v}})
            return
        bar = {"open": open, "high": high, "low": low, "close": close}
        missing = [k for k, v in bar.items() if v is None]
        if missing:
            raise TypeError(f"update_last needs {', '.join(missing)} for an OHLC series")
        o, h, lo, c = float(open), float(high), float(low), float(close)  # type: ignore[arg-type]
        v = None if volume is None else float(volume)
        self._native.update_last(t, o, h, lo, c, v)
        bar: dict[str, Any] = {"time": t, "open": o, "high": h, "low": lo, "close": c}
        if v is not None:
            bar["volume"] = v
        self._chart._record({"op": "update", "series": self.id, "bar": bar})

    @property
    def options(self) -> dict[str, Any]:
        """Current options (see `Chart.add_series`)."""
        return self._native.options()

    def apply_options(self, **options: Any) -> None:
        self._native.apply_options(options)
        self._chart._record({"op": "applyOptions", "series": self.id,
                             "options": _jupyter.series_options(options)})

    def create_price_line(self, price: float, **options: Any) -> PriceLine:
        """A horizontal line at `price`. Options: `color`, `line_width`,
        `line_style`, `axis_label_visible`, `title`."""
        o = {"price": float(price), **options}
        line = PriceLine(self._chart, self._native.create_price_line(o))
        self._chart._record({"op": "createPriceLine", "series": self.id, "id": line._id,
                             "options": _jupyter.camel_keys(o)})
        return line

    def set_markers(self, markers: Iterable[Mapping[str, Any]]) -> None:
        """Replaces the markers: dicts with `time`, `position` ('above_bar',
        'below_bar'), `shape` ('circle', 'square', 'arrow_up', 'arrow_down'),
        `color`, `text`."""
        out = []
        for m in markers:
            m = dict(m)
            if "time" in m:
                m["time"] = _data.time_ms(m["time"])
            out.append(m)
        self._native.set_markers(out)
        self._chart._record({"op": "setMarkers", "series": self.id, "markers": _jupyter.markers(out)})

    @property
    def markers(self) -> list[dict[str, Any]]:
        return self._native.markers()

    def remove(self) -> None:
        self._native.remove()
        self._chart._record({"op": "removeSeries", "series": self.id})


class Indicator:
    """An indicator of a `Chart` (from `Chart.add_indicator`)."""

    def __init__(self, chart: Chart, native: Any) -> None:
        self._chart = chart
        self._native = native

    def __repr__(self) -> str:
        return f"<ferrochart.Indicator {self.id}>"

    @property
    def id(self) -> int:
        return self._native.id

    @property
    def outputs(self) -> list[Series]:
        """The series it draws."""
        return [Series(self._chart, s) for s in self._native.outputs]

    def remove(self) -> None:
        self._native.remove()
        self._chart._record({"op": "removeIndicator", "id": self.id})


class PriceLine:
    """A price line (from `Series.create_price_line`)."""

    def __init__(self, chart: Chart, native: Any) -> None:
        self._chart = chart
        self._native = native
        self._id = chart._next_line
        chart._next_line += 1

    @property
    def options(self) -> dict[str, Any]:
        return self._native.options()

    def apply_options(self, **options: Any) -> None:
        self._native.apply_options(options)
        self._chart._record({"op": "applyPriceLine", "id": self._id, "options": _jupyter.camel_keys(options)})

    def remove(self) -> None:
        self._native.remove()
        self._chart._record({"op": "removePriceLine", "id": self._id})


def _anchors(anchors: Iterable[Any]) -> list[dict[str, Any]]:
    """Anchors as `{time (ms), price}` dicts, from dicts or `(time, price)`
    pairs; times may be datetimes."""
    out = []
    for a in anchors:
        if isinstance(a, Mapping):
            a = dict(a)
            if "time" in a and a["time"] is not None:
                a["time"] = _data.time_ms(a["time"])
            out.append(a)
        else:
            try:
                t, p = a
            except (TypeError, ValueError):
                raise TypeError("an anchor is a dict {'time', 'price'} or a (time, price) pair") from None
            out.append({"time": _data.time_ms(t), "price": p})
    return out


def _param(value: Any) -> Any:
    """A drawing parameter as a plain value (sequences: lists of floats)."""
    if isinstance(value, (bool, int, float, str)) or value is None:
        return value
    return [float(x) for x in value]


class Drawings:
    """The drawings of a chart (`Chart.drawings`). Drawings have integer
    ids; changes made here and by the user in a window are undoable.

    >>> line = chart.drawings.add("trend_line", [(t0, 100.0), (t1, 120.0)],
    ...                           style={"line": "#e91e63", "line_width": 2})
    >>> chart.drawings.add("fib_retracement", [(t0, 90), (t1, 130)],
    ...                    params={"levels": [0, 0.5, 1]})

    `kinds()` lists the tools. A drawing's saved form (`get`) is a dict
    `{kind, anchors, style, params}`: anchors `{time (ms), price}`; style
    `line`, `line_width`, `line_dash` ('solid', 'dashed', 'dotted'), `fill`,
    `text` (CSS colors), `font_size`; params depend on the tool (`text` of
    texts and callouts, `levels` of Fibonacci tools)."""

    def __init__(self, chart: Chart, native: Any) -> None:
        self._chart = chart
        self._native = native

    def __repr__(self) -> str:
        return f"<ferrochart.Drawings {len(self)} drawing(s)>"

    def __len__(self) -> int:
        return len(self._native.ids())

    def __iter__(self) -> Any:
        return iter(self._native.ids())

    def _record(self, op: str, **fields: Any) -> None:
        self._chart._record({"op": op, **fields})

    def kinds(self) -> list[str]:
        """Kinds `add` and `start_tool` accept."""
        kinds: list[str] = self._native.kinds()
        return kinds

    def add(
        self,
        kind: str | Mapping[str, Any],
        anchors: Iterable[Any] | None = None,
        *,
        pane: int | None = None,
        style: Mapping[str, Any] | None = None,
        params: Mapping[str, Any] | None = None,
    ) -> int:
        """Adds a drawing of `kind` with `anchors` (dicts `{time, price}` or
        `(time, price)` pairs; times in ms or datetimes) on `pane` (default:
        the main pane); returns its id. `kind` may instead be a saved form
        from `get`."""
        if isinstance(kind, Mapping):
            if anchors is not None or style is not None or params is not None:
                raise TypeError("pass either a saved drawing or kind, anchors, style and params")
            data = dict(kind)
            data["anchors"] = _anchors(data.get("anchors") or [])
        else:
            if anchors is None:
                raise TypeError("add(kind, anchors) needs anchors")
            data = {"kind": kind, "anchors": _anchors(anchors)}
            if style is not None:
                data["style"] = dict(style)
            if params is not None:
                data["params"] = params
        if data.get("params") is not None:
            data["params"] = {k: _param(v) for k, v in dict(data["params"]).items()}
        did: int = self._native.add(data, pane)
        self._record("addDrawing", id=did, data=_jupyter.drawing_data(data), pane=pane)
        return did

    def remove(self, id: int) -> None:
        self._native.remove(id)
        self._record("removeDrawing", id=id)

    def ids(self) -> list[int]:
        """Ids, bottom to top."""
        ids: list[int] = self._native.ids()
        return ids

    def get(self, id: int) -> dict[str, Any] | None:
        """The saved form of a drawing, None for an unknown id."""
        data: dict[str, Any] | None = self._native.get(id)
        return data

    def pane_of(self, id: int) -> int | None:
        pane: int | None = self._native.pane_of(id)
        return pane

    def set_anchors(self, id: int, anchors: Iterable[Any]) -> None:
        """Moves the anchors (as many as the drawing has)."""
        a = _anchors(anchors)
        self._native.set_anchors(id, a)
        self._record("drawingAnchors", id=id, anchors=a)

    def set_style(self, id: int, **style: Any) -> None:
        """Changes some style keys: `set_style(id, line="#ff0000")`."""
        self._native.set_style(id, style)
        self._record("drawingStyle", id=id, style=_jupyter.camel_keys(style))

    def set_param(self, id: int, key: str, value: Any) -> None:
        """Sets a tool parameter, e.g. `'text'` or `'levels'`."""
        value = _param(value)
        self._native.set_param(id, key, value)
        self._record("drawingParam", id=id, key=key, value=value)

    def set_visible(self, id: int, visible: bool) -> None:
        self._native.set_visible(id, bool(visible))
        self._record("drawingVisible", id=id, value=bool(visible))

    def is_visible(self, id: int) -> bool | None:
        """Whether a drawing is shown, None for an unknown id."""
        v: bool | None = self._native.is_visible(id)
        return v

    def set_locked(self, id: int, locked: bool) -> None:
        """Locks a drawing against editing (it can still be selected and
        removed)."""
        self._native.set_locked(id, bool(locked))
        self._record("drawingLocked", id=id, value=bool(locked))

    def is_locked(self, id: int) -> bool | None:
        v: bool | None = self._native.is_locked(id)
        return v

    def bring_to_front(self, id: int) -> None:
        self._native.bring_to_front(id)
        self._record("drawingFront", id=id)

    def send_to_back(self, id: int) -> None:
        self._native.send_to_back(id)
        self._record("drawingBack", id=id)

    def select(self, id: int | None) -> None:
        """Selects a drawing (None clears the selection)."""
        self._native.select(id)
        self._record("selectDrawing", id=id)

    @property
    def selected(self) -> int | None:
        sel: int | None = self._native.selected
        return sel

    def start_tool(self, kind: str) -> None:
        """Arms a tool: the next clicks on the chart (in a window or the
        notebook) create a drawing of `kind`."""
        self._native.start_tool(kind)
        self._record("startTool", kind=kind)

    def cancel_tool(self) -> None:
        """Cancels the armed tool or the drawing being created."""
        self._native.cancel_tool()
        self._record("cancelTool")

    @property
    def active_tool(self) -> str | None:
        tool: str | None = self._native.active_tool
        return tool

    def undo(self) -> bool:
        """Undoes the last drawing change; False if there was none."""
        done: bool = self._native.undo()
        if done:
            self._record("undoDrawing")
        return done

    def redo(self) -> bool:
        """Redoes the last undone change; False if there was none."""
        done: bool = self._native.redo()
        if done:
            self._record("redoDrawing")
        return done

    @property
    def can_undo(self) -> bool:
        v: bool = self._native.can_undo
        return v

    @property
    def can_redo(self) -> bool:
        v: bool = self._native.can_redo
        return v
