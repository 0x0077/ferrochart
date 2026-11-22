"""Writes the reference data of the indicator tests (`docs/book/src/indicators.md`).

Reads crates/ferrochart-indicators/tests/fixtures/input.csv and writes
`<name>.csv` for every indicator in `indicators.INDICATORS`. (Gap bars,
`docs/book/src/indicators.md`, are tested in Rust: a series with gaps must give the
outputs of its valid bars, spread back, so no fixtures are needed for them.)

Columns are the indicator's outputs in order; values use Python's `repr`
(shortest round-trip form), `NaN` for missing values.

Usage: python3 -I tools/reference/generate.py
"""

import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import indicators  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates/ferrochart-indicators/tests/fixtures"


def read_input():
    raw = np.genfromtxt(FIXTURES / "input.csv", delimiter=",", names=True, dtype=None)
    d = {name: np.asarray(raw[name], dtype=np.float64) for name in raw.dtype.names}
    d["time"] = np.asarray(raw["time"], dtype=np.int64)
    return d


def fmt(v):
    return "NaN" if np.isnan(v) else repr(float(v))


def write(path, outputs):
    names = list(outputs)
    rows = [",".join(names)]
    for t in range(len(outputs[names[0]])):
        rows.append(",".join(fmt(outputs[n][t]) for n in names))
    path.write_text("\n".join(rows) + "\n")


def main():
    d = read_input()
    for name, (func, kw) in indicators.INDICATORS.items():
        write(FIXTURES / f"{name}.csv", func(d, **kw))
    print(f"wrote {len(indicators.INDICATORS)} files to {FIXTURES}")


if __name__ == "__main__":
    main()
