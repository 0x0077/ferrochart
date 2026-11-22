"""Turns DataFrames, dicts of arrays and datetimes into what the engine takes:
contiguous int64 millisecond times and float64 columns.

Naive datetimes are taken as UTC; time-zone aware ones are converted to UTC.
pandas and polars are optional: they are recognized by type, not imported.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any, Mapping

import numpy as np

OHLC = ("open", "high", "low", "close")


def _module(obj: Any) -> str:
    return type(obj).__module__.split(".")[0]


def is_dataframe(obj: Any) -> bool:
    return _module(obj) in ("pandas", "polars") and hasattr(obj, "columns")


def _columns(data: Any) -> dict[str, Any]:
    """Columns by lower-case name. A pandas DataFrame without a `time` column
    gives its index as `time` if it holds datetimes or is named `time`."""
    if is_dataframe(data):
        if _module(data) == "polars":
            cols = {str(c).lower(): data[c] for c in data.columns}
        else:
            cols = {str(c).lower(): data[c] for c in data.columns}
            index = data.index
            if "time" not in cols and (
                str(getattr(index, "name", "")).lower() == "time"
                or np.asarray(index).dtype.kind == "M"
                or getattr(index, "tz", None) is not None
            ):
                cols["time"] = index
        return cols
    if isinstance(data, Mapping):
        return {str(k).lower(): v for k, v in data.items()}
    raise TypeError(
        "data must be a pandas or polars DataFrame or a dict of arrays, "
        f"not {type(data).__name__}"
    )


def times(col: Any) -> np.ndarray:
    """A time column as contiguous int64 UTC milliseconds."""
    if _module(col) == "pandas":
        if hasattr(col, "dt") and getattr(col.dt, "tz", None) is not None:  # Series
            col = col.dt.tz_convert("UTC").dt.tz_localize(None)
        elif getattr(col, "tz", None) is not None:  # DatetimeIndex
            col = col.tz_convert("UTC").tz_localize(None)
        col = col.to_numpy()
    elif _module(col) == "polars":
        col = col.to_numpy()
    a = np.asarray(col)
    if a.dtype.kind == "O":
        a = np.array([time_ms(t) for t in a], dtype=np.int64)
    if a.dtype.kind == "M":
        return np.ascontiguousarray(a.astype("datetime64[ms]").astype(np.int64))
    if a.dtype.kind in "iu":
        return np.ascontiguousarray(a, dtype=np.int64)
    if a.dtype.kind == "f":
        if not np.all(np.isfinite(a)) or np.any(a != np.round(a)):
            raise ValueError("float times must be whole milliseconds")
        return np.ascontiguousarray(a, dtype=np.int64)
    raise TypeError(f"time column of dtype {a.dtype} is not datetime64 or integer milliseconds")


def floats(col: Any, name: str) -> np.ndarray:
    """A numeric column as contiguous float64 (missing values: NaN)."""
    if _module(col) == "pandas":
        return np.ascontiguousarray(col.to_numpy(dtype=np.float64, na_value=np.nan))
    if _module(col) == "polars":
        col = col.cast(float).to_numpy()
    try:
        return np.ascontiguousarray(col, dtype=np.float64)
    except (TypeError, ValueError) as e:
        raise TypeError(f"column '{name}' must be numeric") from e


def time_ms(t: Any) -> int:
    """One time as UTC milliseconds: int, datetime, date, numpy.datetime64 or
    pandas.Timestamp."""
    if isinstance(t, (bool, np.bool_)):
        raise TypeError("a time must be an integer (ms) or a datetime, not a bool")
    if isinstance(t, (int, np.integer)):
        return int(t)
    if isinstance(t, (float, np.floating)):
        if not float(t).is_integer():
            raise ValueError("float times must be whole milliseconds")
        return int(t)
    if isinstance(t, np.datetime64):
        return int(t.astype("datetime64[ms]").astype(np.int64))
    if isinstance(t, _dt.datetime):
        if t.tzinfo is None:
            t = t.replace(tzinfo=_dt.timezone.utc)
        delta = t - _dt.datetime(1970, 1, 1, tzinfo=_dt.timezone.utc)
        return delta // _dt.timedelta(milliseconds=1)
    if isinstance(t, _dt.date):
        return time_ms(_dt.datetime(t.year, t.month, t.day))
    raise TypeError(f"cannot use {type(t).__name__} as a time")


def is_time(v: Any) -> bool:
    return isinstance(v, (np.datetime64, _dt.date))


def ohlc_columns(data: Any) -> dict[str, np.ndarray]:
    """`time`, `open`, `high`, `low`, `close` and optional `volume`."""
    cols = _columns(data)
    missing = [k for k in ("time", *OHLC) if k not in cols]
    if missing:
        raise ValueError(
            f"missing column(s) {', '.join(missing)}: OHLC series take "
            "time, open, high, low, close and optional volume"
        )
    out = {"time": times(cols["time"])}
    for k in OHLC:
        out[k] = floats(cols[k], k)
    if cols.get("volume") is not None:
        out["volume"] = floats(cols["volume"], "volume")
    return out


def value_columns(data: Any) -> dict[str, Any]:
    """`time`, `value` and optional `color` (CSS strings, None for the
    series color)."""
    cols = _columns(data)
    missing = [k for k in ("time", "value") if k not in cols]
    if missing:
        raise ValueError(
            f"missing column(s) {', '.join(missing)}: line, area, baseline, "
            "histogram and points series take time, value and optional color"
        )
    out: dict[str, Any] = {"time": times(cols["time"]), "value": floats(cols["value"], "value")}
    color = cols.get("color")
    if color is not None:
        if _module(color) in ("pandas", "polars"):
            color = color.to_list()
        out["color"] = [None if c is None or c != c else str(c) for c in color]
    return out


#: Columns of `Mark.set_data`.
MARK_COLUMNS = ("x", "y", "y0", "value", "size", "color", "step", "label")
_NUMERIC = ("y", "y0", "value", "size")


def _xs(col: Any) -> np.ndarray:
    """An x column as float64: numbers, or datetimes as UTC milliseconds."""
    if _module(col) == "pandas" and col.dtype.kind == "M":  # also time-zone aware
        return times(col).astype(np.float64)
    a = col.to_numpy() if _module(col) in ("pandas", "polars") else np.asarray(col)
    first = a[0] if a.dtype.kind == "O" and len(a) else None
    if a.dtype.kind == "M" or is_time(first):
        return times(a).astype(np.float64)
    if a.dtype.kind in "US" or isinstance(first, str):
        raise TypeError(
            "x must be numbers or datetimes; for categories use "
            "plot.axis('x').set_categories(names) and x = 0, 1, 2, …"
        )
    return floats(a, "x")


def _strings(col: Any, name: str, optional: bool) -> list[Any]:
    if _module(col) in ("pandas", "polars"):
        col = col.to_list()
    out: list[Any] = []
    for v in col:
        if v is None or (isinstance(v, float) and v != v):
            if not optional:
                raise TypeError(f"column '{name}' must not contain None")
            out.append(None)
        else:
            out.append(str(v))
    return out


def mark_columns(data: Any, columns: Mapping[str, Any]) -> dict[str, Any]:
    """The columns of `Mark.set_data(data, **columns)`: `x` and `y` as
    contiguous float64 (datetimes in `x` as UTC ms), optional `y0`,
    `value`, `size` (float64), `color` (CSS strings or None), `step`,
    `label` (strings).

    `data` may be a DataFrame or dict with those column names (other
    columns are ignored; a pandas DataFrame without `x` gives its index), a
    pandas Series (`x` is its index), a list or 1-D array of y values (`x`
    is 0, 1, 2, …) or an (n, 2) array of x and y. `columns` add or replace
    columns."""
    unknown = [k for k in columns if k not in MARK_COLUMNS]
    if unknown:
        raise TypeError(
            f"unknown column(s) {', '.join(unknown)} (expected: {', '.join(MARK_COLUMNS)})"
        )
    cols: dict[str, Any] = {}
    if data is None:
        pass
    elif is_dataframe(data) or isinstance(data, Mapping):
        found = _columns(data)
        cols = {k: found[k] for k in MARK_COLUMNS if k in found}
        if "x" not in cols and _module(data) == "pandas" and is_dataframe(data):
            cols["x"] = data.index
    elif _module(data) == "pandas" and hasattr(data, "index"):  # Series
        cols = {"x": data.index, "y": data}
    elif _module(data) == "polars":
        cols = {"y": data}
    else:
        a = np.asarray(data)
        if a.ndim == 2 and a.shape[1] == 2:
            cols = {"x": a[:, 0], "y": a[:, 1]}
        elif a.ndim == 1:
            cols = {"y": a}
        else:
            raise TypeError(
                "data must be a DataFrame, a dict of columns, a pandas Series, "
                f"a list or 1-D array of y values or an (n, 2) array, not {type(data).__name__}"
            )
    cols.update({k: v for k, v in columns.items() if v is not None})
    if "y" not in cols:
        raise ValueError("missing column 'y'")
    out: dict[str, Any] = {}
    for k in _NUMERIC:
        if k in cols:
            out[k] = floats(cols[k], k)
    n = len(out["y"])
    out["x"] = _xs(cols["x"]) if "x" in cols else np.arange(n, dtype=np.float64)
    if "color" in cols:
        out["color"] = _strings(cols["color"], "color", True)
    if "step" in cols:
        out["step"] = _strings(cols["step"], "step", False)
    if "label" in cols:
        out["label"] = _strings(cols["label"], "label", True)
    return out


def value(v: Any) -> Any:
    """A number or a datetime (as UTC ms) for an x or y position."""
    return float(time_ms(v)) if is_time(v) else v
