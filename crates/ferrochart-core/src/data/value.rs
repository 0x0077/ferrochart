//! Storage of single-value series (line, area, baseline, histogram).
//!
//! Values are stored densely on the global time index: element `p` is logical
//! index `first + p`. A series whose times are a sparse subset of the index is
//! densified when it enters the store: missing slots are interpolated linearly
//! for line-like series (which draws exactly the straight segments between the
//! given points, because x is linear in the logical index) and are NaN gaps for
//! histograms.
//!
//! The LOD pyramid is the OHLC pyramid with all four price columns aliased to
//! the value column, so every bucket stores `(first, max, min, last)` in `f32`
//! with directed rounding (16 bytes per bucket). Views
//! use the same aliasing: at level 0, `open == high == low == close == value`.
//! NaN values are gaps; min/max ignore them.
//!

use super::lod::{Lod, Raw};
use super::ohlc::{Column, OhlcLevelView};
use super::{ChangeLog, DeqColumn, IdxSpan};
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug)]
struct ExtremesCache {
    from: i64,
    to: i64,
    generation: u64,
    result: Option<(f64, f64)>,
}

/// Values and optional per-point colors of one series.
#[derive(Clone, Debug, Default)]
pub(crate) struct ValueStore {
    first: i64,
    values: DeqColumn<f64>,
    colors: Option<DeqColumn<u32>>,
    lod: Lod,
    log: ChangeLog,
    extremes_cache: Option<ExtremesCache>,
}

macro_rules! raw {
    ($s:expr) => {
        Raw {
            first: $s.first,
            open: &$s.values,
            high: &$s.values,
            low: &$s.values,
            close: &$s.values,
            volume: None,
        }
    };
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Infinite values are rejected; NaN is a gap.
pub(crate) fn check_values(values: &[f64]) -> Result<()> {
    if values.iter().any(|v| v.is_infinite()) {
        return Err(Error::InvalidArgument(
            "values must be finite (NaN marks a gap)",
        ));
    }
    Ok(())
}

/// Spreads `values` (with optional `colors`) onto consecutive slots: point `i`
/// goes to slot `offsets[i]`. Slots between points are interpolated linearly
/// when `interpolate` (a NaN neighbour makes the slot NaN), NaN otherwise.
/// Colors of filled slots are 0 (transparent).
pub(crate) fn densify(
    offsets: &[u32],
    values: &[f64],
    colors: Option<&[u32]>,
    interpolate: bool,
) -> Result<(Vec<f64>, Option<Vec<u32>>)> {
    let n = offsets.last().map_or(0, |&o| o as usize + 1);
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
    let mut out_colors = match colors {
        Some(_) => {
            let mut c = Vec::new();
            c.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
            Some(c)
        }
        None => None,
    };
    for (i, (&off, &v)) in offsets.iter().zip(values).enumerate() {
        let off = off as usize;
        if let (Some(&prev_off), Some(&prev)) = (
            i.checked_sub(1).and_then(|k| offsets.get(k)),
            i.checked_sub(1).and_then(|k| values.get(k)),
        ) {
            let gap = off - prev_off as usize;
            for s in 1..gap {
                let fill = if interpolate {
                    prev + (v - prev) * (s as f64 / gap as f64)
                } else {
                    f64::NAN
                };
                out.push(fill);
                if let Some(c) = out_colors.as_mut() {
                    c.push(0);
                }
            }
        }
        out.push(v);
        if let (Some(c), Some(src)) = (out_colors.as_mut(), colors) {
            c.push(src.get(i).copied().unwrap_or(0));
        }
    }
    Ok((out, out_colors))
}

impl ValueStore {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn first(&self) -> i64 {
        self.first
    }

    pub fn end(&self) -> i64 {
        self.first + to_i64(self.len())
    }

    pub fn log(&self) -> &ChangeLog {
        &self.log
    }

    pub fn lod(&self) -> &Lod {
        &self.lod
    }

    /// Value at logical index `j`.
    pub fn value_at(&self, j: i64) -> Option<f64> {
        let p = usize::try_from(j.checked_sub(self.first)?).ok()?;
        self.values.get(p).copied()
    }

    /// Level-0 per-point colors, if the series has them.
    pub fn colors(&self) -> Option<&[u32]> {
        self.colors.as_deref()
    }

    /// View of level `level`: open/high/low/close alias the values at level 0
    /// and are `first/max/min/last` at LOD levels.
    pub fn level_view(&self, level: u8) -> Option<OhlcLevelView<'_>> {
        if level == 0 {
            let v = Column::F64(&self.values);
            return Some(OhlcLevelView {
                level,
                first: self.first,
                open: v,
                high: v,
                low: v,
                close: v,
                volume: None,
            });
        }
        let l = self.lod.level(level)?;
        Some(OhlcLevelView {
            level,
            first: l.first,
            open: Column::F32(&l.open),
            high: Column::F32(&l.high),
            low: Column::F32(&l.low),
            close: Column::F32(&l.close),
            volume: None,
        })
    }

    /// Replaces all data: `values[p]` goes to logical index `first + p`.
    pub fn replace(
        &mut self,
        first: i64,
        values: Vec<f64>,
        colors: Option<Vec<u32>>,
    ) -> Result<()> {
        check_values(&values)?;
        if colors.as_ref().is_some_and(|c| c.len() != values.len()) {
            return Err(Error::LengthMismatch { column: "color" });
        }
        let mut next = Self {
            first,
            values: DeqColumn::from_vec(values),
            colors: colors.map(DeqColumn::from_vec),
            lod: Lod::default(),
            log: core::mem::take(&mut self.log),
            extremes_cache: None,
        };
        next.lod = match Lod::build(&raw!(next)) {
            Ok(lod) => lod,
            Err(e) => {
                self.log = next.log;
                return Err(e);
            }
        };
        next.values.shrink_to_fit();
        if let Some(c) = next.colors.as_mut() {
            c.shrink_to_fit();
        }
        next.log.replace();
        *self = next;
        Ok(())
    }

    fn check_colors(&self, colors: Option<&[u32]>, n: usize) -> Result<()> {
        match (self.colors.is_some() || self.len() == 0, colors) {
            (_, Some(c)) if c.len() != n => Err(Error::LengthMismatch { column: "color" }),
            (true, _) | (false, None) => Ok(()),
            (false, Some(_)) => Err(Error::LengthMismatch { column: "color" }),
        }
    }

    /// Appends values right after the last one (`first + len` onwards).
    pub fn append(&mut self, values: &[f64], colors: Option<&[u32]>) -> Result<()> {
        if self.len() == 0 {
            return self.replace(self.first, values.to_vec(), colors.map(<[u32]>::to_vec));
        }
        check_values(values)?;
        self.check_colors(colors, values.len())?;
        let m = values.len();
        let new_end = self.end().saturating_add(to_i64(m));
        self.values.reserve_back(m)?;
        if let Some(c) = self.colors.as_mut() {
            c.reserve_back(m)?;
        }
        self.lod.reserve(self.first, new_end)?;
        self.values.extend_back(values)?;
        if let Some(c) = self.colors.as_mut() {
            match colors {
                Some(src) => c.extend_back(src)?,
                None => c.grow_back(m, 0)?,
            }
        }
        let span = IdxSpan {
            start: new_end - to_i64(m),
            end: new_end,
        };
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(())
    }

    /// Inserts values right before the first one.
    pub fn prepend(&mut self, values: &[f64], colors: Option<&[u32]>) -> Result<()> {
        if self.len() == 0 {
            let first = self.first - to_i64(values.len());
            return self.replace(first, values.to_vec(), colors.map(<[u32]>::to_vec));
        }
        check_values(values)?;
        self.check_colors(colors, values.len())?;
        let m = values.len();
        let new_first = self.first.saturating_sub(to_i64(m));
        self.values.reserve_front(m)?;
        if let Some(c) = self.colors.as_mut() {
            c.reserve_front(m)?;
        }
        self.lod.reserve(new_first, self.end())?;
        self.values.extend_front(values)?;
        if let Some(c) = self.colors.as_mut() {
            match colors {
                Some(src) => c.extend_front(src)?,
                None => c.grow_front(m, 0)?,
            }
        }
        let span = IdxSpan {
            start: new_first,
            end: self.first,
        };
        self.first = new_first;
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(())
    }

    /// Overwrites the last value in place (no allocation).
    pub fn set_last(&mut self, value: f64, color: Option<u32>) -> Result<()> {
        check_values(&[value])?;
        let Some(p) = self.len().checked_sub(1) else {
            return Err(Error::InvalidArgument("the series has no data"));
        };
        self.values[p] = value;
        if let (Some(c), Some(x)) = (self.colors.as_mut(), color) {
            c[p] = x;
        }
        let span = IdxSpan {
            start: self.end() - 1,
            end: self.end(),
        };
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(())
    }

    /// Minimum and maximum over logical indices `from..to`, ignoring gaps;
    /// cached by `(range, generation)`.
    pub fn extremes(&mut self, from: i64, to: i64) -> Option<(f64, f64)> {
        let generation = self.log.generation();
        if let Some(c) = self.extremes_cache
            && (c.from, c.to, c.generation) == (from, to, generation)
        {
            return c.result;
        }
        let result = self
            .lod
            .extremes(&raw!(self), from, to)
            .filter(|(lo, hi)| lo.is_finite() && hi.is_finite());
        self.extremes_cache = Some(ExtremesCache {
            from,
            to,
            generation,
            result,
        });
        result
    }

    /// Heap bytes held by the series (values, colors and LOD).
    pub fn heap_bytes(&self) -> usize {
        self.len() * core::mem::size_of::<f64>()
            + self.colors.as_ref().map_or(0, |c| c.len() * 4)
            + self.lod.heap_bytes()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact values are the point of these tests
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn densify_interpolates_or_leaves_gaps() {
        let (v, _) = densify(&[0, 2, 3], &[1.0, 3.0, 9.0], None, true).unwrap();
        assert_eq!(v, [1.0, 2.0, 3.0, 9.0]);
        let (v, c) = densify(&[0, 2], &[1.0, 3.0], Some(&[7, 8]), false).unwrap();
        assert_eq!(v[0], 1.0);
        assert!(v[1].is_nan());
        assert_eq!(c.unwrap(), [7, 0, 8]);
    }

    #[test]
    fn lod_buckets_are_first_max_min_last() {
        let mut s = ValueStore::default();
        // Logical indices 0..8 lie in two level-2 buckets (ANCHOR is a multiple of 4).
        s.replace(0, vec![5.0, 9.0, 1.0, 4.0, 2.0, f64::NAN, 3.0, 6.0], None)
            .unwrap();
        let v = s.level_view(2).unwrap();
        let b = |j| [v.open, v.high, v.low, v.close].map(|c| c.at(v.first, j).unwrap());
        let j0 = v.first;
        assert_eq!(b(j0), [5.0, 9.0, 1.0, 4.0]);
        assert_eq!(b(j0 + 1), [2.0, 6.0, 2.0, 6.0]);
        assert_eq!(s.extremes(0, 8), Some((1.0, 9.0)));
        assert_eq!(s.extremes(5, 6), None); // only a gap
    }

    #[test]
    fn colors_must_match() {
        let mut s = ValueStore::default();
        s.replace(0, vec![1.0, 2.0], Some(vec![1, 2])).unwrap();
        assert!(s.append(&[3.0], Some(&[1, 2])).is_err());
        s.append(&[3.0], None).unwrap();
        assert_eq!(s.colors().unwrap(), [1, 2, 0]);
        assert!(s.append(&[f64::INFINITY], None).is_err());
    }

    proptest! {
        /// Incremental appends, prepends and last-value updates give the same
        /// LOD as a full rebuild.
        #[test]
        fn incremental_equals_full(
            base in proptest::collection::vec(-1e3f64..1e3, 1..300),
            ops in proptest::collection::vec((0u8..3, proptest::collection::vec(-1e3f64..1e3, 1..40)), 0..10),
        ) {
            let mut s = ValueStore::default();
            s.replace(1000, base.clone(), None).unwrap();
            let mut all = base;
            let mut first = 1000;
            for (op, vals) in ops {
                match op {
                    0 => { s.append(&vals, None).unwrap(); all.extend_from_slice(&vals); }
                    1 => {
                        s.prepend(&vals, None).unwrap();
                        first -= vals.len() as i64;
                        let mut v = vals.clone();
                        v.extend_from_slice(&all);
                        all = v;
                    }
                    _ => {
                        s.set_last(vals[0], None).unwrap();
                        if let Some(l) = all.last_mut() { *l = vals[0]; }
                    }
                }
            }
            let mut full = ValueStore::default();
            full.replace(first, all, None).unwrap();
            prop_assert_eq!(s.first(), full.first());
            for level in [0u8, 2, 3, 4, 5, 6] {
                let (a, b) = (s.level_view(level), full.level_view(level));
                prop_assert_eq!(a.is_some(), b.is_some());
                if let (Some(a), Some(b)) = (a, b) {
                    prop_assert_eq!(a.first, b.first);
                    prop_assert_eq!(a.len(), b.len());
                    for p in 0..a.len() {
                        for (x, y) in [(a.open, b.open), (a.high, b.high), (a.low, b.low), (a.close, b.close)] {
                            prop_assert_eq!(x.get(p).map(f64::to_bits), y.get(p).map(f64::to_bits));
                        }
                    }
                }
            }
        }
    }
}
