//! Named synthetic series and events for the chart library.
//!
//! Each series is a row of a table: a generator per output column, tied to
//! a symbol's price path so that the panes under a chart move with its
//! price. The tables are JSON, one file per library area
//! (`site/server/series/*.json`), embedded at build time and parsed once;
//! a file that does not parse is logged and skipped (the test below fails
//! on it), so a bad row never breaks the build.
//!
//! Values are taken at the end of each bar (or now, for the forming one).
//! `ahead` bars past now continue the same formulas: the path is defined
//! for any time, so schedules and projections can reach into the future.
//!
//! JSON shape (fields of a generator default as in `Gen`):
//!
//! ```json
//! { "series": [ { "name": "tvl", "description": "...", "unit": "USD",
//!                 "shares": false,
//!                 "cols": [ { "name": "value", "level": { "base": 5e9, "vol": 0.2, "beta": 0.7 } } ] } ],
//!   "events": [ { "name": "unlocks", "description": "...",
//!                 "when": { "every": { "days": 30 } },
//!                 "labels": ["Unlock"], "value": { "amount": { "scale": 1e6 } } } ] }
//! ```

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::bars::{self, Interval};
use super::noise::{Path, gauss, hash, min_octave, seed_of, smooth};
use super::symbols::{DAY_MS, Symbol};

/// The embedded tables, by file name.
static FILES: &[(&str, &str)] = &[
    ("common", include_str!("../../../series/common.json")),
    ("crypto", include_str!("../../../series/crypto.json")),
    ("defi", include_str!("../../../series/defi.json")),
    (
        "microstructure",
        include_str!("../../../series/microstructure.json"),
    ),
    ("equities", include_str!("../../../series/equities.json")),
    ("options", include_str!("../../../series/options.json")),
    ("macro", include_str!("../../../series/macro.json")),
    (
        "commodities",
        include_str!("../../../series/commodities.json"),
    ),
    ("quant", include_str!("../../../series/quant.json")),
    ("finance", include_str!("../../../series/finance.json")),
];

/// How a column is generated. Amplitudes are in the column's unit unless
/// noted; `beta` ties a column to the symbol's price.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Gen {
    /// A positive level, `base · exp(vol·n + beta·Δlog price + growth·years)`
    /// with `n` a slow unit noise and both differences taken from now:
    /// `base` is roughly today's value. Defaults: base 1, vol 0.1.
    Level {
        #[serde(default = "one")]
        base: f64,
        #[serde(default = "tenth")]
        vol: f64,
        #[serde(default)]
        beta: f64,
        #[serde(default)]
        growth: f64,
    },
    /// Mean-reverting around `center`: `center + amp·n + beta·amp·z`, `z` the
    /// standardised return over the last 8 bars; clamped to `lo..=hi`.
    /// Defaults: center 0, amp 1, no bounds.
    Osc {
        #[serde(default)]
        center: f64,
        #[serde(default = "one")]
        amp: f64,
        #[serde(default)]
        beta: f64,
        lo: Option<f64>,
        hi: Option<f64>,
    },
    /// A signed amount per bar (net flows): `scale` per day, times the bar's
    /// share of a day; `beta` > 0 leans with the bar's move.
    Flow {
        #[serde(default = "one")]
        scale: f64,
        #[serde(default)]
        beta: f64,
    },
    /// A positive amount per bar (volumes, counts, fees): `scale` per day
    /// times the bar's share of a day, log-noise `vol` (default 0.3);
    /// `beta` > 0 grows with the size of the bar's move.
    Amount {
        #[serde(default = "one")]
        scale: f64,
        #[serde(default = "vol3")]
        vol: f64,
        #[serde(default)]
        beta: f64,
    },
    /// A probability in (0, 1): `logistic(center + amp·n + beta·z)`.
    Prob {
        #[serde(default)]
        center: f64,
        #[serde(default = "one")]
        amp: f64,
        #[serde(default)]
        beta: f64,
    },
    /// The symbol's price `lag` bars earlier, times `factor`, with relative
    /// noise `noise` (another venue, an oracle, a NAV). Default noise 0.002.
    Track {
        #[serde(default = "one")]
        factor: f64,
        #[serde(default = "noise2")]
        noise: f64,
        #[serde(default)]
        lag: f64,
    },
    /// A signed level that wanders (cumulative quantities that can go
    /// negative): `scale·n` plus `beta·scale` per 10 daily standard
    /// deviations of price change since now.
    Walk {
        #[serde(default = "one")]
        scale: f64,
        #[serde(default)]
        beta: f64,
    },
    /// A step level: `base` changed by `step` (relative) at random times,
    /// about `perYear` times a year (share counts, policy rates).
    Steps {
        #[serde(default = "one")]
        base: f64,
        #[serde(default = "step2")]
        step: f64,
        #[serde(default = "four", rename = "perYear")]
        per_year: f64,
    },
}

fn one() -> f64 {
    1.0
}
fn tenth() -> f64 {
    0.1
}
fn vol3() -> f64 {
    0.3
}
fn noise2() -> f64 {
    0.002
}
fn step2() -> f64 {
    0.02
}
fn four() -> f64 {
    4.0
}

/// One output column.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Col {
    pub name: String,
    #[serde(flatten)]
    pub generator: Gen,
}

/// A named series: columns besides `time`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub unit: String,
    pub cols: Vec<Col>,
    /// Columns are normalised to sum to 1 on each row (shares of a whole).
    #[serde(default)]
    pub shares: bool,
}

/// When events happen.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum When {
    /// At random, about `perDay` a day.
    Random {
        #[serde(rename = "perDay")]
        per_day: f64,
    },
    /// Every `days` days: the first bar at or after each date (phase taken
    /// from the name).
    Every { days: f64 },
    /// On bars that move more than `z` standard deviations.
    Move { z: f64 },
}

/// Named events: `time` (bar start), `label`, `value`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSpec {
    pub name: String,
    pub description: String,
    pub when: When,
    /// Labels, picked at random. With two labels: the first for values ≥ 0,
    /// the second for negative ones.
    #[serde(default)]
    pub labels: Vec<String>,
    pub value: Gen,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    series: Vec<Spec>,
    #[serde(default)]
    events: Vec<EventSpec>,
}

/// Parses the embedded tables; errors by file.
fn parse_all() -> (Vec<Spec>, Vec<EventSpec>, Vec<String>) {
    let (mut series, mut events, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    for (name, text) in FILES {
        match serde_json::from_str::<File>(text) {
            Ok(f) => {
                series.extend(f.series);
                events.extend(f.events);
            }
            Err(e) => errors.push(format!("series/{name}.json: {e}")),
        }
    }
    (series, events, errors)
}

fn tables() -> &'static (Vec<Spec>, Vec<EventSpec>) {
    static T: OnceLock<(Vec<Spec>, Vec<EventSpec>)> = OnceLock::new();
    T.get_or_init(|| {
        let (s, e, errors) = parse_all();
        for err in errors {
            tracing::error!("skipped {err}");
        }
        (s, e)
    })
}

pub fn all_series() -> impl Iterator<Item = &'static Spec> {
    tables().0.iter()
}

pub fn all_events() -> impl Iterator<Item = &'static EventSpec> {
    tables().1.iter()
}

pub fn find(name: &str) -> Option<&'static Spec> {
    all_series().find(|s| s.name.eq_ignore_ascii_case(name))
}

pub fn find_events(name: &str) -> Option<&'static EventSpec> {
    all_events().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// For `/api/series` and `/api/events`.
#[derive(Serialize)]
pub struct Info {
    pub name: &'static str,
    pub description: &'static str,
    pub unit: &'static str,
    pub columns: Vec<&'static str>,
}

pub fn series_info() -> Vec<Info> {
    all_series()
        .map(|s| Info {
            name: &s.name,
            description: &s.description,
            unit: &s.unit,
            columns: s.cols.iter().map(|c| c.name.as_str()).collect(),
        })
        .collect()
}

pub fn events_info() -> Vec<Info> {
    all_events()
        .map(|s| Info {
            name: &s.name,
            description: &s.description,
            unit: "",
            columns: vec!["label", "value"],
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cadence: fixed intervals and calendar months.

/// Bar spacing of a series: a fixed interval or calendar months.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    Fixed(Interval),
    /// 1 (`1mo`), 3 (`1q`) or 12 (`1y`) months, starting on the 1st, 00:00 UTC.
    Months(u32),
}

impl Cadence {
    pub const EXTRA: &'static [&'static str] = &["1mo", "1q", "1y"];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "1mo" => Some(Self::Months(1)),
            "1q" => Some(Self::Months(3)),
            "1y" => Some(Self::Months(12)),
            _ => Interval::parse(s).map(Self::Fixed),
        }
    }

    /// Typical length, ms.
    pub fn ms(self) -> i64 {
        match self {
            Self::Fixed(iv) => iv.ms(),
            Self::Months(m) => i64::from(m) * 30 * DAY_MS + i64::from(m) * DAY_MS / 2,
        }
    }

    /// Start of the bar after the one starting at `start`.
    fn next(self, start: i64) -> i64 {
        match self {
            Self::Fixed(iv) => start + iv.ms(),
            Self::Months(m) => {
                let (y, mo, _) = civil(start.div_euclid(DAY_MS));
                let total = y * 12 + i64::from(mo - 1) + i64::from(m);
                days_from_civil(total.div_euclid(12), (total.rem_euclid(12) + 1) as u32, 1) * DAY_MS
            }
        }
    }

    /// Start of the bar containing `t`.
    fn floor(self, t: i64) -> i64 {
        match self {
            Self::Fixed(iv) => iv.floor(t),
            Self::Months(m) => {
                let (y, mo, _) = civil(t.div_euclid(DAY_MS));
                let idx = y * 12 + i64::from(mo - 1);
                let idx = idx - idx.rem_euclid(i64::from(m));
                days_from_civil(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32, 1) * DAY_MS
            }
        }
    }
}

/// (year, month 1–12, day 1–31) of a day number (days since 1970-01-01).
fn civil(z: i64) -> (i64, u32, u32) {
    // H. Hinnant's civil_from_days.
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Bar starts: the last `count` up to now, then `ahead` more. Fixed
/// intervals skip closed markets (also ahead); months do not.
pub fn starts(sym: &Symbol, cadence: Cadence, count: usize, ahead: usize, now: i64) -> Vec<i64> {
    let mut past = Vec::with_capacity(count + ahead);
    match cadence {
        Cadence::Fixed(iv) => {
            let span =
                (count as f64 / sym.open_fraction().max(0.05) * 1.25) as i64 * iv.ms() + 4 * DAY_MS;
            past = bars::starts(sym, iv, now - span, now.saturating_add(1), now);
            past.drain(..past.len().saturating_sub(count));
        }
        Cadence::Months(_) => {
            let mut s = cadence.floor(now);
            for _ in 0..count {
                past.push(s);
                s = cadence.floor(s - 1);
            }
            past.reverse();
        }
    }
    let mut s = past
        .last()
        .map_or_else(|| cadence.floor(now), |&l| cadence.next(l));
    let mut added = 0;
    let mut guard = 0;
    while added < ahead && guard < ahead * 20 + 100 {
        guard += 1;
        let open = match cadence {
            Cadence::Fixed(iv) => bars::has_session(sym, iv, s),
            Cadence::Months(_) => true,
        };
        if open && s > now {
            past.push(s);
            added += 1;
        }
        s = cadence.next(s);
    }
    past
}

// ---------------------------------------------------------------------------
// Generation.

/// Rounds to 6 significant digits.
pub fn sig(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return if x.is_finite() { 0.0 } else { f64::NAN };
    }
    let d = 5 - x.abs().log10().floor() as i32;
    let m = 10f64.powi(d);
    (x * m).round() / m
}

/// What generators see of one bar.
struct Ctx<'a> {
    sym: &'a Symbol,
    path: &'a Path,
    /// Bar start and the time sampled (its end, or now).
    start: i64,
    t: i64,
    now: i64,
    bar_ms: i64,
    /// Lowest octave to evaluate the path with.
    fine: u32,
    /// Octave range of the slow noise (~8 bars and slower).
    lo: u32,
    hi: u32,
}

impl Ctx<'_> {
    fn lp(&self, t: i64) -> f64 {
        self.path.log_at(t, self.fine)
    }
    fn days(&self) -> f64 {
        self.bar_ms as f64 / DAY_MS as f64
    }
    /// Standardised return over the last `bars` bars.
    fn z(&self, bars: i64) -> f64 {
        let r = self.lp(self.t) - self.lp(self.t - bars * self.bar_ms);
        r / (self.sym.daily_vol * (bars as f64 * self.days()).sqrt()).max(1e-12)
    }
    fn noise(&self, seed: u64) -> f64 {
        smooth(seed, self.t, self.lo, self.hi)
    }
    fn shock(&self, seed: u64, k: u64) -> f64 {
        gauss(hash(seed, self.start as u64, k))
    }
}

fn eval(g: Gen, c: &Ctx, seed: u64) -> f64 {
    match g {
        Gen::Level {
            base,
            vol,
            beta,
            growth,
        } => {
            let years = (c.t - c.now) as f64 / (365.25 * DAY_MS as f64);
            base * (vol * c.noise(seed) + beta * (c.lp(c.t) - c.lp(c.now)) + growth * years).exp()
        }
        Gen::Osc {
            center,
            amp,
            beta,
            lo,
            hi,
        } => {
            let n = 0.85 * c.noise(seed) + 0.3 * c.shock(seed, 1);
            let v = center + amp * n + beta * amp * c.z(8);
            v.max(lo.unwrap_or(f64::MIN)).min(hi.unwrap_or(f64::MAX))
        }
        Gen::Flow { scale, beta } => {
            let n = 0.7 * c.shock(seed, 2) + 0.5 * c.noise(seed);
            scale * c.days() * (n + beta * c.z(1))
        }
        Gen::Amount { scale, vol, beta } => {
            let n = 0.6 * c.shock(seed, 3) + 0.7 * c.noise(seed);
            scale * c.days() * (vol * n + beta * c.z(1).abs()).exp()
        }
        Gen::Prob { center, amp, beta } => {
            let x = center + amp * c.noise(seed) + beta * c.z(8);
            1.0 / (1.0 + (-x).exp())
        }
        Gen::Track { factor, noise, lag } => {
            let t = c.t - (lag * c.bar_ms as f64) as i64;
            let n = 0.6 * c.shock(seed, 4) + 0.6 * c.noise(seed);
            c.sym.price(c.lp(t)) * factor * (noise * n).exp()
        }
        Gen::Walk { scale, beta } => {
            let slow = smooth(seed, c.t, c.lo + 2, c.hi + 3);
            let dz = (c.lp(c.t) - c.lp(c.now)) / c.sym.daily_vol.max(1e-12) / 10.0;
            scale * (slow + beta * dz)
        }
        Gen::Steps {
            base,
            step,
            per_year,
        } => {
            // Up and down steps in 10-day buckets, counted from a fixed
            // start ~6 years before now (the same for every bar).
            let bucket = 10 * DAY_MS;
            let p = (per_year * 10.0 / 365.25).min(1.0);
            let end = c.t.div_euclid(bucket);
            let first = c.now.div_euclid(bucket) - 220;
            let mut level = 0.0;
            for b in first..=end {
                let h = hash(seed, b as u64, 7);
                if super::noise::unit(h) < p {
                    level += if super::noise::unit(hash(h, 1, 1)) < 0.55 {
                        1.0
                    } else {
                        -1.0
                    };
                }
            }
            base * (1.0 + step).powf(level)
        }
    }
}

fn ctx<'a>(sym: &'a Symbol, path: &'a Path, cadence: Cadence, start: i64, now: i64) -> Ctx<'a> {
    let bar_ms = cadence.ms();
    let end = match cadence {
        Cadence::Fixed(_) => start + bar_ms - 1,
        Cadence::Months(_) => cadence.next(start) - 1,
    };
    let t = if start <= now { end.min(now) } else { end };
    let lo = min_octave(bar_ms * 8) + 1;
    Ctx {
        sym,
        path,
        start,
        t,
        now,
        bar_ms,
        fine: min_octave(bar_ms),
        lo,
        hi: (lo + 6).min(super::noise::OCTAVES),
    }
}

/// Columns of a series.
pub struct Table {
    pub time: Vec<i64>,
    pub columns: Vec<(String, Vec<f64>)>,
}

pub fn compute(sym: &Symbol, spec: &Spec, cadence: Cadence, starts: &[i64], now: i64) -> Table {
    let path = sym.path();
    let base = sym.seed() ^ seed_of(&spec.name);
    let mut cols: Vec<Vec<f64>> = spec
        .cols
        .iter()
        .map(|_| Vec::with_capacity(starts.len()))
        .collect();
    let mut row = vec![0.0; spec.cols.len()];
    for &start in starts {
        let c = ctx(sym, &path, cadence, start, now);
        for (i, col) in spec.cols.iter().enumerate() {
            row[i] = eval(col.generator, &c, base ^ seed_of(&col.name));
        }
        if spec.shares {
            let sum: f64 = row.iter().map(|v| v.max(0.0)).sum();
            for v in &mut row {
                *v = if sum > 0.0 { v.max(0.0) / sum } else { 0.0 };
            }
        }
        for (col, v) in cols.iter_mut().zip(&row) {
            col.push(sig(*v));
        }
    }
    Table {
        time: starts.to_vec(),
        columns: spec.cols.iter().map(|c| c.name.clone()).zip(cols).collect(),
    }
}

/// Events: bar starts, labels and values.
pub struct Events {
    pub time: Vec<i64>,
    pub label: Vec<String>,
    pub value: Vec<f64>,
}

pub fn events(
    sym: &Symbol,
    spec: &EventSpec,
    cadence: Cadence,
    starts: &[i64],
    now: i64,
) -> Events {
    let path = sym.path();
    let seed = sym.seed() ^ seed_of(&spec.name);
    let mut out = Events {
        time: Vec::new(),
        label: Vec::new(),
        value: Vec::new(),
    };
    let phase = (super::noise::unit(seed) * 1e9) as i64;
    for (i, &start) in starts.iter().enumerate() {
        let c = ctx(sym, &path, cadence, start, now);
        let hit = match spec.when {
            When::Random { per_day } => {
                let p = 1.0 - (-per_day * c.days()).exp();
                super::noise::unit(hash(seed, start as u64, 11)) < p
            }
            When::Every { days } => {
                let period = (days * DAY_MS as f64) as i64;
                let k = |t: i64| (t - phase).div_euclid(period.max(1));
                let prev = if i == 0 {
                    start - c.bar_ms
                } else {
                    starts[i - 1]
                };
                k(start) != k(prev)
            }
            When::Move { z } => {
                let r = c.lp(c.t) - c.lp(start);
                (r / (sym.daily_vol * c.days().sqrt()).max(1e-12)).abs() > z
            }
        };
        if !hit {
            continue;
        }
        let v = eval(spec.value, &c, seed ^ 0xE7E7);
        let label = match spec.labels.as_slice() {
            [] => "",
            [up, down] => {
                if v >= 0.0 {
                    up
                } else {
                    down
                }
            }
            l => &l[(hash(seed, start as u64, 12) % l.len() as u64) as usize],
        };
        out.time.push(start);
        out.label.push(label.to_owned());
        out.value.push(sig(v));
    }
    out
}

/// Quantile paths from the last close: `time` (the next `ahead` bars) and
/// one column per quantile (percent), lognormal with the symbol's volatility.
pub fn forecast(
    sym: &Symbol,
    iv: Interval,
    ahead: usize,
    quantiles: &[f64],
    now: i64,
) -> (Vec<i64>, Vec<(String, Vec<f64>)>) {
    let path = sym.path();
    let last = iv.floor(now);
    let p0 = path.log_at(now, 0);
    let fut = starts(sym, Cadence::Fixed(iv), 0, ahead, last);
    let mut time = vec![now];
    time.extend(fut.iter().copied().filter(|&t| t > now));
    time.truncate(ahead + 1);
    let cols = quantiles
        .iter()
        .map(|&q| {
            let z = inv_norm(q / 100.0);
            let vals = time
                .iter()
                .map(|&t| {
                    let days = (t - now).max(0) as f64 / DAY_MS as f64;
                    let sd = sym.daily_vol * days.sqrt();
                    sym.round_price(sym.price(p0 - 0.5 * sd * sd + z * sd))
                })
                .collect();
            (format!("p{}", trim(q)), vals)
        })
        .collect();
    (time, cols)
}

fn trim(q: f64) -> String {
    let s = format!("{q}");
    s.replace('.', "_")
}

/// Inverse standard normal CDF (Acklam's approximation).
pub fn inv_norm(p: f64) -> f64 {
    let p = p.clamp(1e-9, 1.0 - 1e-9);
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let tail = |q: f64| {
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    if p < 0.02425 {
        tail((-2.0 * p.ln()).sqrt())
    } else if p > 1.0 - 0.02425 {
        -tail((-2.0 * (1.0 - p).ln()).sqrt())
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::symbols;

    const NOW: i64 = 1_763_726_400_000;

    #[test]
    fn calendar_round_trips() {
        for z in [-1000, 0, 19_000, 20_413, 30_000] {
            let (y, m, d) = civil(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(civil(20_413), (2025, 11, 21));
        let q = Cadence::Months(3);
        assert_eq!(civil(q.floor(NOW) / DAY_MS), (2025, 10, 1));
        assert_eq!(civil(q.next(q.floor(NOW)) / DAY_MS), (2026, 1, 1));
    }

    #[test]
    fn tables_parse() {
        let (_, _, errors) = parse_all();
        assert!(errors.is_empty(), "{errors:#?}");
    }

    #[test]
    fn names_are_unique_and_every_series_is_finite() {
        let mut names: Vec<&str> = all_series().map(|s| s.name.as_str()).collect();
        names.extend(all_events().map(|s| s.name.as_str()));
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate series or event name");
        let Some(btc) = symbols::find("BTCUSD") else {
            panic!()
        };
        for cadence in [Cadence::Fixed(Interval(3_600_000)), Cadence::Months(3)] {
            let s = starts(btc, cadence, 40, 8, NOW);
            assert_eq!(s.len(), 48);
            assert!(s.windows(2).all(|w| w[0] < w[1]));
            for spec in all_series() {
                let t = compute(btc, spec, cadence, &s, NOW);
                for (c, v) in &t.columns {
                    assert!(v.iter().all(|x| x.is_finite()), "{}.{c}", spec.name);
                }
            }
            for spec in all_events() {
                let e = events(btc, spec, cadence, &s, NOW);
                assert!(e.value.iter().all(|x| x.is_finite()), "{}", spec.name);
            }
        }
    }

    #[test]
    fn forecast_quantiles_are_ordered() {
        let Some(btc) = symbols::find("BTCUSD") else {
            panic!()
        };
        let (t, cols) = forecast(btc, Interval(86_400_000), 30, &[5.0, 50.0, 95.0], NOW);
        assert_eq!(t.len(), 31);
        assert_eq!(cols[0].0, "p5");
        for i in 1..t.len() {
            assert!(cols[0].1[i] < cols[1].1[i] && cols[1].1[i] < cols[2].1[i]);
        }
        assert!((inv_norm(0.975) - 1.959_96).abs() < 1e-3);
    }
}
