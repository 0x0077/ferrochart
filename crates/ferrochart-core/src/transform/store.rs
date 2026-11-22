//! Storage of a transform series: the input
//! bars, kept inside the series, and the output elements, which are the
//! whole time axis (a chart has at most one transform series and no
//! time-based series next to it).
//!
//! The output is the elements `push` wrote for every input but the last
//! valid one (committed, never rewritten) followed by what `peek` writes for
//! the last one (the tail). Each change rewrites only the tail: positions
//! that differ are overwritten in place, and the store and the axis are
//! truncated or extended when the tail's length changes.

use crate::data::ohlc::{OhlcStore, check_columns};
use crate::data::value::check_values;
use crate::data::{
    BarUpdate, DataChange, OhlcColumns, TimeIndex, Timestamp, ValidationMode, ValueColumns,
};
use crate::error::{Error, Result};
use crate::plugin::{InputBar, SeriesTransform, TransformBar, TransformOutput};

/// A transform series' data.
pub(crate) struct TransformStore {
    pub(crate) plugin: Box<dyn SeriesTransform>,
    /// The inputs as set (gap bars included).
    input: OhlcColumns,
    /// The last input that is not a gap (the one that is peeked).
    last_real: Option<usize>,
    /// The output; logical index `out.first() + p` is slot `p` of the axis.
    pub(crate) out: OhlcStore,
    /// `TransformBar::flag` of each output element.
    pub(crate) flags: Vec<u8>,
    /// Output elements written by `push`.
    committed: usize,
    /// Time of the last committed element.
    last_time: Option<Timestamp>,
    // Reused buffers.
    buf: TransformOutput,
    finished: Vec<(Timestamp, TransformBar)>,
    times: Vec<Timestamp>,
}

impl core::fmt::Debug for TransformStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TransformStore")
            .field("name", &self.plugin.name())
            .field("inputs", &self.input.time.len())
            .field("outputs", &self.out.len())
            .field("committed", &self.committed)
            .finish_non_exhaustive()
    }
}

/// Input bar `i`; `None` for a gap bar.
fn input_bar(c: &OhlcColumns, i: usize) -> Option<InputBar> {
    let open = *c.open.get(i)?;
    if open.is_nan() {
        return None;
    }
    Some(InputBar {
        time: *c.time.get(i)?,
        open,
        high: c.high[i],
        low: c.low[i],
        close: c.close[i],
        volume: c.volume.as_ref().map_or(f64::NAN, |v| v[i]),
    })
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// `Err(TimeNotIncreasing)` unless `times` are strictly increasing and come
/// after `after`.
fn check_times(times: &[Timestamp], after: Option<Timestamp>) -> Result<()> {
    if let (Some(a), Some(&t)) = (after, times.first())
        && t <= a
    {
        return Err(Error::TimeNotIncreasing { index: 0 });
    }
    match times.windows(2).position(|w| w[1] <= w[0]) {
        Some(k) => Err(Error::TimeNotIncreasing { index: k + 1 }),
        None => Ok(()),
    }
}

/// Validated input columns (value data becomes `o = h = l = c`). Returns
/// the columns and the number of repaired bars.
pub(crate) fn input_columns(
    data: crate::chart::SeriesData,
    mode: ValidationMode,
) -> Result<(OhlcColumns, usize)> {
    match data {
        crate::chart::SeriesData::Ohlc(mut c) => {
            if c.time.len() != c.open.len() {
                return Err(Error::LengthMismatch { column: "open" });
            }
            let repaired = check_columns(&mut c, mode)?;
            check_times(&c.time, None)?;
            Ok((c, repaired))
        }
        crate::chart::SeriesData::Value(ValueColumns { time, value, .. }) => {
            if value.len() != time.len() {
                return Err(Error::LengthMismatch { column: "value" });
            }
            check_values(&value)?;
            check_times(&time, None)?;
            Ok((
                OhlcColumns {
                    time,
                    open: value.clone(),
                    high: value.clone(),
                    low: value.clone(),
                    close: value,
                    volume: None,
                },
                0,
            ))
        }
    }
}

fn extend<T: Copy>(dst: &mut Vec<T>, src: &[T]) -> Result<()> {
    dst.try_reserve(src.len()).map_err(|_| Error::OutOfMemory)?;
    dst.extend_from_slice(src);
    Ok(())
}

impl TransformStore {
    pub(crate) fn new(plugin: Box<dyn SeriesTransform>) -> Self {
        Self {
            plugin,
            input: OhlcColumns::default(),
            last_real: None,
            out: OhlcStore::default(),
            flags: Vec::new(),
            committed: 0,
            last_time: None,
            buf: TransformOutput::default(),
            finished: Vec::new(),
            times: Vec::new(),
        }
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        let cols = 5 + usize::from(self.input.volume.is_some());
        cols * self.input.time.capacity() * 8 + self.out.heap_bytes() + self.flags.capacity()
    }

    /// Replaces the inputs and recomputes everything; `index` becomes the
    /// output times. On error nothing changes.
    pub(crate) fn set(&mut self, index: &mut TimeIndex, input: OhlcColumns) -> Result<()> {
        let old = core::mem::replace(&mut self.input, input);
        let out = self.recompute(index);
        if out.is_err() {
            self.input = old;
            // It succeeded with these inputs before.
            let _ = self.recompute(index);
        }
        out
    }

    /// Recomputes the whole output from the inputs.
    fn recompute(&mut self, index: &mut TimeIndex) -> Result<()> {
        self.plugin.reset();
        let n = self.input.time.len();
        let last_real = (0..n).rev().find(|&i| input_bar(&self.input, i).is_some());
        let mut elems: Vec<(Timestamp, TransformBar)> = Vec::new();
        let mut committed = 0;
        for i in 0..n {
            let Some(bar) = input_bar(&self.input, i) else {
                continue;
            };
            self.buf.clear();
            let last = Some(i) == last_real;
            if last {
                self.plugin.peek(bar, &mut self.buf);
            } else {
                self.plugin.push(bar, &mut self.buf);
            }
            self.buf.check()?;
            elems
                .try_reserve(self.buf.as_slice().len())
                .map_err(|_| Error::OutOfMemory)?;
            elems.extend(self.buf.as_slice().iter().map(|b| (bar.time, *b)));
            if !last {
                committed = elems.len();
            }
        }
        // Times.
        let mut times = Vec::new();
        times
            .try_reserve_exact(elems.len())
            .map_err(|_| Error::OutOfMemory)?;
        let mut prev: Option<Timestamp> = None;
        for &(t, _) in &elems {
            let t = prev.map_or(t, |p| t.max(p.saturating_add(1)));
            times.push(t);
            prev = Some(t);
        }
        let last_time = committed.checked_sub(1).map(|p| times[p]);
        let cols = self.columns(&elems.iter().map(|e| &e.1))?;
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(elems.len())
            .map_err(|_| Error::OutOfMemory)?;
        flags.extend(elems.iter().map(|e| e.1.flag));
        let new_index = TimeIndex::from_times(times)?;
        self.out
            .replace_at(new_index.first_index(), cols, ValidationMode::Repair)?;
        *index = new_index;
        index.shrink_to_fit();
        self.flags = flags;
        self.committed = committed;
        self.last_time = last_time;
        self.last_real = last_real;
        Ok(())
    }

    /// Output columns (no times) of `bars`.
    fn columns<'a>(
        &self,
        bars: &(impl ExactSizeIterator<Item = &'a TransformBar> + Clone),
    ) -> Result<OhlcColumns> {
        let n = bars.len();
        let col = |f: fn(&TransformBar) -> f64| -> Result<Vec<f64>> {
            let mut v = Vec::new();
            v.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
            v.extend(bars.clone().map(f));
            Ok(v)
        };
        Ok(OhlcColumns {
            time: Vec::new(),
            open: col(|b| b.open)?,
            high: col(|b| b.high)?,
            low: col(|b| b.low)?,
            close: col(|b| b.close)?,
            volume: if self.input.volume.is_some() {
                Some(col(|b| if b.volume.is_nan() { 0.0 } else { b.volume })?)
            } else {
                None
            },
        })
    }

    /// Appends inputs after the last one. Returns the change of the output
    /// (`None` if it did not change).
    pub(crate) fn append(
        &mut self,
        index: &mut TimeIndex,
        new: OhlcColumns,
    ) -> Result<Option<DataChange>> {
        if self.input.time.is_empty() {
            self.set(index, new)?;
            return Ok(Some(DataChange::Replace));
        }
        if new.volume.is_some() != self.input.volume.is_some() {
            return Err(Error::LengthMismatch { column: "volume" });
        }
        check_times(&new.time, self.input.time.last().copied())?;
        let from = self.input.time.len();
        self.extend_input(&new)?;
        let out = self.advance(index, from);
        if out.is_err() {
            self.truncate_input(from);
            let _ = self.recompute(index);
        }
        out
    }

    fn extend_input(&mut self, new: &OhlcColumns) -> Result<()> {
        let from = self.input.time.len();
        let out = (|| {
            extend(&mut self.input.time, &new.time)?;
            extend(&mut self.input.open, &new.open)?;
            extend(&mut self.input.high, &new.high)?;
            extend(&mut self.input.low, &new.low)?;
            extend(&mut self.input.close, &new.close)?;
            if let (Some(v), Some(src)) = (self.input.volume.as_mut(), new.volume.as_ref()) {
                extend(v, src)?;
            }
            Ok(())
        })();
        if out.is_err() {
            self.truncate_input(from);
        }
        out
    }

    fn truncate_input(&mut self, n: usize) {
        let c = &mut self.input;
        c.time.truncate(n);
        c.open.truncate(n);
        c.high.truncate(n);
        c.low.truncate(n);
        c.close.truncate(n);
        if let Some(v) = c.volume.as_mut() {
            v.truncate(n);
        }
    }

    /// Runs the inputs `from..` (just added): commits the old last one and
    /// all new ones but the last, peeks the last.
    fn advance(&mut self, index: &mut TimeIndex, from: usize) -> Result<Option<DataChange>> {
        let n = self.input.time.len();
        let Some(last) = (from..n)
            .rev()
            .find(|&i| input_bar(&self.input, i).is_some())
        else {
            return Ok(None);
        };
        self.finished.clear();
        let commits = self
            .last_real
            .into_iter()
            .chain((from..last).filter(|&i| input_bar(&self.input, i).is_some()));
        for i in commits {
            let Some(bar) = input_bar(&self.input, i) else {
                continue;
            };
            self.buf.clear();
            self.plugin.push(bar, &mut self.buf);
            self.buf.check()?;
            self.finished
                .try_reserve(self.buf.as_slice().len())
                .map_err(|_| Error::OutOfMemory)?;
            self.finished
                .extend(self.buf.as_slice().iter().map(|b| (bar.time, *b)));
        }
        let Some(bar) = input_bar(&self.input, last) else {
            return Ok(None);
        };
        self.buf.clear();
        self.plugin.peek(bar, &mut self.buf);
        self.buf.check()?;
        self.last_real = Some(last);
        self.apply_tail(index, bar.time)
    }

    /// Real-time update of the inputs: replaces the last input if
    /// `bar.time` is its time, appends one if later. Returns the change of
    /// the output (`None` if it did not change). Does not allocate when the
    /// last input is replaced and the tail keeps its length.
    pub(crate) fn update_last(
        &mut self,
        index: &mut TimeIndex,
        bar: BarUpdate,
    ) -> Result<Option<DataChange>> {
        if bar.volume.is_some() != self.input.volume.is_some() && !self.input.time.is_empty() {
            return Err(Error::LengthMismatch { column: "volume" });
        }
        let one = || OhlcColumns {
            time: vec![bar.time],
            open: vec![bar.open],
            high: vec![bar.high],
            low: vec![bar.low],
            close: vec![bar.close],
            volume: bar.volume.map(|v| vec![v]),
        };
        let Some(&last_t) = self.input.time.last() else {
            self.set(index, one())?;
            return Ok(Some(DataChange::Replace));
        };
        if bar.time > last_t {
            return self.append(index, one());
        }
        if bar.time < last_t {
            return Err(Error::TimeNotIncreasing { index: 0 });
        }
        let p = self.input.time.len() - 1;
        let gap = bar.open.is_nan();
        if gap || self.last_real != Some(p) {
            // A gap at the end, or the last input was one: start over.
            let mut input = self.input.clone();
            input.open[p] = bar.open;
            input.high[p] = bar.high;
            input.low[p] = bar.low;
            input.close[p] = bar.close;
            if let (Some(v), Some(x)) = (input.volume.as_mut(), bar.volume) {
                v[p] = x;
            }
            self.set(index, input)?;
            return Ok(Some(DataChange::Replace));
        }
        let c = &mut self.input;
        c.open[p] = bar.open;
        c.high[p] = bar.high;
        c.low[p] = bar.low;
        c.close[p] = bar.close;
        if let (Some(v), Some(x)) = (c.volume.as_mut(), bar.volume) {
            v[p] = x;
        }
        let Some(input) = input_bar(&self.input, p) else {
            return Ok(None);
        };
        self.finished.clear();
        self.buf.clear();
        self.plugin.peek(input, &mut self.buf);
        if let Err(e) = self.buf.check() {
            self.clear(index);
            return Err(e);
        }
        self.apply_tail(index, input.time)
    }

    /// Inserts older inputs before the first one and recomputes.
    pub(crate) fn prepend(&mut self, index: &mut TimeIndex, mut new: OhlcColumns) -> Result<()> {
        if new.volume.is_some() != self.input.volume.is_some() && !self.input.time.is_empty() {
            return Err(Error::LengthMismatch { column: "volume" });
        }
        if let (Some(&a), Some(&b)) = (new.time.last(), self.input.time.first())
            && a >= b
        {
            return Err(Error::TimeNotIncreasing {
                index: new.time.len() - 1,
            });
        }
        extend(&mut new.time, &self.input.time)?;
        extend(&mut new.open, &self.input.open)?;
        extend(&mut new.high, &self.input.high)?;
        extend(&mut new.low, &self.input.low)?;
        extend(&mut new.close, &self.input.close)?;
        if let (Some(v), Some(src)) = (new.volume.as_mut(), self.input.volume.as_ref()) {
            extend(v, src)?;
        }
        self.set(index, new)
    }

    /// Drops all output (after an error): nothing is shown until the next
    /// successful `set_data`.
    fn clear(&mut self, index: &mut TimeIndex) {
        let n = self.out.len();
        self.out.truncate_back(n);
        self.flags.clear();
        index.truncate_ends(0, index.len());
        self.input = OhlcColumns::default();
        self.plugin.reset();
        self.committed = 0;
        self.last_time = None;
        self.last_real = None;
    }

    /// Replaces the tail (the elements after the committed ones) with
    /// `self.finished` (new committed elements, with the time of their
    /// input) followed by `self.buf` (the new tail, at `tail_time`). On error
    /// the output is recomputed from the inputs (cleared if that fails too).
    fn apply_tail(
        &mut self,
        index: &mut TimeIndex,
        tail_time: Timestamp,
    ) -> Result<Option<DataChange>> {
        // Moved out (no allocation) so the store can change while they are read.
        let finished = core::mem::take(&mut self.finished);
        let buf = core::mem::take(&mut self.buf);
        let out = self.write_tail(index, &finished, buf.as_slice(), tail_time);
        self.finished = finished;
        self.buf = buf;
        if out.is_err() && self.recompute(index).is_err() {
            self.clear(index);
        }
        out
    }

    fn write_tail(
        &mut self,
        index: &mut TimeIndex,
        finished: &[(Timestamp, TransformBar)],
        tail: &[TransformBar],
        tail_time: Timestamp,
    ) -> Result<Option<DataChange>> {
        let base = self.committed;
        let old_len = self.out.len();
        let old_n = old_len - base;
        let n_new = finished.len() + tail.len();
        let overlap = old_n.min(n_new);
        let first = self.out.first();
        let at = |p: usize| i64::try_from(base + p).unwrap_or(i64::MAX);
        let elems = || {
            finished
                .iter()
                .copied()
                .chain(tail.iter().map(|&b| (tail_time, b)))
        };
        let has_volume = self.out.has_volume();
        let volume_of = |b: &TransformBar| if b.volume.is_nan() { 0.0 } else { b.volume };
        // Times, and the first positions whose time (`td`) or element (`d`)
        // differs.
        self.times.clear();
        self.times
            .try_reserve(n_new)
            .map_err(|_| Error::OutOfMemory)?;
        let mut prev = self.last_time;
        let (mut d, mut td) = (overlap, overlap);
        for (p, (t, b)) in elems().enumerate() {
            let t = prev.map_or(t, |x| t.max(x.saturating_add(1)));
            self.times.push(t);
            prev = Some(t);
            if p >= overlap {
                continue;
            }
            let time_same = index.time_at(index.first_index() + at(p)) == Some(t);
            if !time_same {
                td = td.min(p);
            }
            if p < d {
                let j = first + at(p);
                let same_values = self.out.bar_at(j).is_some_and(|x| {
                    same(x[0], b.open)
                        && same(x[1], b.high)
                        && same(x[2], b.low)
                        && same(x[3], b.close)
                }) && (!has_volume
                    || self
                        .out
                        .volume_at(j)
                        .is_some_and(|v| same(v, volume_of(&b))))
                    && self.flags.get(base + p) == Some(&b.flag);
                if !(same_values && time_same) {
                    d = p;
                }
            }
        }
        // The axis.
        if td < overlap || old_n != n_new {
            index.truncate_ends(0, old_n - td);
            index.reserve_back(n_new - td)?;
            index.append(&self.times[td..])?;
        }
        // The values: in place where both have an element, then truncate or
        // extend.
        for (p, (t, b)) in elems().enumerate().take(overlap).skip(d) {
            let bar = BarUpdate {
                time: t,
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: has_volume.then(|| volume_of(&b)),
            };
            self.out
                .set_at(first + at(p), bar, ValidationMode::Repair)?;
            self.flags[base + p] = b.flag;
        }
        if n_new < old_n {
            self.out.truncate_back(old_n - n_new);
            self.flags.truncate(base + n_new);
        } else if n_new > old_n {
            let rest: Vec<TransformBar> = elems().skip(old_n).map(|e| e.1).collect();
            let cols = self.columns(&rest.iter())?;
            if self.out.len() == 0 {
                self.out
                    .replace_at(index.first_index(), cols, ValidationMode::Repair)?;
            } else {
                self.out.append(None, cols, ValidationMode::Repair)?;
            }
            self.flags
                .try_reserve(rest.len())
                .map_err(|_| Error::OutOfMemory)?;
            self.flags.extend(rest.iter().map(|b| b.flag));
        }
        // Commit.
        let k = finished.len();
        self.committed = base + k;
        if k > 0 {
            self.last_time = Some(self.times[k - 1]);
        }
        Ok(if d == old_n && d == n_new {
            None
        } else if old_len == 0 {
            Some(DataChange::Replace)
        } else if n_new == old_n && d + 1 == n_new {
            Some(DataChange::UpdateLast)
        } else if n_new > old_n && d == old_n {
            Some(DataChange::Append {
                count: n_new - old_n,
            })
        } else {
            Some(DataChange::Replace)
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::transform::{Kagi, PointFigure, RangeBars, Renko, Reversal};
    use proptest::prelude::*;

    fn make(k: u8) -> Box<dyn SeriesTransform> {
        match k % 6 {
            0 => Box::new(Renko::fixed(1.0).unwrap()),
            1 => Box::new(Renko::atr(5).unwrap()),
            2 => Box::new(RangeBars::new(1.5).unwrap()),
            3 => Box::new(Kagi::new(Reversal::Fixed(2.0)).unwrap()),
            4 => Box::new(Kagi::new(Reversal::Percent(3.0)).unwrap()),
            _ => Box::new(PointFigure::new(1.0, 3).unwrap()),
        }
    }

    /// A consumer of the output's changes with the peek/push model of
    /// indicators: running sums of the closes. Only correct if every
    /// change is reported right.
    #[derive(Default)]
    struct Sums {
        committed: f64,
        out: Vec<f64>,
    }

    impl Sums {
        fn close(s: &TransformStore, p: usize) -> f64 {
            s.out
                .bar_at(s.out.first() + p as i64)
                .map_or(f64::NAN, |b| b[3])
        }

        fn apply(&mut self, s: &TransformStore, change: Option<DataChange>) {
            let len = s.out.len();
            match change {
                None => {}
                Some(DataChange::UpdateLast) => {
                    self.out[len - 1] = self.committed + Self::close(s, len - 1);
                }
                Some(DataChange::Append { count }) => {
                    let start = len - count;
                    self.committed += Self::close(s, start - 1);
                    self.out.truncate(start);
                    for p in start..len {
                        self.out.push(self.committed + Self::close(s, p));
                        if p + 1 < len {
                            self.committed += Self::close(s, p);
                        }
                    }
                }
                Some(_) => {
                    *self = Self::default();
                    for p in 0..len {
                        self.out.push(self.committed + Self::close(s, p));
                        if p + 1 < len {
                            self.committed += Self::close(s, p);
                        }
                    }
                }
            }
            assert_eq!(self.out.len(), len);
        }
    }

    fn bar(time: i64, open: f64, close: f64, wick: f64) -> BarUpdate {
        BarUpdate {
            time,
            open,
            high: open.max(close) + wick,
            low: open.min(close) - wick,
            close,
            volume: Some(1.0 + wick),
        }
    }

    fn push_bar(c: &mut OhlcColumns, b: BarUpdate) {
        c.time.push(b.time);
        c.open.push(b.open);
        c.high.push(b.high);
        c.low.push(b.low);
        c.close.push(b.close);
        if let (Some(v), Some(x)) = (c.volume.as_mut(), b.volume) {
            v.push(x);
        }
    }

    fn empty() -> OhlcColumns {
        OhlcColumns {
            volume: Some(Vec::new()),
            ..OhlcColumns::default()
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]
        /// Incremental updates (ticks on the last input, new inputs, batches)
        /// give exactly the output of one full computation, and the changes
        /// they report keep a peek/push consumer exact.
        #[test]
        fn incremental_equals_full(
            k in 0u8..6,
            n0 in 0usize..20,
            steps in proptest::collection::vec((0u8..3, -3.0f64..3.0, 0.0f64..1.5, 1usize..5), 1..60),
        ) {
            let mut all = empty();
            let mut price = 100.0;
            let mut t = 1_000;
            for _ in 0..n0 {
                let b = bar(t, price, price + 0.7, 0.3);
                price += 0.7;
                t += 60;
                push_bar(&mut all, b);
            }
            let (mut index, mut s) = (TimeIndex::new(), TransformStore::new(make(k)));
            s.set(&mut index, all.clone()).unwrap();
            let mut sums = Sums::default();
            sums.apply(&s, Some(DataChange::Replace));
            for (op, mv, wick, batch) in steps {
                let change = match op {
                    // A tick on the last input.
                    0 if !all.time.is_empty() => {
                        let p = all.time.len() - 1;
                        let close = all.close[p] + mv;
                        let b = BarUpdate {
                            time: all.time[p],
                            open: all.open[p],
                            high: all.high[p].max(close),
                            low: all.low[p].min(close),
                            close,
                            volume: Some(all.volume.as_ref().unwrap()[p] + 1.0),
                        };
                        all.open.pop(); all.high.pop(); all.low.pop(); all.close.pop(); all.time.pop();
                        all.volume.as_mut().unwrap().pop();
                        push_bar(&mut all, b);
                        s.update_last(&mut index, b).unwrap()
                    }
                    // A batch of new inputs.
                    2 => {
                        let mut new = empty();
                        for _ in 0..batch {
                            let b = bar(t, price, price + mv, wick);
                            price += mv;
                            t += 60;
                            push_bar(&mut new, b);
                            push_bar(&mut all, b);
                        }
                        s.append(&mut index, new).unwrap()
                    }
                    // A new input.
                    _ => {
                        let b = bar(t, price, price + mv, wick);
                        price += mv;
                        t += 1 + (batch as i64 % 3) * 59;
                        push_bar(&mut all, b);
                        s.update_last(&mut index, b).unwrap()
                    }
                };
                prop_assert_eq!(index.len(), s.out.len());
                prop_assert_eq!(s.flags.len(), s.out.len());
                prop_assert_eq!(index.first_index(), s.out.first());
                sums.apply(&s, change);
            }
            let (mut full_index, mut full) = (TimeIndex::new(), TransformStore::new(make(k)));
            full.set(&mut full_index, all).unwrap();
            prop_assert_eq!(index.times(), full_index.times());
            prop_assert_eq!(&s.flags, &full.flags);
            prop_assert_eq!(s.committed, full.committed);
            let (a, b) = (s.out.first(), full.out.first());
            for p in 0..s.out.len() as i64 {
                let (x, y) = (s.out.bar_at(a + p), full.out.bar_at(b + p));
                prop_assert_eq!(x.map(|v| v.map(f64::to_bits)), y.map(|v| v.map(f64::to_bits)));
                prop_assert_eq!(s.out.volume_at(a + p).map(f64::to_bits), full.out.volume_at(b + p).map(f64::to_bits));
            }
            let mut fresh = Sums::default();
            fresh.apply(&full, Some(DataChange::Replace));
            prop_assert_eq!(
                sums.out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                fresh.out.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn same_tail_length_does_not_touch_the_axis() {
        let mut s = TransformStore::new(make(3));
        let mut index = TimeIndex::new();
        let mut all = empty();
        for (i, p) in [100.0, 103.0, 100.5, 104.0].iter().enumerate() {
            push_bar(&mut all, bar(i as i64 * 60, *p, *p, 0.0));
        }
        s.set(&mut index, all).unwrap();
        let times = index.times().to_vec();
        // The forming Kagi line extends: only the last element changes.
        let change = s
            .update_last(&mut index, bar(180, 104.0, 105.0, 0.0))
            .unwrap();
        assert_eq!(change, Some(DataChange::UpdateLast));
        assert_eq!(index.times(), &times[..]);
        // The same input again: no change.
        let change = s
            .update_last(&mut index, bar(180, 104.0, 105.0, 0.0))
            .unwrap();
        assert_eq!(change, None);
        let change = s
            .update_last(&mut index, bar(180, 104.0, 106.0, 0.0))
            .unwrap();
        assert_eq!(change, Some(DataChange::UpdateLast));
        assert_eq!(index.times(), &times[..]);
    }

    #[test]
    fn errors_clear_or_keep_the_output() {
        let mut s = TransformStore::new(Box::new(Renko::fixed(1.0).unwrap()));
        let mut index = TimeIndex::new();
        let mut all = empty();
        push_bar(&mut all, bar(0, 100.0, 100.0, 0.0));
        push_bar(&mut all, bar(60, 100.0, 103.0, 0.0));
        s.set(&mut index, all).unwrap();
        assert_eq!(s.out.len(), 3);
        // Earlier time: an error, nothing changes.
        assert!(s.update_last(&mut index, bar(0, 1.0, 1.0, 0.0)).is_err());
        assert_eq!(s.out.len(), 3);
        // A jump of more than 65536 bricks: the series is cleared.
        assert!(s.update_last(&mut index, bar(60, 100.0, 1e6, 0.0)).is_err());
        assert_eq!((s.out.len(), index.len()), (0, 0));
        // And works again with new data.
        s.update_last(&mut index, bar(120, 100.0, 100.0, 0.0))
            .unwrap();
        s.update_last(&mut index, bar(180, 100.0, 102.0, 0.0))
            .unwrap();
        assert_eq!((s.out.len(), index.len()), (2, 2));
    }
}
