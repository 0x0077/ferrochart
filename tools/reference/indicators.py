"""Reference implementation of the indicators in `docs/book/src/indicators.md`.

Only depends on numpy. Each function follows the formulas of
`docs/book/src/indicators.md` line by line and favors clarity over speed. Functions take
arrays of *valid* bars (no gap bars); `with_gaps` maps a function over data
that has gap bars (`docs/book/src/indicators.md`).

All window functions return float64 arrays of the input length, NaN during the
warmup.
"""

import math

import numpy as np

NAN = float("nan")


# ---- helpers (`docs/book/src/indicators.md`) ---------------------------------------


def first_valid(x):
    idx = np.flatnonzero(~np.isnan(x))
    return int(idx[0]) if idx.size else len(x)


def sma(x, n):
    out = np.full(len(x), NAN)
    f = first_valid(x)
    for t in range(f + n - 1, len(x)):
        out[t] = x[t - n + 1 : t + 1].sum() / n
    return out


def _recursive(x, n, alpha):
    out = np.full(len(x), NAN)
    f = first_valid(x)
    if f + n - 1 >= len(x):
        return out
    e = x[f : f + n].sum() / n
    out[f + n - 1] = e
    for t in range(f + n, len(x)):
        e = e + alpha * (x[t] - e)
        out[t] = e
    return out


def ema(x, n):
    return _recursive(x, n, 2.0 / (n + 1))


def rma(x, n):
    """Wilder's moving average; `(x - r) / n` as in the spec."""
    out = np.full(len(x), NAN)
    f = first_valid(x)
    if f + n - 1 >= len(x):
        return out
    r = x[f : f + n].sum() / n
    out[f + n - 1] = r
    for t in range(f + n, len(x)):
        r = r + (x[t] - r) / n
        out[t] = r
    return out


def wma(x, n):
    out = np.full(len(x), NAN)
    f = first_valid(x)
    w = np.arange(n, 0, -1, dtype=np.float64)  # weight n for x[t], 1 for x[t-n+1]
    denom = n * (n + 1) / 2
    for t in range(f + n - 1, len(x)):
        window = x[t - n + 1 : t + 1][::-1]
        out[t] = (w * window).sum() / denom
    return out


def rolling_max(x, n):
    out = np.full(len(x), NAN)
    for t in range(n - 1, len(x)):
        out[t] = x[t - n + 1 : t + 1].max()
    return out


def rolling_min(x, n):
    out = np.full(len(x), NAN)
    for t in range(n - 1, len(x)):
        out[t] = x[t - n + 1 : t + 1].min()
    return out


def rolling_sum(x, n):
    out = np.full(len(x), NAN)
    f = first_valid(x)
    for t in range(f + n - 1, len(x)):
        out[t] = x[t - n + 1 : t + 1].sum()
    return out


def source(d, name="close"):
    o, h, l, c = d["open"], d["high"], d["low"], d["close"]
    return {
        "close": c,
        "open": o,
        "high": h,
        "low": l,
        "hl2": (h + l) / 2,
        "hlc3": (h + l + c) / 3,
        "ohlc4": (o + h + l + c) / 4,
    }[name]


def true_range(d):
    h, l, c = d["high"], d["low"], d["close"]
    tr = np.full(len(c), NAN)
    for t in range(1, len(c)):
        tr[t] = max(h[t] - l[t], abs(h[t] - c[t - 1]), abs(l[t] - c[t - 1]))
    return tr


def safe_div(a, b, if_zero):
    return if_zero if b == 0 else a / b


# ---- 1. moving averages ------------------------------------------------------


def ind_sma(d, n=20):
    return {"sma": sma(source(d), n)}


def ind_ema(d, n=20):
    return {"ema": ema(source(d), n)}


def ind_wma(d, n=20):
    return {"wma": wma(source(d), n)}


def ind_hma(d, n=20):
    p = source(d)
    raw = 2 * wma(p, n // 2) - wma(p, n)
    return {"hma": wma(raw, int(math.isqrt(n)))}


def ind_vwma(d, n=20):
    p, v = source(d), d["volume"]
    out = np.full(len(p), NAN)
    for t in range(n - 1, len(p)):
        sv = v[t - n + 1 : t + 1].sum()
        out[t] = safe_div((p[t - n + 1 : t + 1] * v[t - n + 1 : t + 1]).sum(), sv, NAN)
    return {"vwma": out}


def ind_dema(d, n=20):
    e1 = ema(source(d), n)
    e2 = ema(e1, n)
    return {"dema": 2 * e1 - e2}


def ind_tema(d, n=20):
    e1 = ema(source(d), n)
    e2 = ema(e1, n)
    e3 = ema(e2, n)
    return {"tema": 3 * e1 - 3 * e2 + e3}


def ind_volume_ma(d, n=20):
    return {"vma": sma(d["volume"], n)}


# ---- 2. channels -------------------------------------------------------------


def ind_boll(d, n=20, k=2.0):
    p = source(d)
    mid = sma(p, n)
    sd = np.full(len(p), NAN)
    for t in range(n - 1, len(p)):
        w = p[t - n + 1 : t + 1]
        sd[t] = math.sqrt(((w - mid[t]) ** 2).sum() / n)
    return {"upper": mid + k * sd, "mid": mid, "lower": mid - k * sd}


def ind_atr(d, n=14):
    return {"atr": rma(true_range(d), n)}


def ind_keltner(d, n=20, m=2.0, atr_n=10):
    mid = ema(d["close"], n)
    a = rma(true_range(d), atr_n)
    return {"upper": mid + m * a, "mid": mid, "lower": mid - m * a}


def ind_donchian(d, n=20):
    up = rolling_max(d["high"], n)
    lo = rolling_min(d["low"], n)
    return {"upper": up, "mid": (up + lo) / 2, "lower": lo}


def ind_ichimoku(d, tenkan=9, kijun=26, senkou=52, shift=26):
    h, l, c = d["high"], d["low"], d["close"]

    def mid(n):
        return (rolling_max(h, n) + rolling_min(l, n)) / 2

    t9, k26, s52 = mid(tenkan), mid(kijun), mid(senkou)
    a = (t9 + k26) / 2
    span_a = np.full(len(c), NAN)
    span_b = np.full(len(c), NAN)
    span_a[shift:] = a[:-shift]
    span_b[shift:] = s52[:-shift]
    chikou = np.full(len(c), NAN)
    chikou[:-shift] = c[shift:]
    return {"tenkan": t9, "kijun": k26, "span_a": span_a, "span_b": span_b, "chikou": chikou}


def ind_sar(d, af_start=0.02, af_step=0.02, af_max=0.2):
    """Mirrors TA-Lib's TA_SAR step by step (`docs/book/src/indicators.md`, SAR)."""
    h, l = d["high"], d["low"]
    out = np.full(len(h), NAN)
    if len(h) < 2:
        return {"sar": out}
    up_move = h[1] - h[0]
    down_move = l[0] - l[1]
    is_long = not (down_move > 0 and down_move > up_move)
    af = af_start
    if is_long:
        ep, sar = h[1], l[0]
    else:
        ep, sar = l[1], h[0]
    new_low, new_high = l[1], h[1]
    for t in range(1, len(h)):
        prev_low, prev_high = new_low, new_high
        new_low, new_high = l[t], h[t]
        if is_long:
            if new_low <= sar:
                is_long = False
                sar = ep
                if sar < prev_high:
                    sar = prev_high
                if sar < new_high:
                    sar = new_high
                out[t] = sar
                af = af_start
                ep = new_low
                sar = sar + af * (ep - sar)
                if sar < prev_high:
                    sar = prev_high
                if sar < new_high:
                    sar = new_high
            else:
                out[t] = sar
                if new_high > ep:
                    ep = new_high
                    af = min(af + af_step, af_max)
                sar = sar + af * (ep - sar)
                if sar > prev_low:
                    sar = prev_low
                if sar > new_low:
                    sar = new_low
        else:
            if new_high >= sar:
                is_long = True
                sar = ep
                if sar > prev_low:
                    sar = prev_low
                if sar > new_low:
                    sar = new_low
                out[t] = sar
                af = af_start
                ep = new_high
                sar = sar + af * (ep - sar)
                if sar > prev_low:
                    sar = prev_low
                if sar > new_low:
                    sar = new_low
            else:
                out[t] = sar
                if new_low < ep:
                    ep = new_low
                    af = min(af + af_step, af_max)
                sar = sar + af * (ep - sar)
                if sar < prev_high:
                    sar = prev_high
                if sar < new_high:
                    sar = new_high
    return {"sar": out}


def ind_supertrend(d, atr_n=10, m=3.0):
    h, l, c = d["high"], d["low"], d["close"]
    a = rma(true_range(d), atr_n)
    hl2 = (h + l) / 2
    long = np.full(len(c), NAN)
    short = np.full(len(c), NAN)
    up = dn = NAN
    bull = True
    for t in range(atr_n, len(c)):
        basic_up = hl2[t] + m * a[t]
        basic_dn = hl2[t] - m * a[t]
        if t == atr_n:
            up, dn, bull = basic_up, basic_dn, True
        else:
            up = basic_up if (basic_up < up or c[t - 1] > up) else up
            dn = basic_dn if (basic_dn > dn or c[t - 1] < dn) else dn
            if bull and c[t] < dn:
                bull = False
            elif not bull and c[t] > up:
                bull = True
        if bull:
            long[t] = dn
        else:
            short[t] = up
    return {"long": long, "short": short}


MS_DAY = 86_400_000


def civil_month(days):
    """Months since 1970-01 of a day number (Howard Hinnant's algorithm)."""
    z = days + 719468
    era = (z if z >= 0 else z - 146096) // 146097
    doe = z - era * 146097
    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    y = yoe + era * 400
    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    mp = (5 * doy + 2) // 153
    month = mp + 3 if mp < 10 else mp - 9
    if month <= 2:
        y += 1
    return y * 12 + month - 1


def session_key(time, anchor, utc_offset_ms):
    days = (int(time) + utc_offset_ms) // MS_DAY
    if anchor == "day":
        return days
    if anchor == "week":
        return (days + 3) // 7  # 1970-01-01 was a Thursday; weeks start on Monday
    return civil_month(days)


def ind_vwap(d, anchor="day", utc_offset_ms=0, t0=None):
    tp = (d["high"] + d["low"] + d["close"]) / 3
    v, time = d["volume"], d["time"]
    out = np.full(len(tp), NAN)
    spv = sv = 0.0
    key = None
    for t in range(len(tp)):
        if anchor == "time":
            if time[t] < t0:
                continue
        else:
            k = session_key(time[t], anchor, utc_offset_ms)
            if k != key:
                key, spv, sv = k, 0.0, 0.0
        spv += tp[t] * v[t]
        sv += v[t]
        out[t] = safe_div(spv, sv, NAN)
    return {"vwap": out}


# ---- 3. oscillators ----------------------------------------------------------


def ind_macd(d, fast=12, slow=26, signal=9):
    p = source(d)
    m = ema(p, fast) - ema(p, slow)
    s = ema(m, signal)
    return {"macd": m, "signal": s, "hist": m - s}


def rsi(p, n):
    gain = np.full(len(p), NAN)
    loss = np.full(len(p), NAN)
    for t in range(1, len(p)):
        dlt = p[t] - p[t - 1]
        gain[t] = max(dlt, 0.0)
        loss[t] = max(-dlt, 0.0)
    ag, al = rma(gain, n), rma(loss, n)
    out = np.full(len(p), NAN)
    for t in range(len(p)):
        if not math.isnan(ag[t]):
            out[t] = safe_div(100 * ag[t], ag[t] + al[t], 50.0)
    return out


def ind_rsi(d, n=14):
    return {"rsi": rsi(source(d), n)}


def stoch_raw(x, hi, lo, n):
    """100·(x − min)/(max − min) over n values; 50 for a zero range."""
    out = np.full(len(x), NAN)
    f = max(first_valid(hi), first_valid(lo))
    for t in range(f + n - 1, len(x)):
        mx = hi[t - n + 1 : t + 1].max()
        mn = lo[t - n + 1 : t + 1].min()
        out[t] = safe_div(100 * (x[t] - mn), mx - mn, 50.0)
    return out


def ind_stoch(d, k=14, smooth_k=3, dd=3):
    raw = stoch_raw(d["close"], d["high"], d["low"], k)
    kk = sma(raw, smooth_k)
    return {"k": kk, "d": sma(kk, dd)}


def ind_stochrsi(d, rsi_n=14, stoch_n=14, smooth_k=3, dd=3):
    r = rsi(source(d), rsi_n)
    raw = stoch_raw(r, r, r, stoch_n)
    kk = sma(raw, smooth_k)
    return {"k": kk, "d": sma(kk, dd)}


def ind_cci(d, n=20):
    tp = (d["high"] + d["low"] + d["close"]) / 3
    ma = sma(tp, n)
    out = np.full(len(tp), NAN)
    for t in range(n - 1, len(tp)):
        md = np.abs(tp[t - n + 1 : t + 1] - ma[t]).sum() / n
        out[t] = safe_div(tp[t] - ma[t], 0.015 * md, 0.0)
    return {"cci": out}


def ind_willr(d, n=14):
    h, l, c = d["high"], d["low"], d["close"]
    out = np.full(len(c), NAN)
    for t in range(n - 1, len(c)):
        mx = h[t - n + 1 : t + 1].max()
        mn = l[t - n + 1 : t + 1].min()
        out[t] = safe_div(-100 * (mx - c[t]), mx - mn, -50.0)
    return {"wr": out}


def ind_mfi(d, n=14):
    tp = (d["high"] + d["low"] + d["close"]) / 3
    mf = tp * d["volume"]
    pos = np.full(len(tp), NAN)
    neg = np.full(len(tp), NAN)
    for t in range(1, len(tp)):
        pos[t] = mf[t] if tp[t] > tp[t - 1] else 0.0
        neg[t] = mf[t] if tp[t] < tp[t - 1] else 0.0
    out = np.full(len(tp), NAN)
    for t in range(n, len(tp)):
        sp = pos[t - n + 1 : t + 1].sum()
        sn = neg[t - n + 1 : t + 1].sum()
        out[t] = safe_div(100 * sp, sp + sn, 50.0)
    return {"mfi": out}


def wilder_sum(x, n):
    """S[n] = Σ_{1..n} x, then S = S − S/n + x (x starts at index 1)."""
    out = np.full(len(x), NAN)
    if n >= len(x):
        return out
    s = x[1 : n + 1].sum()
    out[n] = s
    for t in range(n + 1, len(x)):
        s = s - s / n + x[t]
        out[t] = s
    return out


def ind_adx(d, n=14):
    h, l = d["high"], d["low"]
    pdm = np.full(len(h), NAN)
    mdm = np.full(len(h), NAN)
    for t in range(1, len(h)):
        up = h[t] - h[t - 1]
        down = l[t - 1] - l[t]
        pdm[t] = up if (up > down and up > 0) else 0.0
        mdm[t] = down if (down > up and down > 0) else 0.0
    sp, sm, st = wilder_sum(pdm, n), wilder_sum(mdm, n), wilder_sum(true_range(d), n)
    pdi = np.full(len(h), NAN)
    mdi = np.full(len(h), NAN)
    dx = np.full(len(h), NAN)
    for t in range(n, len(h)):
        pdi[t] = safe_div(100 * sp[t], st[t], 0.0)
        mdi[t] = safe_div(100 * sm[t], st[t], 0.0)
        dx[t] = safe_div(100 * abs(pdi[t] - mdi[t]), pdi[t] + mdi[t], 0.0)
    return {"adx": rma(dx, n), "plus_di": pdi, "minus_di": mdi}


def ind_obv(d):
    c, v = d["close"], d["volume"]
    out = np.full(len(c), NAN)
    if len(c):
        out[0] = v[0]
    for t in range(1, len(c)):
        if c[t] > c[t - 1]:
            out[t] = out[t - 1] + v[t]
        elif c[t] < c[t - 1]:
            out[t] = out[t - 1] - v[t]
        else:
            out[t] = out[t - 1]
    return {"obv": out}


def ind_cmf(d, n=20):
    h, l, c, v = d["high"], d["low"], d["close"], d["volume"]
    mfv = np.array(
        [safe_div((c[t] - l[t]) - (h[t] - c[t]), h[t] - l[t], 0.0) * v[t] for t in range(len(c))]
    )
    out = np.full(len(c), NAN)
    for t in range(n - 1, len(c)):
        out[t] = safe_div(mfv[t - n + 1 : t + 1].sum(), v[t - n + 1 : t + 1].sum(), NAN)
    return {"cmf": out}


def ind_roc(d, n=10):
    p = source(d)
    out = np.full(len(p), NAN)
    for t in range(n, len(p)):
        out[t] = NAN if p[t - n] == 0 else 100 * (p[t] / p[t - n] - 1)
    return {"roc": out}


def ind_momentum(d, n=10):
    p = source(d)
    out = np.full(len(p), NAN)
    out[n:] = p[n:] - p[:-n]
    return {"mom": out}


def ind_trix(d, n=15):
    e3 = ema(ema(ema(source(d), n), n), n)
    out = np.full(len(e3), NAN)
    for t in range(1, len(e3)):
        if not math.isnan(e3[t - 1]):
            out[t] = NAN if e3[t - 1] == 0 else 100 * (e3[t] / e3[t - 1] - 1)
    return {"trix": out}


def ind_aroon(d, n=14):
    h, l = d["high"], d["low"]
    up = np.full(len(h), NAN)
    down = np.full(len(h), NAN)
    for t in range(n, len(h)):
        wh = h[t - n : t + 1]
        wl = l[t - n : t + 1]
        # Bars since the extreme; ties: the most recent bar.
        since_high = int(np.argmax(wh[::-1]))
        since_low = int(np.argmin(wl[::-1]))
        up[t] = 100 * (n - since_high) / n
        down[t] = 100 * (n - since_low) / n
    return {"up": up, "down": down}


# ---- registry ----------------------------------------------------------------

# name -> (function, keyword arguments); the defaults of `docs/book/src/indicators.md`.
INDICATORS = {
    "sma": (ind_sma, {}),
    "ema": (ind_ema, {}),
    "wma": (ind_wma, {}),
    "hma": (ind_hma, {}),
    "vwma": (ind_vwma, {}),
    "dema": (ind_dema, {}),
    "tema": (ind_tema, {}),
    "volume_ma": (ind_volume_ma, {}),
    "boll": (ind_boll, {}),
    "keltner": (ind_keltner, {}),
    "donchian": (ind_donchian, {}),
    "ichimoku": (ind_ichimoku, {}),
    "sar": (ind_sar, {}),
    "supertrend": (ind_supertrend, {}),
    "vwap": (ind_vwap, {}),
    "vwap_month": (ind_vwap, {"anchor": "month"}),
    "macd": (ind_macd, {}),
    "rsi": (ind_rsi, {}),
    "stoch": (ind_stoch, {}),
    "stochrsi": (ind_stochrsi, {}),
    "cci": (ind_cci, {}),
    "willr": (ind_willr, {}),
    "mfi": (ind_mfi, {}),
    "adx": (ind_adx, {}),
    "atr": (ind_atr, {}),
    "obv": (ind_obv, {}),
    "cmf": (ind_cmf, {}),
    "roc": (ind_roc, {}),
    "momentum": (ind_momentum, {}),
    "trix": (ind_trix, {}),
    "aroon": (ind_aroon, {}),
}


def with_gaps(func, d, gap, **kw):
    """Runs `func` on the valid bars of `d` (`gap[t]` marks gap bars) and
    spreads the outputs back, NaN at gaps (`docs/book/src/indicators.md`)."""
    keep = ~gap
    valid = {k: v[keep] for k, v in d.items()}
    res = func(valid, **kw)
    out = {}
    for name, col in res.items():
        full = np.full(len(gap), NAN)
        full[keep] = col
        out[name] = full
    return out
