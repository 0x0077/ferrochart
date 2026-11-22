//! OHLCV bars cut from the price path.

use serde::Serialize;

use super::noise::{min_octave, smooth};
use super::symbols::{DAY_MS, Symbol};

/// Samples per bar for high and low.
const SAMPLES: i64 = 16;

/// Bar interval, milliseconds; weekly bars start on Monday 00:00 UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval(pub i64);

impl Interval {
    pub const ALLOWED: &'static [&'static str] = &[
        "1s", "5s", "15s", "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "1d", "1w",
    ];

    pub fn parse(s: &str) -> Option<Self> {
        if !Self::ALLOWED.contains(&s) {
            return None;
        }
        let (n, unit) = s.split_at(s.len() - 1);
        let n: i64 = n.parse().ok()?;
        let unit_ms = match unit {
            "s" => 1_000,
            "m" => 60_000,
            "h" => 3_600_000,
            "d" => DAY_MS,
            "w" => 7 * DAY_MS,
            _ => return None,
        };
        Some(Self(n * unit_ms))
    }

    pub fn ms(self) -> i64 {
        self.0
    }

    /// Start of the bar containing `t`.
    pub fn floor(self, t: i64) -> i64 {
        // Weeks: 1970-01-05 was a Monday.
        let offset = if self.0 == 7 * DAY_MS { 4 * DAY_MS } else { 0 };
        (t - offset).div_euclid(self.0) * self.0 + offset
    }
}

/// Bars as columns (the shape `Series.setData` takes).
#[derive(Debug, Default, Serialize)]
pub struct Bars {
    pub time: Vec<i64>,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub volume: Vec<f64>,
}

impl Bars {
    pub fn len(&self) -> usize {
        self.time.len()
    }

    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }

    /// Keeps the last `n` bars.
    pub fn keep_last(&mut self, n: usize) {
        let cut = self.len().saturating_sub(n);
        for col in [
            &mut self.open,
            &mut self.high,
            &mut self.low,
            &mut self.close,
            &mut self.volume,
        ] {
            col.drain(..cut);
        }
        self.time.drain(..cut);
    }
}

/// One bar starting at `start`, using the path up to `now`; `None` when the
/// market is closed for the whole bar or it has not started.
pub fn bar(sym: &Symbol, interval: Interval, start: i64, now: i64) -> Option<[f64; 5]> {
    let path = sym.path();
    let len = interval.ms();
    let end = (start + len).min(now.saturating_add(1));
    if end <= start {
        return None;
    }
    let step = (len / SAMPLES).max(1);
    let fine = min_octave(step);
    let vseed = sym.seed() ^ 0x5157;
    let (mut first, mut last) = (None::<(i64, f64)>, None::<(i64, f64)>);
    let (mut hi, mut lo) = (f64::MIN, f64::MAX);
    let mut volume = 0.0;
    let mut t = start;
    while t < end {
        if sym.in_session(t) {
            // Exact path at the first sample (the open); interior samples
            // skip octaves finer than the step.
            let exact = first.is_none();
            let l = path.log_at(t, if exact { 0 } else { fine });
            if exact {
                first = Some((t, l));
            }
            last = Some((t, l));
            hi = hi.max(l);
            lo = lo.min(l);
            let regime = path.regime(t);
            let rate = sym.trades_per_sec
                * sym.avg_size
                * sym.activity(t)
                * regime
                * regime
                * (0.55 * smooth(vseed, t, 6, 16)).exp();
            volume += rate * (step.min(end - t)) as f64 / 1000.0;
        }
        t += step;
    }
    let (_, open) = first?;
    // Volume: open time weighted by intraday activity, scaled by the
    // volatility regime and a slower noise (both taken once per bar).
    let mid = start + len / 2;
    let regime = path.regime(mid);
    volume *= sym.trades_per_sec
        * sym.avg_size
        * regime
        * regime
        * (0.55 * smooth(vseed, mid, 6, 16)).exp();
    let (last_t, _) = last?;
    // The close is the exact path at the end of the bar (or now), within the
    // session: the next bar's open is the same path a moment later.
    let close_t = if end - 1 > last_t && sym.in_session(end - 1) {
        end - 1
    } else {
        last_t
    };
    let close = path.log_at(close_t, 0);
    hi = hi.max(open).max(close);
    lo = lo.min(open).min(close);
    let px = |l: f64| sym.round_price(sym.price(l));
    let (o, h, l, c) = (px(open), px(hi), px(lo), px(close));
    Some([
        o,
        h.max(o).max(c),
        l.min(o).min(c),
        c,
        sym.round_size(volume),
    ])
}

/// Whether the market is open at some point of the bar starting at `start`.
pub fn has_session(sym: &Symbol, interval: Interval, start: i64) -> bool {
    let step = (interval.ms() / SAMPLES).max(1);
    (0..SAMPLES).any(|i| sym.in_session(start + i * step))
}

/// Starts of the bars in `[from, to)` (none after `now`) when the market is open.
pub fn starts(sym: &Symbol, interval: Interval, from: i64, to: i64, now: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let mut start = interval.floor(from);
    if start < from {
        start += interval.ms();
    }
    while start < to.min(now.saturating_add(1)) {
        if has_session(sym, interval, start) {
            out.push(start);
        }
        start += interval.ms();
    }
    out
}

/// Bars with start in `[from, to)`, none after `now`.
pub fn bars(sym: &Symbol, interval: Interval, from: i64, to: i64, now: i64) -> Bars {
    let mut out = Bars::default();
    let mut start = interval.floor(from);
    if start < from {
        start += interval.ms();
    }
    let stop = to.min(now.saturating_add(1));
    while start < stop {
        if let Some([o, h, l, c, v]) = bar(sym, interval, start, now) {
            out.time.push(start);
            out.open.push(o);
            out.high.push(h);
            out.low.push(l);
            out.close.push(c);
            out.volume.push(v);
        }
        start += interval.ms();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::symbols::find;

    const T0: i64 = 1_763_683_200_000; // 2025-11-21 00:00 UTC, Friday

    #[test]
    fn intervals() {
        assert_eq!(Interval::parse("15m"), Some(Interval(900_000)));
        assert_eq!(Interval::parse("7m"), None);
        let w = Interval::parse("1w").map(|i| i.floor(T0));
        assert_eq!(w, Some(T0 - 4 * DAY_MS)); // Monday
    }

    #[test]
    fn bars_are_consistent() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let i = Interval(60_000);
        let b = bars(btc, i, T0, T0 + 3_600_000, i64::MAX);
        assert_eq!(b.len(), 60);
        for k in 0..b.len() {
            assert!(b.low[k] <= b.open[k].min(b.close[k]));
            assert!(b.high[k] >= b.open[k].max(b.close[k]));
            assert!(b.volume[k] > 0.0);
        }
        // Same bars from a sub-range.
        let c = bars(btc, i, T0 + 600_000, T0 + 1_200_000, i64::MAX);
        assert_eq!(c.close, b.close[10..20]);
        // An hour bar spans the minute bars.
        let h = bars(btc, Interval(3_600_000), T0, T0 + 1, i64::MAX);
        assert_eq!(h.open[0], b.open[0]);
        let max = b.high.iter().cloned().fold(f64::MIN, f64::max);
        assert!(
            (h.high[0] - max).abs() / max < 0.002,
            "{} vs {max}",
            h.high[0]
        );
    }

    #[test]
    fn no_bars_when_closed_or_in_the_future() {
        let Some(es) = find("ES") else { panic!() };
        let sat = T0 + DAY_MS;
        assert_eq!(
            bars(es, Interval(60_000), sat, sat + DAY_MS, i64::MAX).len(),
            0
        );
        // Friday: 390 one-minute bars in the cash session.
        assert_eq!(
            bars(es, Interval(60_000), T0, T0 + DAY_MS, i64::MAX).len(),
            390
        );
        let Some(btc) = find("BTCUSD") else { panic!() };
        let now = T0 + 90_000;
        let live = bars(btc, Interval(60_000), T0, T0 + DAY_MS, now);
        assert_eq!(live.len(), 2); // the second one is forming
    }
}
