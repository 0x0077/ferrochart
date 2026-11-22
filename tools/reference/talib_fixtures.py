"""Writes TA-Lib's outputs for the 21 indicators TA-Lib has
(`docs/book/src/indicators.md`).

Reads crates/ferrochart-indicators/tests/fixtures/input.csv and writes
`talib/<name>.csv` next to it, columns in the order of our outputs. The Rust
test compares them, skipping the bars `docs/book/src/indicators.md` names for each
indicator whose seed differs from TA-Lib's.

Needs TA-Lib (`brew install ta-lib`, `pip install numpy TA-Lib`); the tests
only read the CSV files.

Usage: python3 -I tools/reference/talib_fixtures.py
"""

import pathlib

import numpy as np
import talib

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates/ferrochart-indicators/tests/fixtures"


def read_input():
    raw = np.genfromtxt(FIXTURES / "input.csv", delimiter=",", names=True, dtype=None)
    return {n: np.asarray(raw[n], dtype=np.float64) for n in raw.dtype.names}


def fmt(v):
    return "NaN" if np.isnan(v) else repr(float(v))


def main():
    d = read_input()
    h, l, c, v = d["high"], d["low"], d["close"], d["volume"]
    macd, signal, hist = talib.MACD(c, 12, 26, 9)
    upper, mid, lower = talib.BBANDS(c, 20, 2.0, 2.0, 0)
    slowk, slowd = talib.STOCH(h, l, c, 14, 3, 0, 3, 0)
    fastk, fastd = talib.STOCHRSI(c, 14, 14, 3, 0)
    aroon_down, aroon_up = talib.AROON(h, l, 14)
    out = {
        "sma": {"sma": talib.SMA(c, 20)},
        "ema": {"ema": talib.EMA(c, 20)},
        "wma": {"wma": talib.WMA(c, 20)},
        "dema": {"dema": talib.DEMA(c, 20)},
        "tema": {"tema": talib.TEMA(c, 20)},
        "boll": {"upper": upper, "mid": mid, "lower": lower},
        "sar": {"sar": talib.SAR(h, l, 0.02, 0.2)},
        "macd": {"macd": macd, "signal": signal, "hist": hist},
        "rsi": {"rsi": talib.RSI(c, 14)},
        "stoch": {"k": slowk, "d": slowd},
        "stochrsi": {"k": fastk, "d": fastd},
        "cci": {"cci": talib.CCI(h, l, c, 20)},
        "willr": {"wr": talib.WILLR(h, l, c, 14)},
        "mfi": {"mfi": talib.MFI(h, l, c, v, 14)},
        "adx": {
            "adx": talib.ADX(h, l, c, 14),
            "plus_di": talib.PLUS_DI(h, l, c, 14),
            "minus_di": talib.MINUS_DI(h, l, c, 14),
        },
        "atr": {"atr": talib.ATR(h, l, c, 14)},
        "obv": {"obv": talib.OBV(c, v)},
        "roc": {"roc": talib.ROC(c, 10)},
        "momentum": {"mom": talib.MOM(c, 10)},
        "trix": {"trix": talib.TRIX(c, 15)},
        "aroon": {"up": aroon_up, "down": aroon_down},
    }
    dst = FIXTURES / "talib"
    dst.mkdir(exist_ok=True)
    for name, cols in out.items():
        names = list(cols)
        rows = [",".join(names)]
        rows += [",".join(fmt(cols[k][t]) for k in names) for t in range(len(c))]
        (dst / f"{name}.csv").write_text("\n".join(rows) + "\n")
    print(f"wrote {len(out)} files to {dst} (TA-Lib {talib.__version__})")


if __name__ == "__main__":
    main()
