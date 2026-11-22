//! Derived market series (derivatives, on-chain, volatility) by bar time,
//! each tied to the symbol's price path so that the charts tell one story.

use serde::Serialize;

use super::bars::Interval;
use super::noise::{gauss, hash, smooth};
use super::symbols::Symbol;

#[derive(Serialize)]
pub struct MetricInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub unit: &'static str,
    /// Output columns besides `time`.
    pub columns: &'static [&'static str],
}

pub static METRICS: &[MetricInfo] = &[
    MetricInfo {
        name: "funding",
        description: "Perpetual funding rate per 8 hours",
        unit: "fraction",
        columns: &["value"],
    },
    MetricInfo {
        name: "openInterest",
        description: "Open interest in contracts",
        unit: "contracts",
        columns: &["value"],
    },
    MetricInfo {
        name: "longShortRatio",
        description: "Accounts long / accounts short",
        unit: "ratio",
        columns: &["value"],
    },
    MetricInfo {
        name: "liquidations",
        description: "Liquidated longs and shorts per bar (notional)",
        unit: "quote currency",
        columns: &["long", "short"],
    },
    MetricInfo {
        name: "cvd",
        description: "Cumulative volume delta (aggressive buys − sells)",
        unit: "base currency",
        columns: &["value"],
    },
    MetricInfo {
        name: "basis",
        description: "Annualised futures basis",
        unit: "fraction",
        columns: &["value"],
    },
    MetricInfo {
        name: "impliedVol",
        description: "At-the-money implied volatility, annualised",
        unit: "fraction",
        columns: &["value"],
    },
    MetricInfo {
        name: "realizedPrice",
        description: "Average cost basis of all holders",
        unit: "quote currency",
        columns: &["value"],
    },
    MetricInfo {
        name: "mvrv",
        description: "Market value / realised value",
        unit: "ratio",
        columns: &["value"],
    },
    MetricInfo {
        name: "exchangeNetflow",
        description: "Net coins moved onto exchanges",
        unit: "base currency",
        columns: &["value"],
    },
    MetricInfo {
        name: "activeAddresses",
        description: "Addresses active in the bar",
        unit: "count",
        columns: &["value"],
    },
];

pub fn find(name: &str) -> Option<&'static MetricInfo> {
    METRICS.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}

/// Columns of a metric: `time` and one vector per entry of `columns`.
pub struct Table {
    pub time: Vec<i64>,
    pub columns: Vec<(&'static str, Vec<f64>)>,
}

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

/// Rounds to 6 significant digits.
fn sig(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return 0.0;
    }
    let d = 5 - x.abs().log10().floor() as i32;
    let m = 10f64.powi(d);
    (x * m).round() / m
}

/// Log of the holders' average cost: an exponentially weighted average of
/// the log price over the past ~180 days (mean age ~60 days).
fn realized_log(path: &super::noise::Path, t: i64) -> f64 {
    let (mut sum, mut weights) = (0.0, 0.0);
    for i in 0..180 {
        let w = (-(i as f64) / 60.0).exp();
        sum += w * path.log_at(t - i * DAY, 12);
        weights += w;
    }
    sum / weights
}

/// Values of `metric` at the end of each bar starting at `starts`.
pub fn compute(
    sym: &Symbol,
    metric: &MetricInfo,
    interval: Interval,
    starts: &[i64],
    now: i64,
) -> Table {
    let path = sym.path();
    let seed = sym.seed() ^ super::noise::seed_of(metric.name);
    let lp = |t: i64| path.log_at(t, 0);
    let per_day = sym.trades_per_sec * sym.avg_size * 86_400.0;
    let mut cols: Vec<Vec<f64>> = metric
        .columns
        .iter()
        .map(|_| Vec::with_capacity(starts.len()))
        .collect();
    for &start in starts {
        let t = (start + interval.ms() - 1).min(now);
        let n = |lo, hi| smooth(seed, t, lo, hi);
        let values: [f64; 2] = match metric.name {
            "funding" => {
                let ret = lp(t) - lp(t - 8 * HOUR);
                [
                    (0.0001 + 0.03 * ret + 0.00008 * n(12, 17)).clamp(-0.0075, 0.0075),
                    0.0,
                ]
            }
            "openInterest" => {
                let excess = lp(t) - path.trend(t, 20);
                [per_day * 0.6 * (0.25 * n(16, 22) + 0.9 * excess).exp(), 0.0]
            }
            "longShortRatio" => [
                (0.08 + 0.22 * n(11, 17) - 2.5 * (lp(t) - lp(t - DAY))).exp(),
                0.0,
            ],
            "liquidations" => {
                let r = lp(t) - lp(start);
                let bar_vol = sym.daily_vol * (interval.ms() as f64 / DAY as f64).sqrt();
                let z = r / bar_vol.max(1e-12);
                let price = sym.price(lp(t));
                let scale = per_day * price * (interval.ms() as f64 / DAY as f64) * 0.02;
                let noise = |k| (0.8 * gauss(hash(seed, start as u64, k))).exp();
                [
                    scale * (-z).max(0.0).powi(2) * noise(1) + scale * 0.05 * noise(2),
                    scale * z.max(0.0).powi(2) * noise(3) + scale * 0.05 * noise(4),
                ]
            }
            "cvd" => [
                per_day * (0.8 * (lp(t) - path.trend(t, 17)) / sym.daily_vol + 0.6 * n(13, 20)),
                0.0,
            ],
            "basis" => [
                0.07 + 0.04 * n(15, 21) + 2.0 * (lp(t) - path.trend(t, 21)),
                0.0,
            ],
            "impliedVol" => [
                sym.daily_vol * 365f64.sqrt() * path.regime(t) * (1.08 + 0.08 * n(12, 16)),
                0.0,
            ],
            "realizedPrice" => [sym.price(realized_log(&path, t)), 0.0],
            "mvrv" => [(lp(t) - realized_log(&path, t)).exp(), 0.0],
            "exchangeNetflow" => {
                let frac = interval.ms() as f64 / DAY as f64;
                [
                    per_day
                        * frac
                        * (0.35 * gauss(hash(seed, start as u64, 9))
                            - 0.6 * (lp(t) - lp(start)) / sym.daily_vol),
                    0.0,
                ]
            }
            "activeAddresses" => {
                let frac = (interval.ms() as f64 / DAY as f64).min(1.0);
                [
                    (900_000.0
                        * frac.sqrt()
                        * (0.3 * n(16, 23) + 0.1 * gauss(hash(seed, start as u64, 7))).exp())
                    .round(),
                    0.0,
                ]
            }
            _ => [f64::NAN, f64::NAN],
        };
        for (col, v) in cols.iter_mut().zip(values) {
            col.push(sig(v));
        }
    }
    Table {
        time: starts.to_vec(),
        columns: metric.columns.iter().copied().zip(cols).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::symbols::find;

    #[test]
    fn every_metric_is_finite() {
        let Some(btc) = find("BTCUSD") else { panic!() };
        let starts: Vec<i64> = (0..50).map(|i| 1_763_683_200_000 + i * HOUR).collect();
        for m in METRICS {
            let t = compute(btc, m, Interval(HOUR), &starts, i64::MAX);
            assert_eq!(t.columns.len(), m.columns.len());
            for (_, c) in &t.columns {
                assert!(c.iter().all(|v| v.is_finite()), "{}", m.name);
            }
        }
    }
}
