"""Tests of `Chart.show()`: real windows that close by themselves
(the private `close_after`). Skipped without a display or with
FERROCHART_SKIP_WINDOW_TESTS=1.

The blocking mode is used: the first `show` of a process fixes the mode, so
the non-blocking mode is tested in a subprocess (Linux and Windows)."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import threading
import time

import pytest

import ferrochart as fc

HOUR = 3_600_000

pytestmark = pytest.mark.skipif(
    bool(os.environ.get("FERROCHART_SKIP_WINDOW_TESTS"))
    or (sys.platform.startswith("linux") and not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))),
    reason="no display, or FERROCHART_SKIP_WINDOW_TESTS is set",
)


def candles(n: int = 300) -> tuple[fc.Chart, fc.Series]:
    c = fc.Chart(width=640, height=360)
    s = c.add_series("candlestick")
    s.set_data(fc.random_walk(3, n, 0, HOUR))
    return c, s


def test_show_returns_when_closed_and_can_be_called_again():
    c, _ = candles()
    for _ in range(2):
        t = time.perf_counter()
        c._show(True, "test", None, None, 0.4)
        assert 0.3 < time.perf_counter() - t < 10
        assert not c.shown
    # The window sized the chart.
    assert c.size == (640, 360)
    other, _ = candles()
    other._show(True, "other", 300, 200, 0.2)
    assert other.size == (300, 200)


def test_events_reach_callbacks_and_errors_do_not_close_the_window(capsys):
    c, _ = candles()
    seen: list[dict] = []
    errors: list[Exception] = []

    @c.on("visible_range_changed")
    def record(e):
        seen.append(e)
        try:
            c.show()  # already in a window
        except ValueError as err:
            errors.append(err)
        raise RuntimeError("boom")  # printed, the window stays

    c._show(True, "events", None, None, 0.5)
    assert seen and set(seen[0]) == {"from", "to"}
    assert errors and "already shown" in str(errors[0])
    assert "RuntimeError: boom" in capsys.readouterr().err
    c.off("visible_range_changed")
    with pytest.raises(ValueError, match="unknown event"):
        c.on("hover", print)


def test_other_threads_run_and_their_changes_are_drawn():
    c, s = candles(100)
    d = fc.random_walk(3, 200, 0, HOUR)
    done = threading.Event()

    def feed():
        for i in range(100, 200, 10):
            s.append({k: v[i : i + 10] for k, v in d.items()})
            time.sleep(0.02)
        done.set()

    threading.Thread(target=feed).start()
    c._show(True, "feed", None, None, 0.8)
    assert done.wait(5), "the feed thread did not run while show() blocked"
    assert c.data_range() == (0, 199)


def test_plots_show_and_draw_changes_from_other_threads():
    p = fc.Plot(width=500, height=300)
    m = p.add_mark("line", [1, 2, 3])
    done = threading.Event()

    def feed():
        for i in range(10):
            m.append(x=[3 + i], y=[10.0 * (i + 1)])
            time.sleep(0.02)
        done.set()

    threading.Thread(target=feed).start()
    p._show(True, "plot", None, None, 0.8)
    assert done.wait(5)
    assert not p.shown and p.size == (500, 300)
    # The window drew the new points: the view (as of the last frame) has them.
    assert p.axis("right").view[1] >= 100


def test_ctrl_c_leaves_show():
    c, _ = candles()
    threading.Timer(0.3, lambda: os.kill(os.getpid(), signal.SIGINT)).start()
    t = time.perf_counter()
    with pytest.raises(KeyboardInterrupt):
        c._show(True, "interrupt", None, None, 10)
    assert time.perf_counter() - t < 5
    assert not c.shown


def test_show_needs_the_main_thread():
    c, _ = candles(10)
    caught: list[BaseException] = []

    def run():
        try:
            c.show()
        except BaseException as e:  # noqa: BLE001
            caught.append(e)

    t = threading.Thread(target=run)
    t.start()
    t.join()
    assert caught and isinstance(caught[0], RuntimeError)


@pytest.mark.skipif(sys.platform != "darwin", reason="macOS behavior")
def test_non_blocking_is_refused_on_macos():
    c, _ = candles(10)
    with pytest.raises(ValueError, match="not available on macOS"):
        c.show(block=False)


@pytest.mark.skipif(sys.platform == "darwin", reason="Linux and Windows only")
def test_non_blocking_windows_in_a_subprocess():
    code = """
import time, ferrochart as fc
c = fc.Chart(width=400, height=300)
s = c.add_series("candlestick")
s.set_data(fc.random_walk(1, 100, 0, 3600000))
events = []
c.on("visible_range_changed", events.append)
c._show(False, "bg", None, None, 0.5)
t = time.time()
while c.shown or not events:
    fc.process_events()
    time.sleep(0.05)
    assert time.time() - t < 10
try:
    c.show()
except ValueError as e:
    assert "block=False" in str(e)
else:
    raise AssertionError("mixing modes was allowed")
print("ok")
"""
    out = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, timeout=60)
    assert out.returncode == 0 and "ok" in out.stdout, out.stderr
