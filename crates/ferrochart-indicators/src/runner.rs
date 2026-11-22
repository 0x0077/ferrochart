//! Drives a [`Calc`] from the slots of its source series and writes the
//! outputs of the slots that changed (`docs/book/src/indicators.md`).
//!
//! The runner commits every bar but the last; the last bar is only peeked, so
//! a real-time update of it is one `peek` (O(1), no allocation) and a new bar
//! is one `push` plus one `peek`. Gap slots are skipped (their outputs stay
//! NaN). Display shifts are the chart's business (`OutputSpec::shift`).

use ferrochart_core::plugin::{IndicatorInput, IndicatorOutputs};

use crate::calc::{Bar, Calc};

/// The source series, slot by slot.
pub(crate) trait Input {
    /// Number of slots.
    fn len(&self) -> usize;
    /// The bar at `slot`; `None` for a gap.
    fn bar(&self, slot: usize) -> Option<Bar>;
}

impl Input for IndicatorInput<'_> {
    fn len(&self) -> usize {
        IndicatorInput::len(self)
    }
    fn bar(&self, slot: usize) -> Option<Bar> {
        IndicatorInput::bar(self, slot).map(|b| Bar {
            time: b.time,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        })
    }
}

impl Input for Vec<Option<Bar>> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn bar(&self, slot: usize) -> Option<Bar> {
        self.get(slot).copied().flatten()
    }
}

/// Where the outputs go.
pub(crate) trait Sink {
    /// Output `o` of `slot` is `v`.
    fn put(&mut self, o: usize, slot: usize, v: f64);
}

impl Sink for IndicatorOutputs {
    fn put(&mut self, o: usize, slot: usize, v: f64) {
        let start = self.start();
        if let Some(x) = slot
            .checked_sub(start)
            .and_then(|i| self.column(o).get_mut(i))
        {
            *x = v;
        }
    }
}

/// `[output][slot]`, grown as needed (tests).
impl Sink for Vec<Vec<f64>> {
    fn put(&mut self, o: usize, slot: usize, v: f64) {
        if self.len() <= o {
            self.resize(o + 1, Vec::new());
        }
        let col = &mut self[o];
        if col.len() <= slot {
            col.resize(slot + 1, f64::NAN);
        }
        col[slot] = v;
    }
}

/// Builds a fresh calculation (full recomputation).
pub(crate) type Factory = Box<dyn Fn() -> Box<dyn Calc> + Send>;

pub(crate) struct Runner {
    factory: Factory,
    calc: Box<dyn Calc>,
    /// The last valid bar and its slot, peeked but not committed.
    pending: Option<(usize, Bar)>,
    scratch: Vec<f64>,
}

impl Runner {
    pub(crate) fn new(factory: Factory) -> Self {
        let calc = factory();
        let k = calc.outputs();
        Self {
            factory,
            calc,
            pending: None,
            scratch: vec![0.0; k],
        }
    }

    /// Recomputes everything (`Replace`, `Prepend`).
    pub(crate) fn full(&mut self, input: &dyn Input, out: &mut dyn Sink) {
        self.calc = (self.factory)();
        self.pending = None;
        self.append(input, input.len(), out);
    }

    /// The last `count` slots of `input` are new (`Append`).
    pub(crate) fn append(&mut self, input: &dyn Input, count: usize, out: &mut dyn Sink) {
        let len = input.len();
        for slot in len.saturating_sub(count)..len {
            let Some(b) = input.bar(slot) else { continue };
            if let Some((_, p)) = self.pending.take() {
                self.calc.push(&p);
            }
            self.pending = Some((slot, b));
            self.write_pending(out);
        }
    }

    /// The last slot of `input` changed (`UpdateLast`). O(1) unless it is
    /// not the pending bar (it switched between gap and bar), which
    /// recomputes everything.
    pub(crate) fn update_last(&mut self, input: &dyn Input, out: &mut dyn Sink) {
        let last = input.len().checked_sub(1);
        match (self.pending, last.and_then(|j| input.bar(j))) {
            (Some((slot, _)), Some(b)) if Some(slot) == last => {
                self.pending = Some((slot, b));
                self.write_pending(out);
            }
            _ => self.full(input, out),
        }
    }

    fn write_pending(&mut self, out: &mut dyn Sink) {
        let Some((slot, b)) = self.pending else {
            return;
        };
        self.calc.peek(&b, &mut self.scratch);
        for (o, &v) in self.scratch.iter().enumerate() {
            out.put(o, slot, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::tests::{factories, input, run_raw};
    use proptest::prelude::*;

    fn same(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
    }

    /// `docs/book/src/indicators.md`: gap bars are skipped. The outputs on data
    /// with gaps are those of the valid bars, spread back, NaN at gaps.
    #[test]
    fn gap_bars_are_skipped() {
        let bars = input();
        let slots: Vec<Option<Bar>> = bars
            .iter()
            .enumerate()
            .flat_map(|(i, b)| {
                // A gap before every 37th bar, two before every 101st.
                let gaps = usize::from(i % 37 == 5) + 2 * usize::from(i % 101 == 7);
                std::iter::repeat_n(None, gaps).chain(std::iter::once(Some(*b)))
            })
            .collect();
        for (name, f) in factories() {
            let want = run_raw(f().as_mut(), &bars);
            let mut r = Runner::new(f);
            let mut out: Vec<Vec<f64>> = Vec::new();
            r.full(&slots, &mut out);
            for (o, col) in out.iter_mut().enumerate() {
                col.resize(slots.len(), f64::NAN);
                let valid: Vec<f64> = col
                    .iter()
                    .zip(&slots)
                    .filter(|(_, b)| b.is_some())
                    .map(|(v, _)| *v)
                    .collect();
                assert!(same(&valid, &want[o]), "{name} output {o}");
                assert!(
                    col.iter()
                        .zip(&slots)
                        .all(|(v, b)| b.is_some() || v.is_nan()),
                    "{name} output {o}: value at a gap"
                );
            }
        }
    }

    #[derive(Clone, Debug)]
    enum Op {
        /// Append this many slots; `true` = gap.
        Append(Vec<bool>),
        /// Replace the last bar by input bar `k` (shifted in time).
        UpdateLast(usize),
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(
            prop_oneof![
                prop::collection::vec(prop::bool::weighted(0.1), 1..40).prop_map(Op::Append),
                (0usize..1000).prop_map(Op::UpdateLast),
            ],
            1..40,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Incremental results equal full results, bit
        /// for bit, for every indicator, under random appends and real-time
        /// updates of the last bar.
        #[test]
        fn incremental_equals_full(ops in ops()) {
            let pool = input();
            let mut runners: Vec<(&str, Runner, Vec<Vec<f64>>)> = factories()
                .into_iter()
                .map(|(n, f)| (n, Runner::new(f), Vec::new()))
                .collect();
            let mut slots: Vec<Option<Bar>> = Vec::new();
            let mut next = 0usize;
            for op in &ops {
                match op {
                    Op::Append(gaps) => {
                        for &gap in gaps {
                            slots.push((!gap).then(|| pool[next % pool.len()]));
                            next += 1;
                        }
                        for (_, r, out) in &mut runners {
                            r.append(&slots, gaps.len(), out);
                        }
                    }
                    Op::UpdateLast(k) => {
                        let Some(Some(last)) = slots.last().copied() else { continue };
                        let b = pool[*k];
                        // Keep the time (VWAP sessions), take the new prices.
                        *slots.last_mut().unwrap() = Some(Bar { time: last.time, ..b });
                        for (_, r, out) in &mut runners {
                            r.update_last(&slots, out);
                        }
                    }
                }
            }
            for (name, _, out) in &runners {
                let mut fresh = Runner::new(factories().into_iter().find(|(n, _)| n == name).unwrap().1);
                let mut want: Vec<Vec<f64>> = Vec::new();
                fresh.full(&slots, &mut want);
                prop_assert_eq!(out.len(), want.len(), "{}", name);
                for o in 0..out.len() {
                    prop_assert!(same(&out[o], &want[o]), "{} output {}", name, o);
                }
            }
        }
    }
}
