//! Trades and order-book snapshots around the price path.

use serde::Serialize;

use super::noise::{gauss, hash, unit};
use super::symbols::Symbol;

/// Trades as columns (the shape `Series.appendTrades` takes).
#[derive(Debug, Default, Serialize)]
pub struct Trades {
    pub time: Vec<i64>,
    pub price: Vec<f64>,
    pub size: Vec<f64>,
    /// 1 buy (aggressor), -1 sell.
    pub side: Vec<i8>,
}

/// Expected number of trades in `[from, to)`, for request limits.
pub fn expected_trades(sym: &Symbol, from: i64, to: i64) -> f64 {
    (to - from).max(0) as f64 / 1000.0 * sym.trades_per_sec * sym.open_fraction() * 1.6
}

/// Trades with time in `[from, to)`. The same trades for any sub-range.
pub fn trades(sym: &Symbol, from: i64, to: i64) -> Trades {
    let path = sym.path();
    let seed = sym.seed() ^ 0x7EAD;
    let mut out = Trades::default();
    let first = from.div_euclid(1000);
    let last = (to - 1).div_euclid(1000);
    let mut times = Vec::new();
    for s in first..=last {
        let t0 = s * 1000;
        if !sym.in_session(t0) {
            continue;
        }
        let regime = path.regime(t0);
        let mean = sym.trades_per_sec * sym.activity(t0) * regime;
        // Poisson count by inversion.
        let u = unit(hash(seed, s as u64, 0));
        let (mut n, mut p) = (0u32, (-mean).exp());
        let mut cdf = p;
        while u > cdf && n < 200 {
            n += 1;
            p *= mean / f64::from(n);
            cdf += p;
        }
        times.clear();
        times.extend(
            (0..n).map(|j| t0 + (unit(hash(seed, s as u64, u64::from(j) + 1)) * 1000.0) as i64),
        );
        times.sort_unstable();
        for (j, &t) in times.iter().enumerate() {
            if t < from || t >= to {
                continue;
            }
            let h = hash(seed ^ 0x51DE, t as u64, j as u64);
            let mid = sym.price(path.log_at(t, 0));
            // Aggressors lean with the move around the trade.
            let drift = path.log_at(t + 400, 0) - path.log_at(t - 400, 0);
            let lean = (drift / (sym.daily_vol / 300.0) * 0.25).clamp(-0.4, 0.4);
            let buy = unit(h) < 0.5 + lean;
            let ticks = mid / sym.tick;
            let price = if buy { ticks.ceil() } else { ticks.floor() } * sym.tick;
            let block = if unit(hash(h, 1, 2)) < 0.012 {
                12.0 + 30.0 * unit(hash(h, 3, 4))
            } else {
                1.0
            };
            let size = sym.avg_size * (0.9 * gauss(hash(h, 5, 6)) - 0.405).exp() * block;
            let min = 10f64.powi(-(sym.size_decimals as i32));
            out.time.push(t);
            out.price.push(sym.round_price(price));
            out.size.push(sym.round_size(size).max(min));
            out.side.push(if buy { 1 } else { -1 });
        }
    }
    out
}

/// Order-book snapshots: for each, `levels` prices on each side of the mid
/// (ascending) and the resting size there.
#[derive(Debug, Default, Serialize)]
pub struct Depth {
    pub time: Vec<i64>,
    pub prices: Vec<Vec<f64>>,
    pub sizes: Vec<Vec<f64>>,
}

/// One snapshot at `t`; levels are `step` ticks apart.
pub fn snapshot(sym: &Symbol, t: i64, levels: usize, step: u32) -> (Vec<f64>, Vec<f64>) {
    let path = sym.path();
    let seed = sym.seed() ^ 0xB00C;
    let row = sym.tick * f64::from(step.max(1));
    let mid = sym.price(path.log_at(t, 0));
    let best_bid = (mid / row).floor() as i64;
    let mut prices = Vec::with_capacity(levels * 2);
    let mut sizes = Vec::with_capacity(levels * 2);
    let quick = (t.div_euclid(20_000)) as u64;
    let slow = (t.div_euclid(900_000)) as u64;
    let base = sym.avg_size * sym.trades_per_sec * 4.0;
    for k in (1 - levels as i64)..=(levels as i64) {
        let level = best_bid + k;
        let dist = if k <= 0 { (1 - k) as f64 } else { k as f64 };
        let l = level as u64;
        // Persistent walls sit at fixed prices for a while.
        let wall_h = hash(seed, l, slow);
        let wall = if unit(wall_h) < 0.035 {
            4.0 + 9.0 * unit(mix_pair(wall_h))
        } else {
            1.0
        };
        let size = base
            * (0.35 + 0.65 * dist.sqrt())
            * (0.6 * gauss(hash(seed ^ 1, l, quick))).exp()
            * wall;
        prices.push(sym.round_price(level as f64 * row));
        sizes.push(sym.round_size(size));
    }
    (prices, sizes)
}

fn mix_pair(h: u64) -> u64 {
    super::noise::mix(h ^ 0xD1CE)
}

pub fn depth(sym: &Symbol, from: i64, to: i64, every: i64, levels: usize, step: u32) -> Depth {
    let mut out = Depth::default();
    let mut t = from.div_euclid(every) * every;
    if t < from {
        t += every;
    }
    while t < to {
        if sym.in_session(t) {
            let (p, s) = snapshot(sym, t, levels, step);
            out.time.push(t);
            out.prices.push(p);
            out.sizes.push(s);
        }
        t += every;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::symbols::find;

    const T0: i64 = 1_763_683_200_000;

    #[test]
    fn trades_are_stable_and_ordered() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let a = trades(btc, T0, T0 + 60_000);
        let n = a.time.len() as f64;
        assert!(n > 60.0 * 9.0 * 0.3 && n < 60.0 * 9.0 * 4.0, "{n} trades");
        assert!(a.time.windows(2).all(|w| w[0] <= w[1]));
        let b = trades(btc, T0 + 10_000, T0 + 20_000);
        let i = a.time.iter().position(|&t| t >= T0 + 10_000).unwrap_or(0);
        assert_eq!(b.price[..], a.price[i..i + b.price.len()]);
        assert!(a.side.contains(&1) && a.side.contains(&-1));
    }

    #[test]
    fn snapshots_straddle_the_mid() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let (p, s) = snapshot(btc, T0, 30, 10);
        assert_eq!(p.len(), 60);
        assert!(p.windows(2).all(|w| w[0] < w[1]));
        assert!(s.iter().all(|&x| x > 0.0));
        let d = depth(btc, T0, T0 + 10_000, 1000, 30, 10);
        assert_eq!(d.time.len(), 10);
    }
}
