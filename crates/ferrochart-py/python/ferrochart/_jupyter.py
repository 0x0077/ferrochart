"""Jupyter display: an anywidget whose front end runs the
FerroChart WebAssembly build (`FerroChart` for charts, `FerroPlot` for plots).

The Python chart or plot records the calls that change it as operations in the JS
API's terms (camelCase names, `_record`), with data columns in the column
buffer format below. A view asks for the operations so far when it is
rendered (a page reload rebuilds the chart from them); later operations are
sent to every view as they happen. Recording is on inside an IPython kernel
only, so scripts do not keep a second copy of their data.

Column buffer (little-endian), shared with the front end:

    offset 0   4 bytes  magic b"FCCB"
           4   u16      version (1)
           6   u16      number of columns n
           8   u32      number of rows
          12   u32      reserved (0)
          16   n × 16   column descriptors: name (8 bytes ASCII, zero
                        padded), dtype u8 (1 float64, 2 int64, 3 uint32),
                        7 reserved bytes
    16 + 16n            the columns in descriptor order, each padded to a
                        multiple of 8 bytes

so every column starts 8-byte aligned and the front end views it as a
`Float64Array`, `BigInt64Array` or `Uint32Array` without copying.
"""

from __future__ import annotations

import struct
import sys
from importlib import resources
from typing import Any, Mapping

import numpy as np

MAGIC = b"FCCB"
VERSION = 1
_DTYPES = {np.dtype("<f8"): 1, np.dtype("<i8"): 2, np.dtype("<u4"): 3}
_DTYPE_OF = {v: k for k, v in _DTYPES.items()}

#: None: record when running in an IPython kernel; True/False: force.
RECORD: bool | None = None


def recording() -> bool:
    return RECORD if RECORD is not None else "ipykernel" in sys.modules


def encode_columns(columns: Mapping[str, np.ndarray]) -> bytes:
    """Columns of equal length in the column buffer format."""
    names = list(columns)
    rows = len(next(iter(columns.values()))) if names else 0
    head = [MAGIC, struct.pack("<HHII", VERSION, len(names), rows, 0)]
    body = []
    for name in names:
        a = np.asarray(columns[name])
        if len(a) != rows:
            raise ValueError(f"column '{name}' has {len(a)} rows, expected {rows}")
        dtype = a.dtype.newbyteorder("<") if a.dtype.byteorder == ">" else a.dtype
        if dtype not in _DTYPES:
            raise TypeError(f"column '{name}': unsupported dtype {a.dtype}")
        raw = np.ascontiguousarray(a, dtype=dtype.newbyteorder("<")).tobytes()
        if len(name.encode()) > 8:
            raise ValueError(f"column name '{name}' is longer than 8 bytes")
        head.append(struct.pack("<8sB7x", name.encode("ascii"), _DTYPES[dtype]))
        body.append(raw + b"\0" * (-len(raw) % 8))
    return b"".join(head + body)


def decode_columns(buf: bytes) -> dict[str, np.ndarray]:
    """The inverse of `encode_columns` (tests; the front end does this in JS)."""
    if buf[:4] != MAGIC:
        raise ValueError("not a column buffer")
    version, n, rows, _ = struct.unpack_from("<HHII", buf, 4)
    if version != VERSION:
        raise ValueError(f"unsupported column buffer version {version}")
    out = {}
    pos = 16 + 16 * n
    for i in range(n):
        name, code = struct.unpack_from("<8sB7x", buf, 16 + 16 * i)
        dtype = _DTYPE_OF[code]
        out[name.rstrip(b"\0").decode("ascii")] = np.frombuffer(buf, dtype, rows, pos)
        pos += rows * dtype.itemsize + (-rows * dtype.itemsize % 8)
    return out


def css_to_u32(colors: list[str | None]) -> np.ndarray:
    """Per-point colors as RGBA u32 (0: the series color) for the buffer."""
    out = np.zeros(len(colors), dtype="<u4")
    for i, c in enumerate(colors):
        if c is None:
            continue
        out[i] = _css_rgba(c)
    return out


def _css_rgba(c: str) -> int:
    s = c.strip().lower()
    if s == "transparent":
        return 0
    if s.startswith("#"):
        h = s[1:]
        if len(h) in (3, 4):
            h = "".join(ch * 2 for ch in h)
        if len(h) == 6:
            h += "ff"
        return int(h, 16)
    if s.startswith(("rgb(", "rgba(")):
        parts = [p.strip() for p in s[s.index("(") + 1 : s.rindex(")")].split(",")]
        r, g, b = (int(float(p)) for p in parts[:3])
        a = round(float(parts[3]) * 255) if len(parts) > 3 else 255
        return (r << 24) | (g << 16) | (b << 8) | a
    raise ValueError(f"'{c}' is not a CSS color")


def camel(s: str) -> str:
    head, *rest = s.split("_")
    return head + "".join(p[:1].upper() + p[1:] for p in rest)


def camel_keys(d: Mapping[str, Any]) -> dict[str, Any]:
    return {camel(k): v for k, v in d.items()}


def series_options(options: Mapping[str, Any]) -> dict[str, Any]:
    return camel_keys(options)


def price_scale_options(options: Mapping[str, Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for k, v in options.items():
        if k == "mode" and isinstance(v, str):
            v = camel(v)
        elif k == "margins" and v is not None:
            v = {"top": float(v[0]), "bottom": float(v[1])}
        elif k == "range" and v is not None:
            v = {"low": float(v[0]), "high": float(v[1])}
        out[camel(k)] = v
    return out


def markers(ms: list[Mapping[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for m in ms:
        m = {camel(k): v for k, v in m.items()}
        for k in ("position", "shape"):
            if isinstance(m.get(k), str):
                m[k] = camel(m[k])
        # Icons are named by their Python id in the front end.
        if m.get("icon") is not None:
            m["icon"] = str(m["icon"])
        out.append(m)
    return out


def drawing_data(d: Mapping[str, Any]) -> dict[str, Any]:
    """A drawing's saved form in JS terms (style keys in camelCase; kinds,
    anchors and tool parameters are the same)."""
    out = {"kind": d["kind"], "anchors": [dict(a) for a in d["anchors"]]}
    if d.get("style") is not None:
        out["style"] = camel_keys(d["style"])
    if d.get("params") is not None:
        out["params"] = {k: list(v) if isinstance(v, (list, tuple)) else v for k, v in d["params"].items()}
    return out


def state(s: Mapping[str, Any]) -> dict[str, Any]:
    """A state from `Chart.save_state` in the JS `ChartState` form."""
    panes = []
    for p in s["panes"]:
        ps = p["price_scale"]
        rng = ps.get("range")
        panes.append({
            "stretch": p["stretch"],
            "priceScale": {
                "mode": camel(ps["mode"]),
                "inverted": ps["inverted"],
                "margins": {"top": ps["margins"][0], "bottom": ps["margins"][1]},
                "range": None if rng is None else {"low": rng[0], "high": rng[1]},
            },
        })
    return {
        "version": s["version"],
        "barSpacing": s["bar_spacing"],
        "rightOffset": s["right_offset"],
        "crosshairMode": s["crosshair_mode"],
        "panes": panes,
        "drawings": [
            {"pane": d["pane"], "visible": d["visible"], "locked": d["locked"],
             "data": drawing_data(d["data"])}
            for d in s["drawings"]
        ],
    }


def _js(v: Any) -> Any:
    if isinstance(v, Mapping):
        return {camel(k): _js(x) for k, x in v.items()}
    if isinstance(v, (list, tuple)):
        return [_js(x) for x in v]
    return v


def plot_options(options: Mapping[str, Any]) -> dict[str, Any]:
    """Mark, axis, rule or label options in JS terms: camelCase keys (also
    in formats and color scales), `icon` names in camelCase."""
    out = _js(options)
    if isinstance(out.get("icon"), str):
        out["icon"] = camel(out["icon"])
    return out


def params(p: Mapping[str, Any]) -> dict[str, Any]:
    return {camel(k): v for k, v in p.items()}


def placement(p: Any) -> Any:
    return camel(p) if isinstance(p, str) else p


def _target(op: Mapping[str, Any]) -> tuple[str, Any]:
    """The series or plot mark of a data operation."""
    return ("series", op["series"]) if "series" in op else ("mark", op.get("mark"))


class Recorder:
    """Operations that rebuild a chart in the front end, in order."""

    def __init__(self) -> None:
        #: (seq, op, buffer or None)
        self.ops: list[tuple[int, dict[str, Any], bytes | None]] = []
        self.seq = 0
        self.widget: Any = None

    def record(self, op: dict[str, Any], buffer: bytes | None = None) -> None:
        self.seq += 1
        self._compact(op)
        self.ops.append((self.seq, op, buffer))
        if self.widget is not None:
            self.widget._broadcast(self.seq, op, buffer)

    def _compact(self, op: dict[str, Any]) -> None:
        """Drops operations `op` makes useless: older data of a series or
        plot mark that gets new data, an update of the same bar (a plot
        mark's update replaces its last point: any update)."""
        kind, target = op["op"], _target(op)
        if kind == "setData":
            self.ops = [
                o for o in self.ops
                if not (o[1]["op"] in ("setData", "append", "update") and _target(o[1]) == target)
            ]
        elif kind == "update" and self.ops:
            last = self.ops[-1][1]
            if last["op"] == "update" and _target(last) == target and (
                "bar" not in op or last["bar"]["time"] == op["bar"]["time"]
            ):
                self.ops.pop()

    def snapshot(self) -> tuple[int, list[dict[str, Any]], list[bytes]]:
        """`(seq, ops, buffers)`; data ops refer to `buffers` by index."""
        ops, buffers = [], []
        for _, op, buf in self.ops:
            if buf is not None:
                op = {**op, "buffer": len(buffers)}
                buffers.append(buf)
            ops.append(op)
        return self.seq, ops, buffers


def _static(name: str) -> bytes:
    return resources.files("ferrochart").joinpath("static", name).read_bytes()


def esm() -> str:
    """The widget's ES module: the wasm-bindgen glue with the widget code
    appended (one module: anywidget loads it from a blob URL, so it cannot
    import siblings)."""
    glue = _static("ferrochart_wasm.js").decode()
    tail = "export { initSync, __wbg_init as default };"
    if tail not in glue:
        raise RuntimeError("unexpected wasm-bindgen glue (rebuild ferrochart/static)")
    glue = glue.replace(tail, "export { initSync };")
    return glue + "\n" + _static("widget.js").decode()


def has_frontend() -> bool:
    try:
        return all(
            resources.files("ferrochart").joinpath("static", n).is_file()
            for n in ("ferrochart_wasm.js", "ferrochart_wasm_bg.wasm", "widget.js")
        )
    except (ModuleNotFoundError, FileNotFoundError):
        return False


def widget_class() -> type:
    """`ChartWidget`, defined on first use (anywidget is optional)."""
    global _WIDGET
    if _WIDGET is not None:
        return _WIDGET
    import anywidget
    import traitlets

    class ChartWidget(anywidget.AnyWidget):  # type: ignore[misc]
        """Shows a `ferrochart.Chart` or `ferrochart.Plot` in Jupyter."""

        _esm = esm()
        #: 'chart' (FerroChart) or 'plot' (FerroPlot).
        kind = traitlets.Unicode("chart").tag(sync=True)
        width = traitlets.Float(1200).tag(sync=True)
        height = traitlets.Float(720).tag(sync=True)
        theme = traitlets.Unicode("light").tag(sync=True)
        #: Creation options (`attributionLogo`; a plot's `crosshair`, `hoverHighlight`).
        options = traitlets.Dict().tag(sync=True)
        #: Events with Python callbacks, forwarded by the front end.
        events = traitlets.List(traitlets.Unicode()).tag(sync=True)

        def __init__(self, chart: Any, kind: str = "chart", options: dict[str, Any] | None = None) -> None:
            w, h = chart.size
            super().__init__(kind=kind, width=w, height=h, theme=chart._theme, options=options or {})
            self._chart = chart
            self.on_msg(self._on_msg)

        def _on_msg(self, _widget: Any, content: Any, buffers: list[Any]) -> None:
            if not isinstance(content, dict):
                return
            if content.get("type") == "init":
                seq, ops, bufs = self._chart._rec.snapshot()
                reply: dict[str, Any] = {"type": "snapshot", "to": content.get("view"), "seq": seq, "ops": ops}
                if content.get("wasm"):
                    reply["wasm"] = len(bufs)
                    bufs = [*bufs, _static("ferrochart_wasm_bg.wasm")]
                self.send(reply, bufs)
            elif content.get("type") == "event":
                payload = content.get("payload", {})
                # marker_hover sends None when the pointer leaves a marker.
                self._chart._emit(content.get("name", ""), None if payload is None else _snake_payload(payload))

        def _broadcast(self, seq: int, op: dict[str, Any], buffer: bytes | None) -> None:
            if buffer is None:
                self.send({"type": "ops", "seq": seq, "ops": [op]})
            else:
                self.send({"type": "ops", "seq": seq, "ops": [{**op, "buffer": 0}]}, [buffer])

    _WIDGET = ChartWidget
    return ChartWidget


_WIDGET: type | None = None


def _snake(s: str) -> str:
    return "".join("_" + c.lower() if c.isupper() else c for c in s)


def _snake_payload(v: Any) -> Any:
    if isinstance(v, dict):
        return {_snake(k): _snake_payload(x) for k, x in v.items()}
    if isinstance(v, list):
        return [_snake_payload(x) for x in v]
    return v
