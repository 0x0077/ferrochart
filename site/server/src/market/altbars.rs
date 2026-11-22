//! Bars that are not cut by time: every N trades, a fixed volume or
//! notional, line break, and cumulative-delta candles. Times are those of
//! the trade (or bar) that completed each element, kept strictly
//! increasing, so the engine's index-based time axis shows them in order.

use super::bars::{self, Bars, Interval};
use super::flow;
use super::symbols::Symbol;

/// How bars are formed.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Every `n` trades.
    Tick(usize),
    /// Every `size` of volume.
    Volume(f64),
    /// Every `notional` of price × size.
    Dollar(f64),
}

fn push(out: &mut Bars, t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) {
    let t = out.time.last().map_or(t, |&last| t.max(last + 1));
    out.time.push(t);
    out.open.push(o);
    out.high.push(h);
    out.low.push(l);
    out.close.push(c);
    out.volume.push(v);
}

/// Trade-driven bars over `[from, to)`; the unfinished last one is kept.
pub fn from_trades(sym: &Symbol, kind: Kind, from: i64, to: i64) -> Bars {
    let tr = flow::trades(sym, from, to);
    let mut out = Bars::default();
    let mut cur: Option<(f64, f64, f64, f64, f64, i64)> = None; // o h l c v t
    let mut filled = 0.0;
    for i in 0..tr.time.len() {
        let (t, p, s) = (tr.time[i], tr.price[i], tr.size[i]);
        let (o, h, l, _, v, _) = cur.unwrap_or((p, p, p, p, 0.0, t));
        cur = Some((o, h.max(p), l.min(p), p, v + s, t));
        filled += match kind {
            Kind::Tick(_) => 1.0,
            Kind::Volume(_) => s,
            Kind::Dollar(_) => s * p,
        };
        let target = match kind {
            Kind::Tick(n) => n as f64,
            Kind::Volume(x) | Kind::Dollar(x) => x,
        };
        if filled >= target
            && let Some((o, h, l, c, v, t)) = cur.take()
        {
            push(&mut out, t, o, h, l, c, sym.round_size(v));
            filled = 0.0;
        }
    }
    if let Some((o, h, l, c, v, t)) = cur {
        push(&mut out, t, o, h, l, c, sym.round_size(v));
    }
    out
}

/// Line break (`lines`, usually 3) of the closes of time bars: a new white
/// line when the close exceeds the last line's top, black below its bottom;
/// a reversal must break the extreme of the last `lines` lines.
pub fn line_break(src: &Bars, lines: usize) -> Bars {
    let mut out = Bars::default();
    let lines = lines.max(1);
    let Some(&first) = src.close.first() else {
        return out;
    };
    // (open, close) of each line.
    let mut made: Vec<(f64, f64)> = Vec::new();
    let mut base = first;
    for i in 1..src.len() {
        let c = src.close[i];
        let (top, bottom, up) = match made.last() {
            Some(&(o, cl)) => (o.max(cl), o.min(cl), cl > o),
            None => (base, base, true),
        };
        let recent = &made[made.len().saturating_sub(lines)..];
        let hi = recent.iter().map(|&(o, c)| o.max(c)).fold(top, f64::max);
        let lo = recent.iter().map(|&(o, c)| o.min(c)).fold(bottom, f64::min);
        let line = if made.is_empty() {
            (c != base).then_some((base, c))
        } else if up {
            if c > top {
                Some((top, c))
            } else if c < lo {
                Some((bottom, c))
            } else {
                None
            }
        } else if c < bottom {
            Some((bottom, c))
        } else if c > hi {
            Some((top, c))
        } else {
            None
        };
        if let Some((o, cl)) = line {
            made.push((o, cl));
            push(
                &mut out,
                src.time[i],
                o,
                o.max(cl),
                o.min(cl),
                cl,
                src.volume[i],
            );
            base = cl;
        }
    }
    out
}

/// Candles of cumulative volume delta (aggressive buys − sells) per time
/// bar over `[from, to)`: open is the previous close.
pub fn delta(sym: &Symbol, iv: Interval, from: i64, to: i64, now: i64) -> Bars {
    let tr = flow::trades(sym, from, to);
    let mut out = Bars::default();
    let starts = bars::starts(sym, iv, from, to, now);
    let mut j = 0;
    let mut cvd = 0.0;
    for &s in &starts {
        let end = s + iv.ms();
        let (o, mut h, mut l) = (cvd, cvd, cvd);
        let mut v = 0.0;
        while j < tr.time.len() && tr.time[j] < end {
            if tr.time[j] >= s {
                cvd += tr.size[j] * f64::from(tr.side[j]);
                h = h.max(cvd);
                l = l.min(cvd);
                v += tr.size[j];
            }
            j += 1;
        }
        let r = |x: f64| sym.round_size(x);
        push(&mut out, s, r(o), r(h), r(l), r(cvd), r(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::symbols::find;

    const T0: i64 = 1_763_683_200_000;

    #[test]
    fn trade_bars_fill_their_target() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let b = from_trades(btc, Kind::Tick(100), T0, T0 + 600_000);
        assert!(b.len() > 10);
        assert!(b.time.windows(2).all(|w| w[0] < w[1]));
        for i in 0..b.len() {
            assert!(
                b.low[i] <= b.open[i].min(b.close[i]) && b.high[i] >= b.open[i].max(b.close[i])
            );
        }
    }

    #[test]
    fn line_break_lines_continue() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let src = bars::bars(btc, Interval(60_000), T0, T0 + 600 * 60_000, i64::MAX);
        let lb = line_break(&src, 3);
        assert!(lb.len() > 5);
        for i in 1..lb.len() {
            let (o, c) = (lb.open[i], lb.close[i]);
            let (po, pc) = (lb.open[i - 1], lb.close[i - 1]);
            assert!(
                o == po.max(pc) || o == po.min(pc),
                "line {i} starts off the last one"
            );
            assert_ne!(o, c);
        }
        let d = delta(btc, Interval(60_000), T0, T0 + 30 * 60_000, i64::MAX);
        assert_eq!(d.len(), 30);
        assert!((1..30).all(|i| d.open[i] == d.close[i - 1]));
    }
}
