//! The 30 indicator calculations of `docs/book/src/indicators.md`, on top of the kernels
//! in [`crate::kernel`].
//!
//! A [`Calc`] sees valid bars only (gap bars are skipped by the caller,
//! `docs/book/src/indicators.md`) and follows the kernels' contract: `peek` computes
//! the outputs of a tentative newest bar without changing the state, `push`
//! commits a bar. Display shifts (Ichimoku) are `OutputSpec::shift`, applied
//! by the chart when drawing.
//!
//! Small-state calculations (SAR, `SuperTrend`, OBV, VWAP) implement `peek` as
//! "copy the state, step the copy"; window calculations peek their kernels.

use crate::kernel::{Ring, RollingExtreme, RollingSum, Sma, Smooth, WilderSum, Wma};

/// One valid bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// NaN when the series has no volume.
    pub volume: f64,
}

/// Price an indicator reads (`docs/book/src/indicators.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// Close (default).
    #[default]
    Close,
    /// Open.
    Open,
    /// High.
    High,
    /// Low.
    Low,
    /// `(H + L) / 2`.
    Hl2,
    /// `(H + L + C) / 3`.
    Hlc3,
    /// `(O + H + L + C) / 4`.
    Ohlc4,
}

impl Source {
    pub(crate) fn of(self, b: &Bar) -> f64 {
        match self {
            Self::Close => b.close,
            Self::Open => b.open,
            Self::High => b.high,
            Self::Low => b.low,
            Self::Hl2 => f64::midpoint(b.high, b.low),
            Self::Hlc3 => hlc3(b),
            Self::Ohlc4 => (b.open + b.high + b.low + b.close) / 4.0,
        }
    }
}

fn hlc3(b: &Bar) -> f64 {
    (b.high + b.low + b.close) / 3.0
}

/// `a / b`, or `if_zero` when `b == 0`.
fn div(a: f64, b: f64, if_zero: f64) -> f64 {
    if b == 0.0 { if_zero } else { a / b }
}

/// True range against the previous close; NaN on the first bar.
fn true_range(b: &Bar, prev_close: Option<f64>) -> f64 {
    prev_close.map_or(f64::NAN, |pc| {
        (b.high - b.low)
            .max((b.high - pc).abs())
            .max((b.low - pc).abs())
    })
}

/// An incremental calculation over valid bars.
pub(crate) trait Calc: Send {
    /// Number of outputs.
    fn outputs(&self) -> usize;
    /// Outputs for `b` as the newest bar, without committing it.
    fn peek(&self, b: &Bar, out: &mut [f64]);
    /// Commits `b`.
    fn push(&mut self, b: &Bar);
}

// ---- 1. moving averages ------------------------------------------------------

pub(crate) struct SmaCalc {
    src: Source,
    k: Sma,
}

impl SmaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: Sma::new(n),
        }
    }
}

impl Calc for SmaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.k.peek(self.src.of(b));
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

pub(crate) struct EmaCalc {
    src: Source,
    k: Smooth,
}

impl EmaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: Smooth::ema(n),
        }
    }
}

impl Calc for EmaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.k.peek(self.src.of(b));
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

pub(crate) struct WmaCalc {
    src: Source,
    k: Wma,
}

impl WmaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: Wma::new(n),
        }
    }
}

impl Calc for WmaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.k.peek(self.src.of(b));
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

pub(crate) struct HmaCalc {
    src: Source,
    half: Wma,
    full: Wma,
    hull: Wma,
}

impl HmaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        let n = n.max(2);
        Self {
            src,
            half: Wma::new(n / 2),
            full: Wma::new(n),
            hull: Wma::new(n.isqrt()),
        }
    }

    fn raw(&self, p: f64) -> f64 {
        2.0 * self.half.peek(p) - self.full.peek(p)
    }
}

impl Calc for HmaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.hull.peek(self.raw(self.src.of(b)));
    }
    fn push(&mut self, b: &Bar) {
        let p = self.src.of(b);
        let raw = self.raw(p);
        self.half.push(p);
        self.full.push(p);
        self.hull.push(raw);
    }
}

pub(crate) struct VwmaCalc {
    src: Source,
    pv: RollingSum,
    v: RollingSum,
}

impl VwmaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            pv: RollingSum::new(n),
            v: RollingSum::new(n),
        }
    }
}

impl Calc for VwmaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let sv = self.v.peek(b.volume);
        out[0] = div(self.pv.peek(self.src.of(b) * b.volume), sv, f64::NAN);
    }
    fn push(&mut self, b: &Bar) {
        self.pv.push(self.src.of(b) * b.volume);
        self.v.push(b.volume);
    }
}

/// EMA, EMA of EMA, EMA of that (DEMA, TEMA, TRIX).
#[derive(Clone, Copy)]
struct EmaChain {
    e: [Smooth; 3],
}

impl EmaChain {
    fn new(n: usize) -> Self {
        Self {
            e: [Smooth::ema(n); 3],
        }
    }

    fn peek(&self, p: f64) -> [f64; 3] {
        let y1 = self.e[0].peek(p);
        let y2 = self.e[1].peek(y1);
        [y1, y2, self.e[2].peek(y2)]
    }

    fn push(&mut self, p: f64) {
        let [y1, y2, _] = self.peek(p);
        self.e[0].push(p);
        self.e[1].push(y1);
        self.e[2].push(y2);
    }
}

pub(crate) struct DemaCalc {
    src: Source,
    k: EmaChain,
}

impl DemaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: EmaChain::new(n),
        }
    }
}

impl Calc for DemaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let [y1, y2, _] = self.k.peek(self.src.of(b));
        out[0] = 2.0 * y1 - y2;
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

pub(crate) struct TemaCalc {
    src: Source,
    k: EmaChain,
}

impl TemaCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: EmaChain::new(n),
        }
    }
}

impl Calc for TemaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let [y1, y2, y3] = self.k.peek(self.src.of(b));
        out[0] = 3.0 * y1 - 3.0 * y2 + y3;
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

pub(crate) struct VolumeMaCalc(Sma);

impl VolumeMaCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self(Sma::new(n))
    }
}

impl Calc for VolumeMaCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.0.peek(b.volume);
    }
    fn push(&mut self, b: &Bar) {
        self.0.push(b.volume);
    }
}

// ---- 2. channels -------------------------------------------------------------

/// Outputs: upper, mid, lower.
pub(crate) struct BollCalc {
    src: Source,
    k: f64,
    mid: Sma,
}

impl BollCalc {
    pub(crate) fn new(n: usize, k: f64, src: Source) -> Self {
        Self {
            src,
            k,
            mid: Sma::new(n),
        }
    }
}

impl Calc for BollCalc {
    fn outputs(&self) -> usize {
        3
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let p = self.src.of(b);
        let mid = self.mid.peek(p);
        let w = self.mid.window();
        let n = (w.cap() + 1) as f64;
        // Directly over the window, oldest first (`docs/book/src/indicators.md`, BOLL).
        let mut ss = 0.0;
        for c in w.iter().chain(std::iter::once(p)) {
            ss += (c - mid) * (c - mid);
        }
        let sd = (ss / n).sqrt();
        out[0] = mid + self.k * sd;
        out[1] = mid;
        out[2] = mid - self.k * sd;
    }
    fn push(&mut self, b: &Bar) {
        self.mid.push(self.src.of(b));
    }
}

/// `RMA(TR, n)`.
#[derive(Clone, Copy)]
pub(crate) struct Atr {
    prev_close: Option<f64>,
    k: Smooth,
}

impl Atr {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            prev_close: None,
            k: Smooth::rma(n),
        }
    }

    pub(crate) fn peek(&self, b: &Bar) -> f64 {
        self.k.peek(true_range(b, self.prev_close))
    }

    pub(crate) fn push(&mut self, b: &Bar) {
        self.k.push(true_range(b, self.prev_close));
        self.prev_close = Some(b.close);
    }
}

pub(crate) struct AtrCalc(Atr);

impl AtrCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self(Atr::new(n))
    }
}

impl Calc for AtrCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.0.peek(b);
    }
    fn push(&mut self, b: &Bar) {
        self.0.push(b);
    }
}

/// Outputs: upper, mid, lower.
pub(crate) struct KeltnerCalc {
    m: f64,
    mid: Smooth,
    atr: Atr,
}

impl KeltnerCalc {
    pub(crate) fn new(n: usize, m: f64, atr_n: usize) -> Self {
        Self {
            m,
            mid: Smooth::ema(n),
            atr: Atr::new(atr_n),
        }
    }
}

impl Calc for KeltnerCalc {
    fn outputs(&self) -> usize {
        3
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let mid = self.mid.peek(b.close);
        let a = self.atr.peek(b);
        out[0] = mid + self.m * a;
        out[1] = mid;
        out[2] = mid - self.m * a;
    }
    fn push(&mut self, b: &Bar) {
        self.mid.push(b.close);
        self.atr.push(b);
    }
}

/// `(max(H, n) + min(L, n)) / 2` and its parts.
struct MidRange {
    hi: RollingExtreme,
    lo: RollingExtreme,
}

impl MidRange {
    fn new(n: usize) -> Self {
        Self {
            hi: RollingExtreme::max(n),
            lo: RollingExtreme::min(n),
        }
    }

    fn peek(&self, b: &Bar) -> (f64, f64) {
        (self.hi.peek(b.high), self.lo.peek(b.low))
    }

    fn mid(&self, b: &Bar) -> f64 {
        let (h, l) = self.peek(b);
        f64::midpoint(h, l)
    }

    fn push(&mut self, b: &Bar) {
        self.hi.push(b.high);
        self.lo.push(b.low);
    }
}

/// Outputs: upper, mid, lower.
pub(crate) struct DonchianCalc(MidRange);

impl DonchianCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self(MidRange::new(n))
    }
}

impl Calc for DonchianCalc {
    fn outputs(&self) -> usize {
        3
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let (up, lo) = self.0.peek(b);
        out[0] = up;
        out[1] = f64::midpoint(up, lo);
        out[2] = lo;
    }
    fn push(&mut self, b: &Bar) {
        self.0.push(b);
    }
}

/// Outputs: tenkan, kijun, `span_a` (+shift), `span_b` (+shift), chikou (−shift).
pub(crate) struct IchimokuCalc {
    tenkan: MidRange,
    kijun: MidRange,
    senkou: MidRange,
}

impl IchimokuCalc {
    /// The display shift is `OutputSpec::shift` (the chart's business).
    pub(crate) fn new(tenkan: usize, kijun: usize, senkou: usize) -> Self {
        Self {
            tenkan: MidRange::new(tenkan),
            kijun: MidRange::new(kijun),
            senkou: MidRange::new(senkou),
        }
    }
}

impl Calc for IchimokuCalc {
    fn outputs(&self) -> usize {
        5
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let t = self.tenkan.mid(b);
        let k = self.kijun.mid(b);
        out[0] = t;
        out[1] = k;
        out[2] = f64::midpoint(t, k);
        out[3] = self.senkou.mid(b);
        out[4] = b.close;
    }
    fn push(&mut self, b: &Bar) {
        self.tenkan.push(b);
        self.kijun.push(b);
        self.senkou.push(b);
    }
}

/// Parabolic SAR; mirrors TA-Lib's `TA_SAR` (`docs/book/src/indicators.md`, SAR).
#[derive(Clone, Copy)]
struct SarState {
    count: u8,
    /// First bar's high and low.
    first: (f64, f64),
    is_long: bool,
    af: f64,
    ep: f64,
    sar: f64,
    /// Previous bar's high and low.
    prev: (f64, f64),
}

pub(crate) struct SarCalc {
    af_start: f64,
    af_step: f64,
    af_max: f64,
    s: SarState,
}

impl SarCalc {
    pub(crate) fn new(af_start: f64, af_step: f64, af_max: f64) -> Self {
        Self {
            af_start,
            af_step,
            af_max,
            s: SarState {
                count: 0,
                first: (0.0, 0.0),
                is_long: true,
                af: af_start,
                ep: 0.0,
                sar: 0.0,
                prev: (0.0, 0.0),
            },
        }
    }

    /// Advances `s` by `b`; returns the output.
    fn step(&self, s: &mut SarState, b: &Bar) -> f64 {
        let (new_high, new_low) = (b.high, b.low);
        match s.count {
            0 => {
                s.first = (b.high, b.low);
                s.count = 1;
                return f64::NAN;
            }
            1 => {
                let (h0, l0) = s.first;
                let up_move = new_high - h0;
                let down_move = l0 - new_low;
                s.is_long = !(down_move > 0.0 && down_move > up_move);
                s.af = self.af_start;
                if s.is_long {
                    (s.ep, s.sar) = (new_high, l0);
                } else {
                    (s.ep, s.sar) = (new_low, h0);
                }
                // TA-Lib starts with the previous bar equal to this one.
                s.prev = (new_high, new_low);
                s.count = 2;
            }
            _ => {}
        }
        let (prev_high, prev_low) = s.prev;
        let out;
        if s.is_long {
            if new_low <= s.sar {
                s.is_long = false;
                s.sar = s.ep;
                if s.sar < prev_high {
                    s.sar = prev_high;
                }
                if s.sar < new_high {
                    s.sar = new_high;
                }
                out = s.sar;
                s.af = self.af_start;
                s.ep = new_low;
                s.sar = s.sar + s.af * (s.ep - s.sar);
                if s.sar < prev_high {
                    s.sar = prev_high;
                }
                if s.sar < new_high {
                    s.sar = new_high;
                }
            } else {
                out = s.sar;
                if new_high > s.ep {
                    s.ep = new_high;
                    s.af = (s.af + self.af_step).min(self.af_max);
                }
                s.sar = s.sar + s.af * (s.ep - s.sar);
                if s.sar > prev_low {
                    s.sar = prev_low;
                }
                if s.sar > new_low {
                    s.sar = new_low;
                }
            }
        } else if new_high >= s.sar {
            s.is_long = true;
            s.sar = s.ep;
            if s.sar > prev_low {
                s.sar = prev_low;
            }
            if s.sar > new_low {
                s.sar = new_low;
            }
            out = s.sar;
            s.af = self.af_start;
            s.ep = new_high;
            s.sar = s.sar + s.af * (s.ep - s.sar);
            if s.sar > prev_low {
                s.sar = prev_low;
            }
            if s.sar > new_low {
                s.sar = new_low;
            }
        } else {
            out = s.sar;
            if new_low < s.ep {
                s.ep = new_low;
                s.af = (s.af + self.af_step).min(self.af_max);
            }
            s.sar = s.sar + s.af * (s.ep - s.sar);
            if s.sar < prev_high {
                s.sar = prev_high;
            }
            if s.sar < new_high {
                s.sar = new_high;
            }
        }
        s.prev = (new_high, new_low);
        out
    }
}

impl Calc for SarCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let mut s = self.s;
        out[0] = self.step(&mut s, b);
    }
    fn push(&mut self, b: &Bar) {
        let mut s = self.s;
        self.step(&mut s, b);
        self.s = s;
    }
}

#[derive(Clone, Copy)]
struct SuperTrendState {
    atr: Atr,
    started: bool,
    up: f64,
    dn: f64,
    bull: bool,
    prev_close: f64,
}

/// Outputs: long (lower band in up trends), short (upper band in down trends).
pub(crate) struct SuperTrendCalc {
    m: f64,
    s: SuperTrendState,
}

impl SuperTrendCalc {
    pub(crate) fn new(atr_n: usize, m: f64) -> Self {
        Self {
            m,
            s: SuperTrendState {
                atr: Atr::new(atr_n),
                started: false,
                up: f64::NAN,
                dn: f64::NAN,
                bull: true,
                prev_close: f64::NAN,
            },
        }
    }

    fn step(&self, s: &mut SuperTrendState, b: &Bar, out: &mut [f64]) {
        let a = s.atr.peek(b);
        s.atr.push(b);
        out[0] = f64::NAN;
        out[1] = f64::NAN;
        if !a.is_nan() {
            let hl2 = f64::midpoint(b.high, b.low);
            let basic_up = hl2 + self.m * a;
            let basic_dn = hl2 - self.m * a;
            if s.started {
                if basic_up < s.up || s.prev_close > s.up {
                    s.up = basic_up;
                }
                if basic_dn > s.dn || s.prev_close < s.dn {
                    s.dn = basic_dn;
                }
                if s.bull && b.close < s.dn {
                    s.bull = false;
                } else if !s.bull && b.close > s.up {
                    s.bull = true;
                }
            } else {
                (s.up, s.dn, s.bull, s.started) = (basic_up, basic_dn, true, true);
            }
            if s.bull {
                out[0] = s.dn;
            } else {
                out[1] = s.up;
            }
        }
        s.prev_close = b.close;
    }
}

impl Calc for SuperTrendCalc {
    fn outputs(&self) -> usize {
        2
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let mut s = self.s;
        self.step(&mut s, b, out);
    }
    fn push(&mut self, b: &Bar) {
        let mut s = self.s;
        self.step(&mut s, b, &mut [0.0; 2]);
        self.s = s;
    }
}

/// Where VWAP restarts (`docs/book/src/indicators.md`, VWAP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VwapAnchor {
    /// Every day (default).
    Day,
    /// Every week, starting on Monday.
    Week,
    /// Every month.
    Month,
    /// Once, at the first bar at or after this time (ms).
    Time(i64),
}

const MS_DAY: i64 = 86_400_000;

/// Months since 1970-01 of a day number (Howard Hinnant's algorithm).
fn civil_month(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        y += 1;
    }
    y * 12 + month - 1
}

#[derive(Clone, Copy)]
struct VwapState {
    key: Option<i64>,
    spv: f64,
    sv: f64,
}

pub(crate) struct VwapCalc {
    anchor: VwapAnchor,
    utc_offset_ms: i64,
    s: VwapState,
}

impl VwapCalc {
    pub(crate) fn new(anchor: VwapAnchor, utc_offset_ms: i64) -> Self {
        Self {
            anchor,
            utc_offset_ms,
            s: VwapState {
                key: None,
                spv: 0.0,
                sv: 0.0,
            },
        }
    }

    fn step(&self, s: &mut VwapState, b: &Bar) -> f64 {
        let days = b.time.saturating_add(self.utc_offset_ms).div_euclid(MS_DAY);
        let key = match self.anchor {
            VwapAnchor::Time(t0) => {
                if b.time < t0 {
                    return f64::NAN;
                }
                0
            }
            VwapAnchor::Day => days,
            VwapAnchor::Week => (days + 3).div_euclid(7),
            VwapAnchor::Month => civil_month(days),
        };
        if s.key != Some(key) {
            *s = VwapState {
                key: Some(key),
                spv: 0.0,
                sv: 0.0,
            };
        }
        s.spv += hlc3(b) * b.volume;
        s.sv += b.volume;
        div(s.spv, s.sv, f64::NAN)
    }
}

impl Calc for VwapCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let mut s = self.s;
        out[0] = self.step(&mut s, b);
    }
    fn push(&mut self, b: &Bar) {
        let mut s = self.s;
        self.step(&mut s, b);
        self.s = s;
    }
}

// ---- 3. oscillators ----------------------------------------------------------

/// Outputs: macd, signal, hist.
pub(crate) struct MacdCalc {
    src: Source,
    fast: Smooth,
    slow: Smooth,
    signal: Smooth,
}

impl MacdCalc {
    pub(crate) fn new(fast: usize, slow: usize, signal: usize, src: Source) -> Self {
        Self {
            src,
            fast: Smooth::ema(fast),
            slow: Smooth::ema(slow),
            signal: Smooth::ema(signal),
        }
    }

    fn macd(&self, p: f64) -> f64 {
        self.fast.peek(p) - self.slow.peek(p)
    }
}

impl Calc for MacdCalc {
    fn outputs(&self) -> usize {
        3
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let m = self.macd(self.src.of(b));
        let s = self.signal.peek(m);
        out[0] = m;
        out[1] = s;
        out[2] = m - s;
    }
    fn push(&mut self, b: &Bar) {
        let p = self.src.of(b);
        let m = self.macd(p);
        self.fast.push(p);
        self.slow.push(p);
        self.signal.push(m);
    }
}

/// RSI of a value stream.
#[derive(Clone, Copy)]
struct Rsi {
    prev: Option<f64>,
    gain: Smooth,
    loss: Smooth,
}

impl Rsi {
    fn new(n: usize) -> Self {
        Self {
            prev: None,
            gain: Smooth::rma(n),
            loss: Smooth::rma(n),
        }
    }

    fn moves(&self, p: f64) -> (f64, f64) {
        self.prev.map_or((f64::NAN, f64::NAN), |q| {
            ((p - q).max(0.0), (q - p).max(0.0))
        })
    }

    fn peek(&self, p: f64) -> f64 {
        let (g, l) = self.moves(p);
        let (ag, al) = (self.gain.peek(g), self.loss.peek(l));
        if ag.is_nan() {
            return f64::NAN;
        }
        div(100.0 * ag, ag + al, 50.0)
    }

    fn push(&mut self, p: f64) {
        let (g, l) = self.moves(p);
        self.gain.push(g);
        self.loss.push(l);
        self.prev = Some(p);
    }
}

pub(crate) struct RsiCalc {
    src: Source,
    k: Rsi,
}

impl RsiCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: Rsi::new(n),
        }
    }
}

impl Calc for RsiCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.k.peek(self.src.of(b));
    }
    fn push(&mut self, b: &Bar) {
        self.k.push(self.src.of(b));
    }
}

/// `100·(x − min)/(max − min)`, 50 for a zero range.
fn stoch_raw(x: f64, max: f64, min: f64) -> f64 {
    if max.is_nan() || min.is_nan() {
        return f64::NAN;
    }
    div(100.0 * (x - min), max - min, 50.0)
}

/// `%K = SMA(raw, smooth_k)`, `%D = SMA(%K, d)`.
struct KD {
    k: Sma,
    d: Sma,
}

impl KD {
    fn new(smooth_k: usize, d: usize) -> Self {
        Self {
            k: Sma::new(smooth_k),
            d: Sma::new(d),
        }
    }

    fn peek(&self, raw: f64, out: &mut [f64]) {
        let k = self.k.peek(raw);
        out[0] = k;
        out[1] = self.d.peek(k);
    }

    fn push(&mut self, raw: f64) {
        let k = self.k.peek(raw);
        self.k.push(raw);
        self.d.push(k);
    }
}

/// Outputs: k, d.
pub(crate) struct StochCalc {
    range: MidRange,
    kd: KD,
}

impl StochCalc {
    pub(crate) fn new(k: usize, smooth_k: usize, d: usize) -> Self {
        Self {
            range: MidRange::new(k),
            kd: KD::new(smooth_k, d),
        }
    }

    fn raw(&self, b: &Bar) -> f64 {
        let (h, l) = self.range.peek(b);
        stoch_raw(b.close, h, l)
    }
}

impl Calc for StochCalc {
    fn outputs(&self) -> usize {
        2
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        self.kd.peek(self.raw(b), out);
    }
    fn push(&mut self, b: &Bar) {
        let raw = self.raw(b);
        self.range.push(b);
        self.kd.push(raw);
    }
}

/// Outputs: k, d.
pub(crate) struct StochRsiCalc {
    src: Source,
    rsi: Rsi,
    hi: RollingExtreme,
    lo: RollingExtreme,
    kd: KD,
}

impl StochRsiCalc {
    pub(crate) fn new(
        rsi_n: usize,
        stoch_n: usize,
        smooth_k: usize,
        d: usize,
        src: Source,
    ) -> Self {
        Self {
            src,
            rsi: Rsi::new(rsi_n),
            hi: RollingExtreme::max(stoch_n),
            lo: RollingExtreme::min(stoch_n),
            kd: KD::new(smooth_k, d),
        }
    }

    fn raw(&self, r: f64) -> f64 {
        stoch_raw(r, self.hi.peek(r), self.lo.peek(r))
    }
}

impl Calc for StochRsiCalc {
    fn outputs(&self) -> usize {
        2
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let r = self.rsi.peek(self.src.of(b));
        self.kd.peek(self.raw(r), out);
    }
    fn push(&mut self, b: &Bar) {
        let p = self.src.of(b);
        let r = self.rsi.peek(p);
        let raw = self.raw(r);
        self.rsi.push(p);
        self.hi.push(r);
        self.lo.push(r);
        self.kd.push(raw);
    }
}

pub(crate) struct CciCalc(Sma);

impl CciCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self(Sma::new(n))
    }
}

impl Calc for CciCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let tp = hlc3(b);
        let ma = self.0.peek(tp);
        if ma.is_nan() {
            out[0] = f64::NAN;
            return;
        }
        let w = self.0.window();
        let mut sum = 0.0;
        for c in w.iter().chain(std::iter::once(tp)) {
            sum += (c - ma).abs();
        }
        let md = sum / (w.cap() + 1) as f64;
        out[0] = div(tp - ma, 0.015 * md, 0.0);
    }
    fn push(&mut self, b: &Bar) {
        self.0.push(hlc3(b));
    }
}

pub(crate) struct WillrCalc(MidRange);

impl WillrCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self(MidRange::new(n))
    }
}

impl Calc for WillrCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let (h, l) = self.0.peek(b);
        out[0] = if h.is_nan() {
            f64::NAN
        } else {
            div(-100.0 * (h - b.close), h - l, -50.0)
        };
    }
    fn push(&mut self, b: &Bar) {
        self.0.push(b);
    }
}

pub(crate) struct MfiCalc {
    prev_tp: Option<f64>,
    pos: RollingSum,
    neg: RollingSum,
}

impl MfiCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            prev_tp: None,
            pos: RollingSum::new(n),
            neg: RollingSum::new(n),
        }
    }

    fn flows(&self, b: &Bar) -> (f64, f64) {
        let tp = hlc3(b);
        let mf = tp * b.volume;
        self.prev_tp.map_or((f64::NAN, f64::NAN), |q| {
            (if tp > q { mf } else { 0.0 }, if tp < q { mf } else { 0.0 })
        })
    }
}

impl Calc for MfiCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let (p, n) = self.flows(b);
        let (sp, sn) = (self.pos.peek(p), self.neg.peek(n));
        out[0] = if sp.is_nan() {
            f64::NAN
        } else {
            div(100.0 * sp, sp + sn, 50.0)
        };
    }
    fn push(&mut self, b: &Bar) {
        let (p, n) = self.flows(b);
        self.pos.push(p);
        self.neg.push(n);
        self.prev_tp = Some(hlc3(b));
    }
}

/// Outputs: adx, `plus_di`, `minus_di`.
pub(crate) struct AdxCalc {
    prev: Option<Bar>,
    pdm: WilderSum,
    mdm: WilderSum,
    tr: WilderSum,
    adx: Smooth,
}

impl AdxCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            prev: None,
            pdm: WilderSum::new(n),
            mdm: WilderSum::new(n),
            tr: WilderSum::new(n),
            adx: Smooth::rma(n),
        }
    }

    /// (+DM, −DM, TR), NaN on the first bar.
    fn moves(&self, b: &Bar) -> (f64, f64, f64) {
        let Some(p) = self.prev else {
            return (f64::NAN, f64::NAN, f64::NAN);
        };
        let up = b.high - p.high;
        let down = p.low - b.low;
        (
            if up > down && up > 0.0 { up } else { 0.0 },
            if down > up && down > 0.0 { down } else { 0.0 },
            true_range(b, Some(p.close)),
        )
    }

    /// (+DI, −DI, DX).
    fn di(&self, b: &Bar) -> (f64, f64, f64) {
        let (p, m, t) = self.moves(b);
        let (sp, sm, st) = (self.pdm.peek(p), self.mdm.peek(m), self.tr.peek(t));
        if st.is_nan() {
            return (f64::NAN, f64::NAN, f64::NAN);
        }
        let pdi = div(100.0 * sp, st, 0.0);
        let mdi = div(100.0 * sm, st, 0.0);
        (pdi, mdi, div(100.0 * (pdi - mdi).abs(), pdi + mdi, 0.0))
    }
}

impl Calc for AdxCalc {
    fn outputs(&self) -> usize {
        3
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let (pdi, mdi, dx) = self.di(b);
        out[0] = self.adx.peek(dx);
        out[1] = pdi;
        out[2] = mdi;
    }
    fn push(&mut self, b: &Bar) {
        let (_, _, dx) = self.di(b);
        let (p, m, t) = self.moves(b);
        self.pdm.push(p);
        self.mdm.push(m);
        self.tr.push(t);
        self.adx.push(dx);
        self.prev = Some(*b);
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ObvCalc {
    prev: Option<(f64, f64)>,
}

impl ObvCalc {
    fn step(&self, b: &Bar) -> f64 {
        match self.prev {
            None => b.volume,
            Some((c, obv)) if b.close > c => obv + b.volume,
            Some((c, obv)) if b.close < c => obv - b.volume,
            Some((_, obv)) => obv,
        }
    }
}

impl Calc for ObvCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        out[0] = self.step(b);
    }
    fn push(&mut self, b: &Bar) {
        self.prev = Some((b.close, self.step(b)));
    }
}

pub(crate) struct CmfCalc {
    mfv: RollingSum,
    v: RollingSum,
}

impl CmfCalc {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            mfv: RollingSum::new(n),
            v: RollingSum::new(n),
        }
    }
}

fn money_flow_volume(b: &Bar) -> f64 {
    div((b.close - b.low) - (b.high - b.close), b.high - b.low, 0.0) * b.volume
}

impl Calc for CmfCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let sv = self.v.peek(b.volume);
        out[0] = div(self.mfv.peek(money_flow_volume(b)), sv, f64::NAN);
    }
    fn push(&mut self, b: &Bar) {
        self.mfv.push(money_flow_volume(b));
        self.v.push(b.volume);
    }
}

/// ROC or Momentum: compares with the value `n` bars back.
pub(crate) struct ChangeCalc {
    src: Source,
    ratio: bool,
    back: Ring,
}

impl ChangeCalc {
    pub(crate) fn roc(n: usize, src: Source) -> Self {
        Self {
            src,
            ratio: true,
            back: Ring::new(n.max(1)),
        }
    }

    pub(crate) fn momentum(n: usize, src: Source) -> Self {
        Self {
            src,
            ratio: false,
            back: Ring::new(n.max(1)),
        }
    }
}

impl Calc for ChangeCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let p = self.src.of(b);
        out[0] = if self.back.is_full() {
            let old = self.back.get(0);
            if !self.ratio {
                p - old
            } else if old == 0.0 {
                f64::NAN
            } else {
                100.0 * (p / old - 1.0)
            }
        } else {
            f64::NAN
        };
    }
    fn push(&mut self, b: &Bar) {
        self.back.push(self.src.of(b));
    }
}

pub(crate) struct TrixCalc {
    src: Source,
    k: EmaChain,
    /// The committed newest triple EMA.
    prev: f64,
}

impl TrixCalc {
    pub(crate) fn new(n: usize, src: Source) -> Self {
        Self {
            src,
            k: EmaChain::new(n),
            prev: f64::NAN,
        }
    }
}

impl Calc for TrixCalc {
    fn outputs(&self) -> usize {
        1
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let e3 = self.k.peek(self.src.of(b))[2];
        out[0] = if self.prev.is_nan() || self.prev == 0.0 {
            f64::NAN
        } else {
            100.0 * (e3 / self.prev - 1.0)
        };
    }
    fn push(&mut self, b: &Bar) {
        let p = self.src.of(b);
        let e3 = self.k.peek(p)[2];
        self.k.push(p);
        self.prev = e3;
    }
}

/// Outputs: up, down.
pub(crate) struct AroonCalc {
    n: usize,
    hi: RollingExtreme,
    lo: RollingExtreme,
}

impl AroonCalc {
    pub(crate) fn new(n: usize) -> Self {
        let n = n.max(1);
        Self {
            n,
            hi: RollingExtreme::max(n + 1),
            lo: RollingExtreme::min(n + 1),
        }
    }
}

impl Calc for AroonCalc {
    fn outputs(&self) -> usize {
        2
    }
    fn peek(&self, b: &Bar, out: &mut [f64]) {
        let n = self.n as f64;
        let (h, since_high) = self.hi.peek_at(b.high);
        let (_, since_low) = self.lo.peek_at(b.low);
        if h.is_nan() {
            out[0] = f64::NAN;
            out[1] = f64::NAN;
        } else {
            out[0] = 100.0 * (n - since_high as f64) / n;
            out[1] = 100.0 * (n - since_low as f64) / n;
        }
    }
    fn push(&mut self, b: &Bar) {
        self.hi.push(b.high);
        self.lo.push(b.low);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    /// Each indicator with the defaults of `docs/book/src/indicators.md` and the name of
    /// its fixture.
    pub(crate) fn all() -> Vec<(&'static str, Box<dyn Calc>)> {
        factories().into_iter().map(|(n, f)| (n, f())).collect()
    }

    /// Like [`all`], as factories.
    pub(crate) fn factories() -> Vec<(&'static str, crate::runner::Factory)> {
        macro_rules! f {
            ($e:expr) => {
                Box::new(move || Box::new($e) as Box<dyn Calc>) as crate::runner::Factory
            };
        }
        let c = Source::Close;
        vec![
            ("sma", f!(SmaCalc::new(20, c))),
            ("ema", f!(EmaCalc::new(20, c))),
            ("wma", f!(WmaCalc::new(20, c))),
            ("hma", f!(HmaCalc::new(20, c))),
            ("vwma", f!(VwmaCalc::new(20, c))),
            ("dema", f!(DemaCalc::new(20, c))),
            ("tema", f!(TemaCalc::new(20, c))),
            ("volume_ma", f!(VolumeMaCalc::new(20))),
            ("boll", f!(BollCalc::new(20, 2.0, c))),
            ("keltner", f!(KeltnerCalc::new(20, 2.0, 10))),
            ("donchian", f!(DonchianCalc::new(20))),
            ("ichimoku", f!(IchimokuCalc::new(9, 26, 52))),
            ("sar", f!(SarCalc::new(0.02, 0.02, 0.2))),
            ("supertrend", f!(SuperTrendCalc::new(10, 3.0))),
            ("vwap", f!(VwapCalc::new(VwapAnchor::Day, 0))),
            ("vwap_month", f!(VwapCalc::new(VwapAnchor::Month, 0))),
            ("macd", f!(MacdCalc::new(12, 26, 9, c))),
            ("rsi", f!(RsiCalc::new(14, c))),
            ("stoch", f!(StochCalc::new(14, 3, 3))),
            ("stochrsi", f!(StochRsiCalc::new(14, 14, 3, 3, c))),
            ("cci", f!(CciCalc::new(20))),
            ("willr", f!(WillrCalc::new(14))),
            ("mfi", f!(MfiCalc::new(14))),
            ("adx", f!(AdxCalc::new(14))),
            ("atr", f!(AtrCalc::new(14))),
            ("obv", f!(ObvCalc::default())),
            ("cmf", f!(CmfCalc::new(20))),
            ("roc", f!(ChangeCalc::roc(10, c))),
            ("momentum", f!(ChangeCalc::momentum(10, c))),
            ("trix", f!(TrixCalc::new(15, c))),
            ("aroon", f!(AroonCalc::new(14))),
        ]
    }

    fn read_csv(name: &str) -> (Vec<String>, Vec<Vec<f64>>) {
        let text = std::fs::read_to_string(format!("{FIXTURES}/{name}.csv")).unwrap();
        let mut lines = text.lines();
        let header = lines
            .next()
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        let rows = lines
            .map(|l| l.split(',').map(|v| v.parse().unwrap()).collect())
            .collect();
        (header, rows)
    }

    pub(crate) fn input() -> Vec<Bar> {
        read_csv("input")
            .1
            .into_iter()
            .map(|r| Bar {
                time: r[0] as i64,
                open: r[1],
                high: r[2],
                low: r[3],
                close: r[4],
                volume: r[5],
            })
            .collect()
    }

    /// Full computation without display shifts: `[output][bar]`.
    pub(crate) fn run_raw(calc: &mut dyn Calc, bars: &[Bar]) -> Vec<Vec<f64>> {
        let mut out = vec![vec![f64::NAN; bars.len()]; calc.outputs()];
        let mut y = vec![0.0; calc.outputs()];
        for (t, b) in bars.iter().enumerate() {
            calc.peek(b, &mut y);
            calc.push(b);
            for (o, &v) in y.iter().enumerate() {
                out[o][t] = v;
            }
        }
        out
    }

    /// Full computation with the display shifts of indicator `name`'s
    /// public outputs (`OutputSpec::shift`): `[output][bar]`, as drawn.
    pub(crate) fn run(name: &str, calc: &mut dyn Calc, bars: &[Bar]) -> Vec<Vec<f64>> {
        use ferrochart_core::plugin::Indicator;
        let raw = run_raw(calc, bars);
        let shifts: Vec<i32> = if name == "ichimoku" {
            let ind = crate::ichimoku(9, 26, 52, 26).unwrap();
            ind.outputs().iter().map(|o| o.shift).collect()
        } else {
            vec![0; raw.len()]
        };
        raw.iter()
            .zip(shifts)
            .map(|(col, s)| {
                (0..bars.len())
                    .map(|t| {
                        t.checked_add_signed(-(s as isize))
                            .and_then(|src| col.get(src))
                            .copied()
                            .unwrap_or(f64::NAN)
                    })
                    .collect()
            })
            .collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact by construction
    fn sources_follow_the_spec() {
        let b = Bar {
            time: 0,
            open: 1.0,
            high: 4.0,
            low: 0.5,
            close: 2.0,
            volume: 1.0,
        };
        let got = [
            Source::Close,
            Source::Open,
            Source::High,
            Source::Low,
            Source::Hl2,
            Source::Hlc3,
            Source::Ohlc4,
        ]
        .map(|s| s.of(&b));
        assert_eq!(got, [2.0, 1.0, 4.0, 0.5, 2.25, 6.5 / 3.0, 7.5 / 4.0]);
    }

    /// VWAP restarts at Monday 00:00 (week) or starts at `t0` (time).
    #[test]
    #[allow(clippy::float_cmp)] // exact by construction
    fn vwap_anchors() {
        const DAY: i64 = 86_400_000;
        // 1970-01-05 was a Monday; bars every 12 h from Saturday 1970-01-03.
        let bars: Vec<Bar> = (0..8)
            .map(|i| {
                let p = f64::from(i) + 1.0;
                Bar {
                    time: 2 * DAY + i64::from(i) * DAY / 2,
                    open: p,
                    high: p,
                    low: p,
                    close: p,
                    volume: 1.0,
                }
            })
            .collect();
        let week = run_raw(&mut VwapCalc::new(VwapAnchor::Week, 0), &bars)[0].clone();
        // Saturday and Sunday (4 bars) average 1..4, then Monday restarts.
        assert_eq!(&week[..6], &[1.0, 1.5, 2.0, 2.5, 5.0, 5.5]);
        let anchored = run_raw(&mut VwapCalc::new(VwapAnchor::Time(3 * DAY), 0), &bars)[0].clone();
        assert!(anchored[..2].iter().all(|v| v.is_nan()));
        assert_eq!(&anchored[2..5], &[3.0, 3.5, 4.0]);
        // A UTC offset moves the session boundary: +12 h makes bar 3 Monday.
        let shifted = run_raw(&mut VwapCalc::new(VwapAnchor::Week, DAY / 2), &bars)[0].clone();
        assert_eq!(shifted[3], 4.0);
    }

    /// The indicators TA-Lib has match it, from the
    /// first value on, or after the bars `docs/book/src/indicators.md` names where the
    /// seeds differ (MACD 250, ADX/DMI 450).
    #[test]
    fn indicators_match_ta_lib() {
        let bars = input();
        let calc = |name: &str| -> Box<dyn Calc> {
            match name {
                // TA-Lib's STOCHRSI does not smooth %K.
                "stochrsi" => Box::new(StochRsiCalc::new(14, 14, 1, 3, Source::Close)),
                _ => factories().into_iter().find(|(n, _)| *n == name).unwrap().1(),
            }
        };
        let names = [
            ("sma", 0),
            ("ema", 0),
            ("wma", 0),
            ("dema", 0),
            ("tema", 0),
            ("boll", 0),
            ("sar", 0),
            ("macd", 250),
            ("rsi", 0),
            ("stoch", 0),
            ("stochrsi", 0),
            ("cci", 0),
            ("willr", 0),
            ("mfi", 0),
            ("adx", 450),
            ("atr", 0),
            ("obv", 0),
            ("roc", 0),
            ("momentum", 0),
            ("trix", 0),
            ("aroon", 0),
        ];
        let mut failed = Vec::new();
        for (name, skip) in names {
            let (header, rows) = read_csv(&format!("talib/{name}"));
            let got = run(name, calc(name).as_mut(), &bars);
            for (o, col) in header.iter().enumerate() {
                let mut compared = 0;
                for t in skip..bars.len() {
                    let want = rows[t][o];
                    if want.is_nan() {
                        continue;
                    }
                    compared += 1;
                    if !close(got[o][t], want) {
                        failed.push(format!("{name}.{col}[{t}]: got {} want {want}", got[o][t]));
                        break;
                    }
                }
                assert!(compared > 500, "{name}.{col}: compared only {compared}");
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// `docs/book/src/indicators.md`: every indicator matches the numpy reference.
    #[test]
    fn indicators_match_the_reference() {
        let bars = input();
        let mut failed = Vec::new();
        for (name, mut calc) in all() {
            let (header, rows) = read_csv(name);
            assert_eq!(header.len(), calc.outputs(), "{name}: outputs");
            let got = run(name, calc.as_mut(), &bars);
            for (o, col) in header.iter().enumerate() {
                let finite = rows.iter().filter(|r| r[o].is_finite()).count();
                assert!(
                    finite > bars.len() / 10,
                    "{name}.{col}: only {finite} values"
                );
                let bad = (0..bars.len()).find(|&t| !close(got[o][t], rows[t][o]));
                if let Some(t) = bad {
                    failed.push(format!(
                        "{name}.{col}[{t}]: got {} want {}",
                        got[o][t], rows[t][o]
                    ));
                }
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }
}
