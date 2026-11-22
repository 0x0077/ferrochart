"""FerroChart: fast financial charts (candlesticks, indicators, drawings)
and plots (lines, scatter, bars, waterfalls, heatmaps) rendered on the GPU,
with a CPU fallback.

>>> import ferrochart as fc
>>> chart = fc.Chart(theme="dark")
>>> candles = chart.add_series("candlestick")
>>> candles.set_data(fc.random_walk(seed=1, n=1000))
>>> chart.add_volume_series(candles)
>>> chart.add_indicator(candles, "boll", n=20, k=2)
>>> chart.to_png("chart.png")
>>> plot = fc.Plot()
>>> plot.add_mark("scatter", {"x": [1, 2, 3], "y": [2, 4, 3]})
>>> plot.to_png("plot.png")
"""

from __future__ import annotations

from typing import Any

import numpy as np

from . import _native
from ._chart import EVENTS, SERIES_KINDS, Chart, Drawings, Indicator, PriceLine, Series
from ._plot import MARK_KINDS, PLOT_EVENTS, Axis, Label, Mark, Plot, Rule

__version__: str = _native.__version__

__all__ = [
    "Axis",
    "Chart",
    "Drawings",
    "EVENTS",
    "Indicator",
    "Label",
    "MARK_KINDS",
    "Mark",
    "PLOT_EVENTS",
    "Plot",
    "PriceLine",
    "Rule",
    "SERIES_KINDS",
    "Series",
    "indicator_names",
    "process_events",
    "random_walk",
]


def random_walk(
    seed: int, n: int, start_ms: int = 0, interval_ms: int = 60_000
) -> dict[str, np.ndarray]:
    """Deterministic random-walk OHLCV bars (the same as the Rust and JS
    `random_walk`): a dict of arrays `time` (int64 ms), `open`, `high`,
    `low`, `close`, `volume`. For demos and tests."""
    return _native.random_walk(int(seed), int(n), int(start_ms), int(interval_ms))


def indicator_names() -> list[str]:
    """Names accepted by `Chart.add_indicator`, in the order of
    `docs/book/src/indicators.md`."""
    names: Any = _native.indicator_names()
    return list(names)


def process_events() -> int:
    """Runs the callbacks of events from windows opened with
    `show(block=False)` now (they otherwise run between bytecodes of the
    main thread); returns how many events were delivered."""
    n: int = _native.process_events()
    return n
