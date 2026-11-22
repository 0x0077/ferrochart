//! Incremental building blocks of the indicators (`docs/book/src/indicators.md`).
//!
//! Every kernel holds only *committed* values. [`peek`](Sma::peek) computes
//! the output for a tentative newest value without changing the state, and
//! `push` commits a value. A real-time `UpdateLast` is therefore just another
//! `peek` (O(1), nothing to undo), `Append` is `push` of the previous last
//! value followed by `peek`, and a full computation runs the very same
//! `peek`/`push` sequence, so incremental and full results are bit-identical.
//!
//! NaN inputs are ignored: `peek(NaN)` is NaN and `push(NaN)` does nothing.
//! Chained kernels (the EMA of an EMA) thus skip their input's warmup, and the
//! first valid values seed them (`docs/book/src/indicators.md`).
//!
//! Running sums are recomputed from the window every `n` pushes, so rounding
//! errors do not accumulate. Buffers are allocated once, in `new`.

use std::collections::VecDeque;

/// The last `cap` committed values, oldest first.
#[derive(Clone, Debug)]
pub(crate) struct Ring {
    buf: Vec<f64>,
    /// Index of the oldest value.
    head: usize,
    len: usize,
}

impl Ring {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            buf: vec![0.0; cap],
            head: 0,
            len: 0,
        }
    }

    pub(crate) fn cap(&self) -> usize {
        self.buf.len()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.len == self.buf.len()
    }

    /// The `i`-th value, oldest first.
    pub(crate) fn get(&self, i: usize) -> f64 {
        self.buf[(self.head + i) % self.buf.len()]
    }

    /// Values oldest first.
    pub(crate) fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(|i| self.get(i))
    }

    /// Adds `x`; returns the value it evicted when full.
    pub(crate) fn push(&mut self, x: f64) -> Option<f64> {
        let cap = self.buf.len();
        if cap == 0 {
            return Some(x);
        }
        if self.len < cap {
            self.buf[(self.head + self.len) % cap] = x;
            self.len += 1;
            None
        } else {
            let old = self.buf[self.head];
            self.buf[self.head] = x;
            self.head = (self.head + 1) % cap;
            Some(old)
        }
    }
}

/// Sum of the newest `n` values: the `n − 1` newest committed ones plus the
/// peeked one.
#[derive(Clone, Debug)]
pub(crate) struct RollingSum {
    ring: Ring,
    sum: f64,
    /// Pushes since `sum` was last recomputed from the window.
    since_exact: usize,
}

impl RollingSum {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            ring: Ring::new(n.saturating_sub(1)),
            sum: 0.0,
            since_exact: 0,
        }
    }

    /// Window length `n`.
    pub(crate) fn n(&self) -> usize {
        self.ring.cap() + 1
    }

    pub(crate) fn peek(&self, x: f64) -> f64 {
        if x.is_nan() || !self.ring.is_full() {
            return f64::NAN;
        }
        self.sum + x
    }

    pub(crate) fn push(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        match self.ring.push(x) {
            Some(old) => self.sum = self.sum - old + x,
            None => self.sum += x,
        }
        self.since_exact += 1;
        if self.since_exact >= self.n() {
            self.sum = self.ring.iter().sum();
            self.since_exact = 0;
        }
    }

    /// The committed window, oldest first.
    pub(crate) fn window(&self) -> &Ring {
        &self.ring
    }
}

/// `SMA(x, n)`.
#[derive(Clone, Debug)]
pub(crate) struct Sma(RollingSum);

impl Sma {
    pub(crate) fn new(n: usize) -> Self {
        Self(RollingSum::new(n))
    }

    pub(crate) fn peek(&self, x: f64) -> f64 {
        self.0.peek(x) / self.0.n() as f64
    }

    pub(crate) fn push(&mut self, x: f64) {
        self.0.push(x);
    }

    /// The committed window (the `n − 1` newest values), oldest first.
    pub(crate) fn window(&self) -> &Ring {
        self.0.window()
    }
}

/// How a [`Smooth`] kernel moves towards the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SmoothKind {
    /// `e + α (x − e)`, `α = 2/(n+1)`.
    Ema,
    /// Wilder: `r + (x − r)/n`.
    Rma,
}

/// `EMA(x, n)` or `RMA(x, n)`, seeded by the SMA of the first `n` values.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Smooth {
    kind: SmoothKind,
    n: usize,
    alpha: f64,
    /// Committed values so far, saturating at `n`.
    count: usize,
    /// Sum of the first values until seeded, then the committed average.
    acc: f64,
}

impl Smooth {
    pub(crate) fn new(kind: SmoothKind, n: usize) -> Self {
        let n = n.max(1);
        Self {
            kind,
            n,
            alpha: 2.0 / (n as f64 + 1.0),
            count: 0,
            acc: 0.0,
        }
    }

    pub(crate) fn ema(n: usize) -> Self {
        Self::new(SmoothKind::Ema, n)
    }

    pub(crate) fn rma(n: usize) -> Self {
        Self::new(SmoothKind::Rma, n)
    }

    fn step(&self, x: f64) -> f64 {
        if self.count < self.n {
            // Seeding: the SMA of the first n values.
            if self.count + 1 == self.n {
                (self.acc + x) / self.n as f64
            } else {
                f64::NAN
            }
        } else {
            match self.kind {
                SmoothKind::Ema => self.acc + self.alpha * (x - self.acc),
                SmoothKind::Rma => self.acc + (x - self.acc) / self.n as f64,
            }
        }
    }

    pub(crate) fn peek(&self, x: f64) -> f64 {
        if x.is_nan() { f64::NAN } else { self.step(x) }
    }

    pub(crate) fn push(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        if self.count + 1 < self.n {
            self.acc += x;
        } else {
            self.acc = self.step(x);
        }
        self.count = (self.count + 1).min(self.n);
    }
}

/// `WMA(x, n)`: weights `n` for the newest value down to 1.
#[derive(Clone, Debug)]
pub(crate) struct Wma {
    /// The `n − 1` newest committed values.
    ring: Ring,
    /// `Σ c` and `Σ w·c` over the ring, weights `n − 1` (newest) down to 1.
    sum: f64,
    weighted: f64,
    since_exact: usize,
}

impl Wma {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            ring: Ring::new(n.max(1) - 1),
            sum: 0.0,
            weighted: 0.0,
            since_exact: 0,
        }
    }

    fn n(&self) -> usize {
        self.ring.cap() + 1
    }

    pub(crate) fn peek(&self, x: f64) -> f64 {
        if x.is_nan() || !self.ring.is_full() {
            return f64::NAN;
        }
        let n = self.n() as f64;
        (self.weighted + n * x) / (n * (n + 1.0) / 2.0)
    }

    pub(crate) fn push(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        let cap = self.ring.cap();
        let was_full = self.ring.is_full();
        let old = self.ring.push(x);
        self.since_exact += 1;
        match old {
            Some(old) if was_full && cap > 0 && self.since_exact < self.n() => {
                // Every committed weight drops by one (the oldest to 0, i.e.
                // out) and `x` comes in with weight n − 1.
                self.weighted = self.weighted - self.sum + cap as f64 * x;
                self.sum = self.sum - old + x;
            }
            _ => self.recompute(),
        }
    }

    fn recompute(&mut self) {
        let mut sum = 0.0;
        let mut weighted = 0.0;
        for (i, c) in self.ring.iter().enumerate() {
            sum += c;
            weighted += (i + 1) as f64 * c;
        }
        self.sum = sum;
        self.weighted = weighted;
        self.since_exact = 0;
    }
}

/// Wilder's running sum (ADX/DMI): the sum of the first `n` values, then
/// `S − S/n + x`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WilderSum {
    n: usize,
    count: usize,
    sum: f64,
}

impl WilderSum {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            n: n.max(1),
            count: 0,
            sum: 0.0,
        }
    }

    fn step(&self, x: f64) -> f64 {
        if self.count < self.n {
            self.sum + x
        } else {
            self.sum - self.sum / self.n as f64 + x
        }
    }

    pub(crate) fn peek(&self, x: f64) -> f64 {
        if x.is_nan() || self.count + 1 < self.n {
            f64::NAN
        } else {
            self.step(x)
        }
    }

    pub(crate) fn push(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        self.sum = self.step(x);
        self.count = (self.count + 1).min(self.n);
    }
}

/// Maximum (or minimum) of the newest `n` values, with a monotonic deque of
/// indices into the committed values (amortized O(1)).
#[derive(Clone, Debug)]
pub(crate) struct RollingExtreme {
    n: usize,
    max: bool,
    /// (sequence number, value), values strictly decreasing for `max`
    /// (increasing for min); the front is the extreme.
    deque: VecDeque<(u64, f64)>,
    /// Sequence number of the next committed value.
    seq: u64,
}

impl RollingExtreme {
    pub(crate) fn max(n: usize) -> Self {
        Self::new(n, true)
    }

    pub(crate) fn min(n: usize) -> Self {
        Self::new(n, false)
    }

    fn new(n: usize, max: bool) -> Self {
        let n = n.max(1);
        Self {
            n,
            max,
            deque: VecDeque::with_capacity(n),
            seq: 0,
        }
    }

    /// `a` is at least as extreme as `b`.
    fn beats(&self, a: f64, b: f64) -> bool {
        if self.max { a >= b } else { a <= b }
    }

    /// Committed values in the window of a peeked value: sequence numbers
    /// `> seq − n`.
    fn front(&self) -> Option<(u64, f64)> {
        let lo = self.seq + 1;
        self.deque
            .iter()
            .copied()
            .find(|&(s, _)| s + self.n as u64 >= lo)
    }

    /// The extreme over the `n − 1` newest committed values and `x`; NaN until
    /// `n − 1` values are committed.
    pub(crate) fn peek(&self, x: f64) -> f64 {
        self.peek_at(x).0
    }

    /// The extreme and how many values back it is (0 = `x`; ties go to the
    /// newest).
    pub(crate) fn peek_at(&self, x: f64) -> (f64, usize) {
        if x.is_nan() || self.seq + 1 < self.n as u64 {
            return (f64::NAN, 0);
        }
        // The front is the oldest in-window committed extreme; a newer value
        // that ties it is further back in the deque only when strictly better,
        // so ties resolve to the newest by construction (see `push`).
        match self.front() {
            Some((s, v)) if !self.beats(x, v) => (v, (self.seq - s) as usize),
            _ => (x, 0),
        }
    }

    pub(crate) fn push(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        // Drop values that `x` matches or beats: on ties the newest wins.
        while let Some(&(_, b)) = self.deque.back() {
            if self.beats(x, b) {
                self.deque.pop_back();
            } else {
                break;
            }
        }
        // Drop values that no future window contains (window of a future
        // peek: committed sequence numbers > seq + 1 − n).
        let s = self.seq;
        while let Some(&(f, _)) = self.deque.front() {
            if f + self.n as u64 <= s + 1 {
                self.deque.pop_front();
            } else {
                break;
            }
        }
        self.deque.push_back((s, x));
        self.seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn close(a: f64, b: f64) -> bool {
        (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    /// Runs `step` (peek, then push) over `xs`.
    fn run(xs: &[f64], step: impl FnMut(f64) -> f64) -> Vec<f64> {
        xs.iter().copied().map(step).collect()
    }

    fn naive(xs: &[f64], n: usize, f: impl Fn(&[f64]) -> f64) -> Vec<f64> {
        (0..xs.len())
            .map(|t| {
                if t + 1 >= n {
                    f(&xs[t + 1 - n..=t])
                } else {
                    f64::NAN
                }
            })
            .collect()
    }

    fn values() -> impl Strategy<Value = Vec<f64>> {
        prop::collection::vec(
            prop_oneof![(-1000i32..1000).prop_map(f64::from), -1e3..1e3f64],
            0..200,
        )
    }

    proptest! {
        #[test]
        fn sma_matches_the_window_mean(xs in values(), n in 1usize..30) {
            let mut k = Sma::new(n);
            let got = run(&xs, |x| { let y = k.peek(x); k.push(x); y });
            let want = naive(&xs, n, |w| w.iter().sum::<f64>() / n as f64);
            prop_assert!(got.iter().zip(&want).all(|(a, b)| close(*a, *b)), "{got:?} {want:?}");
        }

        #[test]
        fn wma_matches_the_weighted_mean(xs in values(), n in 1usize..30) {
            let mut k = Wma::new(n);
            let got = run(&xs, |x| { let y = k.peek(x); k.push(x); y });
            let den = (n * (n + 1) / 2) as f64;
            let want = naive(&xs, n, |w| {
                w.iter().enumerate().map(|(i, v)| (i + 1) as f64 * v).sum::<f64>() / den
            });
            prop_assert!(got.iter().zip(&want).all(|(a, b)| close(*a, *b)), "{got:?} {want:?}");
        }

        #[test]
        fn extremes_match_the_window(xs in values(), n in 1usize..30) {
            let (mut hi, mut lo) = (RollingExtreme::max(n), RollingExtreme::min(n));
            let mut got = Vec::new();
            for &x in &xs {
                got.push((hi.peek_at(x), lo.peek_at(x)));
                hi.push(x);
                lo.push(x);
            }
            for t in 0..xs.len() {
                if t + 1 < n {
                    prop_assert!(got[t].0.0.is_nan());
                    continue;
                }
                let w = &xs[t + 1 - n..=t];
                let max = w.iter().copied().fold(f64::MIN, f64::max);
                let min = w.iter().copied().fold(f64::MAX, f64::min);
                // Bars back to the newest occurrence.
                let back_max = w.iter().rev().position(|&v| v.to_bits() == max.to_bits()).unwrap();
                let back_min = w.iter().rev().position(|&v| v.to_bits() == min.to_bits()).unwrap();
                prop_assert_eq!(got[t].0, (max, back_max));
                prop_assert_eq!(got[t].1, (min, back_min));
            }
        }

        /// EMA/RMA: SMA seed of the first n values, NaN inputs skipped.
        #[test]
        fn smoothing_follows_the_recurrence(xs in values(), n in 1usize..30, lead in 0usize..5, ema in any::<bool>()) {
            let mut input = vec![f64::NAN; lead];
            input.extend_from_slice(&xs);
            let mut k = if ema { Smooth::ema(n) } else { Smooth::rma(n) };
            let got = run(&input, |x| { let y = k.peek(x); k.push(x); y });
            let mut want = vec![f64::NAN; input.len()];
            if xs.len() >= n {
                let mut e = xs[..n].iter().sum::<f64>() / n as f64;
                want[lead + n - 1] = e;
                for t in n..xs.len() {
                    e = if ema {
                        e + 2.0 / (n as f64 + 1.0) * (xs[t] - e)
                    } else {
                        e + (xs[t] - e) / n as f64
                    };
                    want[lead + t] = e;
                }
            }
            // The seed sum is accumulated in the same order: bit-identical.
            prop_assert!(got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits() || close(*a, *b)));
        }

        /// Peeking never changes the state: interleaving any number of peeks
        /// (real-time updates of the last bar) gives the same results.
        #[test]
        fn peek_has_no_effect(xs in values(), n in 1usize..20, extra in values()) {
            let (mut a, mut b) = (Wma::new(n), Wma::new(n));
            let (mut c, mut d) = (RollingExtreme::max(n), RollingExtreme::max(n));
            for (i, &x) in xs.iter().enumerate() {
                for &e in extra.iter().skip(i % 3).take(2) {
                    let _ = (b.peek(e), d.peek(e));
                }
                prop_assert_eq!(a.peek(x).to_bits(), b.peek(x).to_bits());
                prop_assert_eq!(c.peek(x).to_bits(), d.peek(x).to_bits());
                a.push(x); b.push(x); c.push(x); d.push(x);
            }
        }
    }
}
