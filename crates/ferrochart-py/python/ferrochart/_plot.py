"""The public `Plot`, `Mark`, `Axis`, `Rule` and `Label` classes:
plots with numeric, category or date axes (lines, areas, scatter, bars,
waterfalls, heatmaps), shown the same three ways as a `Chart` (a native
window, Jupyter, PNG). Options are keyword arguments in snake_case; the
native module checks them (unknown names are errors)."""

from __future__ import annotations

import os
import threading
import traceback
import weakref
from typing import Any, Callable, Iterable, Mapping

from . import _data, _jupyter
from . import _native

#: Accepted `kind`s of `Plot.add_mark`.
MARK_KINDS = ("line", "area", "scatter", "bars", "waterfall", "cells")

#: Events of `Plot.on`.
PLOT_EVENTS = ("hover", "click", "view_change")

#: Axes of `Plot.axis`.
AXES = ("x", "left", "right")


def _dispatch(ref: "weakref.ref[Plot]", name: str, payload: dict[str, Any]) -> None:
    """The native event handler of a plot (holds it weakly: no cycle)."""
    plot = ref()
    if plot is not None:
        plot._emit(name, payload)


def _positions(options: Mapping[str, Any], keys: Iterable[str]) -> dict[str, Any]:
    """`options` with datetimes under `keys` as UTC ms."""
    out = dict(options)
    for k in keys:
        if k in out and out[k] is not None:
            out[k] = _data.value(out[k])
    return out


class Plot:
    """A plot: marks on numeric, category or date axes.

    >>> import ferrochart as fc
    >>> plot = fc.Plot(width=640, height=400)
    >>> bars = plot.add_mark("bars", [3, 1, 4, 1, 5], color="#2196f3")
    >>> plot.axis("x").set_categories(["a", "b", "c", "d", "e"])
    >>> plot.axis("x").apply_options(type="category")
    >>> line = plot.add_mark("line", {"x": [0, 4], "y": [2, 4]})
    >>> png = plot.to_png()   # PNG bytes; plot.to_png("plot.png") writes a file

    The right axis shows values; the left one is hidden until a mark uses it
    (`y_axis="left"`) and it is made visible."""

    def __init__(
        self,
        theme: str = "light",
        width: float = 800,
        height: float = 500,
        *,
        crosshair: bool = False,
        hover_highlight: bool = True,
        attribution_logo: bool = True,
    ) -> None:
        self._native = _native.Plot(theme, float(width), float(height), bool(crosshair), bool(hover_highlight))
        self._theme = theme
        # Creation options of the Jupyter front end; `attribution_logo` is
        # the FerroChart logo there (never in native windows or `to_png`).
        self._options = {
            "crosshair": bool(crosshair),
            "hoverHighlight": bool(hover_highlight),
            "attributionLogo": bool(attribution_logo),
        }
        self._rec = _jupyter.Recorder() if _jupyter.recording() else None
        self._handlers: dict[str, list[Callable[[dict[str, Any]], Any]]] = {}
        ref = weakref.ref(self)
        self._native.set_event_handler(lambda name, payload: _dispatch(ref, name, payload))

    def __repr__(self) -> str:
        w, h = self.size
        return f"<ferrochart.Plot {w:g}×{h:g}, {len(self.marks)} mark(s)>"

    def _record(self, op: dict[str, Any], buffer: bytes | None = None) -> None:
        if self._rec is not None:
            self._rec.record(op, buffer)

    # Marks, axes, rules, labels ---------------------------------------------

    def add_mark(self, kind: str, data: Any = None, **options: Any) -> Mark:
        """Adds a mark of `kind` (see `MARK_KINDS`) drawn above the earlier
        ones, with optional `data` (as `Mark.set_data`).

        Options: `y_axis` ('right' or 'left'), `orientation` ('vertical',
        'horizontal': bars and waterfalls), `color`, `down_color`,
        `total_color` (CSS colors; None: palette or theme), `fill_alpha`,
        `line_width`, `line_style` ('solid', 'dashed', 'dotted'),
        `step_line`, `point_size`, `icon` ('circle', 'square', 'arrow_up',
        'arrow_down'), `base_value`, `group` and `stack` (marks sharing a
        number sit side by side or add up; None for none), `band` (fraction
        of a slot; None: default), `color_scale` ('sequential', 'diverging'
        or a dict with `stops` [(position, color), …], `center`, `domain`
        (lo, hi), `quantile`), `show_values`, `value_format` (as axis
        `format`), `cell_size` ((w, h) in data units, None: automatic),
        `hoverable`, `visible`."""
        m = Mark(self, self._native.add_mark(kind, options))
        self._record({"op": "addMark", "id": m.id, "kind": kind,
                      "options": _jupyter.plot_options(options)})
        if data is not None:
            m.set_data(data)
        return m

    @property
    def marks(self) -> list[Mark]:
        """The marks, bottom to top."""
        return [Mark(self, self._native.mark_by_id(i)) for i in self._native.mark_ids()]

    def _mark_of(self, mid: int) -> Mark | int:
        native = self._native.mark_by_id(mid)
        return mid if native is None else Mark(self, native)

    def axis(self, name: str) -> Axis:
        """Axis `'x'`, `'left'` (hidden by default) or `'right'`."""
        if name not in AXES:
            raise ValueError(f"unknown axis '{name}' (expected 'x', 'left' or 'right')")
        return Axis(self, name)

    def add_rule(self, value: Any, axis: str = "right", **options: Any) -> Rule:
        """A line across the plot at `value` of `axis` ('x' gives a vertical
        line), or a band up to `to`. Options: `to`, `color`, `line_width`,
        `line_style`, `label` (text on the axis), `label_background`.
        Values may be datetimes."""
        o = _positions({"axis": axis, "value": value, **options}, ("value", "to"))
        r = Rule(self, self._native.add_rule(o))
        self._record({"op": "addRule", "id": r.id, "options": _jupyter.plot_options(o)})
        return r

    def add_label(self, x: Any, y: Any, text: str = "", **options: Any) -> Label:
        """A text or symbol at data position (`x`, `y`). Options: `y_axis`,
        `icon` ('circle', 'square', 'arrow_up', 'arrow_down' or None),
        `color`, `align` ('left', 'center', 'right'), `offset` (dx, dy) in
        CSS px."""
        o = _positions({"x": x, "y": y, "text": text, **options}, ("x", "y"))
        lab = Label(self, self._native.add_label(o))
        self._record({"op": "addLabel", "id": lab.id, "options": _jupyter.plot_options(o)})
        return lab

    def fit(self) -> None:
        """Shows all data: automatic axes follow the data again."""
        self._native.fit()
        self._record({"op": "fit"})

    # Events -----------------------------------------------------------------

    def on(self, event: str, callback: Callable[[dict[str, Any]], Any] | None = None) -> Any:
        """Calls `callback(payload)` on `event` (see `PLOT_EVENTS`) while the
        plot is shown. Usable as a decorator: `@plot.on("click")`.

        Payloads: `hover`: `hit`; `click`: `hit`, `x`, `y` (axis values at
        the pointer: `y` on the right axis, the left one if the right is
        hidden); `view_change`: nothing. `hit` is None or a dict with
        `mark` (a `Mark`), `index` (of the point in the mark's data), `x`,
        `y` and `position` ({x, y} in CSS px). Callbacks run as for
        `Chart.on`."""
        if event not in PLOT_EVENTS:
            raise ValueError(f"unknown event '{event}' (expected one of: {', '.join(PLOT_EVENTS)})")

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

    def _emit(self, name: str, payload: dict[str, Any]) -> None:
        handlers = list(self._handlers.get(name, ()))
        if not handlers:
            return
        hit = payload.get("hit")
        if isinstance(hit, dict) and isinstance(hit.get("mark"), int):
            hit["mark"] = self._mark_of(hit["mark"])
        for cb in handlers:
            try:
                cb(payload)
            except Exception:
                traceback.print_exc()

    # Jupyter ----------------------------------------------------------------

    def widget(self) -> Any:
        """The Jupyter widget showing this plot (see `Chart.widget`)."""
        if self._rec is None:
            raise RuntimeError(
                "this plot was created outside a Jupyter kernel, so it cannot be "
                "rebuilt in the browser; create it in the notebook"
            )
        if not _jupyter.has_frontend():
            raise RuntimeError("this build of ferrochart has no Jupyter front end")
        if self._rec.widget is None:
            self._rec.widget = _jupyter.widget_class()(self, kind="plot", options=self._options)
            self._sync_events()
        return self._rec.widget

    def _repr_mimebundle_(self, include: Any = None, exclude: Any = None) -> Any:
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
        """Opens a native window showing the plot; see `Chart.show`."""
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
        """Whether the plot is in a window."""
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
        """Renders the plot offscreen as PNG; see `Chart.to_png`."""
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


class Mark:
    """A mark of a `Plot` (from `Plot.add_mark`)."""

    def __init__(self, plot: Plot, native: Any) -> None:
        self._plot = plot
        self._native = native

    def __repr__(self) -> str:
        try:
            return f"<ferrochart.Mark {self.id} {self.kind}>"
        except ValueError:
            return f"<ferrochart.Mark {self.id} (removed)>"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Mark) and self._native == other._native

    def __hash__(self) -> int:
        return hash(self._native)

    @property
    def id(self) -> int:
        return self._native.id

    @property
    def kind(self) -> str:
        return self._native.kind

    def _put(self, data: Any, columns: Mapping[str, Any], append: bool) -> None:
        c = _data.mark_columns(data, columns)
        self._native.set_data(
            c["x"], c["y"], c.get("y0"), c.get("value"), c.get("size"),
            c.get("color"), c.get("step"), c.get("label"), append,
        )
        if self._plot._rec is not None:
            numeric = {k: c[k] for k in ("x", "y", "y0", "value", "size") if k in c}
            if "color" in c:
                numeric["color"] = _jupyter.css_to_u32(c["color"])
            op: dict[str, Any] = {"op": "append" if append else "setData", "mark": self.id}
            for k in ("step", "label"):
                if k in c:
                    op[k] = c[k]
            self._plot._record(op, _jupyter.encode_columns(numeric))

    def set_data(self, data: Any = None, **columns: Any) -> None:
        """Replaces all data. `data` is a DataFrame or dict with columns `x`,
        `y` and optional `y0` (lower edge of areas, start of bars), `value`
        (colored by the color scale), `size` (scatter diameters, CSS px),
        `color` (CSS colors, None for the mark color), `step` ('delta',
        'subtotal', 'total': waterfalls), `label` (texts); a pandas Series
        (x from its index); a list or 1-D array of y values (x = 0, 1, …);
        or an (n, 2) array. Columns may also be keywords:
        `set_data(x=…, y=…)`. `x` may hold datetimes (UTC ms on the axis;
        use a 'date' axis format). NaN in `y` is a gap."""
        self._put(data, columns, append=False)

    def append(self, data: Any = None, **columns: Any) -> None:
        """Appends points (the same columns as the existing data)."""
        self._put(data, columns, append=True)

    def update_last(
        self,
        x: Any,
        y: float,
        *,
        y0: float | None = None,
        value: float | None = None,
        size: float | None = None,
        color: str | None = None,
    ) -> None:
        """Replaces the last point (a live value)."""
        xv = float(_data.value(x))
        self._native.update_last(xv, float(y), y0, value, size, color)
        point: dict[str, Any] = {"x": xv, "y": float(y)}
        for k, v in (("y0", y0), ("value", value), ("size", size), ("color", color)):
            if v is not None:
                point[k] = v
        self._plot._record({"op": "update", "mark": self.id, "point": point})

    @property
    def options(self) -> dict[str, Any]:
        """Current options (see `Plot.add_mark`)."""
        return self._native.options()

    def apply_options(self, **options: Any) -> None:
        """Changes options; others keep their value."""
        self._native.apply_options(options)
        self._plot._record({"op": "markOptions", "mark": self.id,
                            "options": _jupyter.plot_options(options)})

    def remove(self) -> None:
        self._native.remove()
        self._plot._record({"op": "removeMark", "mark": self.id})


class Axis:
    """An axis of a `Plot`: 'x', 'left' or 'right'."""

    def __init__(self, plot: Plot, name: str) -> None:
        self._plot = plot
        self.name = name

    def __repr__(self) -> str:
        return f"<ferrochart.Axis {self.name}>"

    @property
    def options(self) -> dict[str, Any]:
        """Current options (see `apply_options`)."""
        return self._plot._native.axis_options(self.name)

    def apply_options(self, **options: Any) -> None:
        """Changes options; others keep their value. `type` ('linear',
        'log', 'category'; a new type starts from its defaults), `range`
        ('auto' follows the data, (lo, hi) is fixed), `include_zero`,
        `margin` (automatic range), `format` ('auto', 'compact', 'date',
        {'type': 'fixed' | 'percent', 'decimals': n} or {'type': 'date',
        'utc_offset_minutes': m}), `inverted`, `visible`, `grid`, `title`
        (None: none), `locked` (pan and zoom leave it alone)."""
        o = dict(options)
        if isinstance(o.get("range"), (tuple, list)):
            o["range"] = tuple(_data.value(v) for v in o["range"])
        self._plot._native.apply_axis_options(self.name, o)
        self._plot._record({"op": "axis", "axis": self.name, "options": _jupyter.plot_options(o)})

    def set_categories(self, labels: Iterable[Any]) -> None:
        """Labels of a category axis: category `k` is `labels[k]`."""
        names = [str(v) for v in labels]
        self._plot._native.set_categories(self.name, names)
        self._plot._record({"op": "categories", "axis": self.name, "labels": names})

    @property
    def categories(self) -> list[str]:
        return self._plot._native.categories(self.name)

    @property
    def view(self) -> tuple[float, float]:
        """The visible range `(lo, hi)` as of the last frame."""
        return self._plot._native.view(self.name)

    def set_view(self, lo: Any, hi: Any) -> None:
        """Shows `lo..hi` (the axis stops following the data until
        `Plot.fit()`)."""
        a, b = float(_data.value(lo)), float(_data.value(hi))
        self._plot._native.set_view(self.name, a, b)
        self._plot._record({"op": "setView", "axis": self.name, "lo": a, "hi": b})


class Rule:
    """A rule (line or band) of a `Plot` (from `Plot.add_rule`)."""

    def __init__(self, plot: Plot, rid: int) -> None:
        self._plot = plot
        self.id = rid

    def __repr__(self) -> str:
        return f"<ferrochart.Rule {self.id}>"

    @property
    def options(self) -> dict[str, Any]:
        return self._plot._native.rule_options(self.id)

    def apply_options(self, **options: Any) -> None:
        o = _positions(options, ("value", "to"))
        self._plot._native.apply_rule_options(self.id, o)
        self._plot._record({"op": "ruleOptions", "id": self.id, "options": _jupyter.plot_options(o)})

    def remove(self) -> None:
        self._plot._native.remove_rule(self.id)
        self._plot._record({"op": "removeRule", "id": self.id})


class Label:
    """A label of a `Plot` (from `Plot.add_label`)."""

    def __init__(self, plot: Plot, lid: int) -> None:
        self._plot = plot
        self.id = lid

    def __repr__(self) -> str:
        return f"<ferrochart.Label {self.id}>"

    def remove(self) -> None:
        self._plot._native.remove_label(self.id)
        self._plot._record({"op": "removeLabel", "id": self.id})
