//! LOD pyramid for OHLC(V) series and the visible-range extremes query.
//!
//! Level `k >= 2` merges `2^k` bars into one bucket, stored as `f32` with directed
//! rounding (high up, low down, the rest to nearest), so the stored `[low, high]`
//! always contains the true range. Buckets are aligned to the fixed logical index
//! origin [`ANCHOR`], not to array positions, so `Prepend` never moves existing
//! buckets. Level 1 is not built.
//!
//! Every bucket of level `k > 2` is derived from its two children at level `k - 1`
//! (level 2 from the raw `f64` bars), both in the full build and in incremental
//! updates, so the two always produce bit-identical results.
//!

use super::{DeqColumn, IdxSpan};
use crate::error::{Error, Result};

/// Fixed logical index origin for LOD buckets and GPU chunks. Logical indices must stay
/// above it.
pub const ANCHOR: i64 = -(1 << 40);

/// Lowest LOD level that is built (level 1 is skipped).
pub const MIN_LOD_LEVEL: u8 = 2;

/// Bucket number of logical index `i` at level `k`.
pub(crate) fn bucket_of(i: i64, k: u32) -> i64 {
    (i - ANCHOR) >> k
}

/// First logical index of bucket `b` at level `k`.
pub(crate) fn bucket_start(b: i64, k: u32) -> i64 {
    ANCHOR + (b << k)
}

/// `f64 -> f32`, rounded towards +∞.
pub(crate) fn f32_up(x: f64) -> f32 {
    let f = x as f32;
    if f64::from(f) < x { f.next_up() } else { f }
}

/// `f64 -> f32`, rounded towards -∞.
pub(crate) fn f32_down(x: f64) -> f32 {
    let f = x as f32;
    if f64::from(f) > x { f.next_down() } else { f }
}

/// Converts a non-negative `i64` position to `usize` (0 if negative).
fn pos(i: i64) -> usize {
    usize::try_from(i).unwrap_or(0)
}

/// Borrowed level-0 data of a series.
#[derive(Clone, Copy)]
pub(crate) struct Raw<'a> {
    pub first: i64,
    pub open: &'a [f64],
    pub high: &'a [f64],
    pub low: &'a [f64],
    pub close: &'a [f64],
    pub volume: Option<&'a [f64]>,
}

impl Raw<'_> {
    pub fn end(&self) -> i64 {
        self.first + i64::try_from(self.open.len()).unwrap_or(i64::MAX)
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Bucket {
    open: f32,
    high: f32,
    low: f32,
    close: f32,
    volume: f32,
}

/// One LOD level: buckets `first..first + len` in `f32` columns.
#[derive(Clone, Debug)]
pub(crate) struct LodLevel {
    pub first: i64,
    pub open: DeqColumn<f32>,
    pub high: DeqColumn<f32>,
    pub low: DeqColumn<f32>,
    pub close: DeqColumn<f32>,
    pub volume: Option<DeqColumn<f32>>,
}

impl LodLevel {
    pub fn len(&self) -> usize {
        self.open.len()
    }

    pub fn end(&self) -> i64 {
        self.first + i64::try_from(self.len()).unwrap_or(i64::MAX)
    }

    fn contains(&self, b: i64) -> bool {
        b >= self.first && b < self.end()
    }

    fn get(&self, b: i64) -> Bucket {
        let p = pos(b - self.first);
        Bucket {
            open: self.open[p],
            high: self.high[p],
            low: self.low[p],
            close: self.close[p],
            volume: self.volume.as_ref().map_or(0.0, |v| v[p]),
        }
    }

    fn set(&mut self, b: i64, x: Bucket) {
        let p = pos(b - self.first);
        self.open[p] = x.open;
        self.high[p] = x.high;
        self.low[p] = x.low;
        self.close[p] = x.close;
        if let Some(v) = self.volume.as_mut() {
            v[p] = x.volume;
        }
    }

    /// Builds buckets `first..end` with `f`, allocating exactly.
    fn build(first: i64, end: i64, has_volume: bool, f: impl Fn(i64) -> Bucket) -> Result<Self> {
        let n = pos(end - first);
        let mut cols: [Vec<f32>; 5] = Default::default();
        for c in &mut cols {
            c.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
        }
        for b in first..end {
            let x = f(b);
            cols[0].push(x.open);
            cols[1].push(x.high);
            cols[2].push(x.low);
            cols[3].push(x.close);
            if has_volume {
                cols[4].push(x.volume);
            }
        }
        let [o, h, l, c, v] = cols;
        Ok(Self {
            first,
            open: DeqColumn::from_vec(o),
            high: DeqColumn::from_vec(h),
            low: DeqColumn::from_vec(l),
            close: DeqColumn::from_vec(c),
            volume: has_volume.then(|| DeqColumn::from_vec(v)),
        })
    }

    fn columns_mut(&mut self) -> impl Iterator<Item = &mut DeqColumn<f32>> {
        [
            Some(&mut self.open),
            Some(&mut self.high),
            Some(&mut self.low),
            Some(&mut self.close),
            self.volume.as_mut(),
        ]
        .into_iter()
        .flatten()
    }

    fn reserve(&mut self, front: usize, back: usize) -> Result<()> {
        for c in self.columns_mut() {
            c.reserve_front(front)?;
            c.reserve_back(back)?;
        }
        Ok(())
    }

    /// Extends the level with zero buckets to cover `first..end`. Infallible after
    /// a matching [`reserve`](Self::reserve).
    fn cover(&mut self, first: i64, end: i64) {
        let front = pos(self.first - first);
        let back = pos(end - self.end());
        for c in self.columns_mut() {
            // Space was reserved, so these do not allocate and cannot fail.
            let _ = c.grow_front(front, 0.0);
            let _ = c.grow_back(back, 0.0);
        }
        self.first = self.first.min(first);
    }

    fn heap_bytes(&self) -> usize {
        let cols = 4 + usize::from(self.volume.is_some());
        cols * self.len() * core::mem::size_of::<f32>()
    }
}

/// Bucket `b` of level 2, computed from the raw bars.
fn raw_bucket(raw: &Raw<'_>, b: i64) -> Bucket {
    let lo = bucket_start(b, 2).max(raw.first);
    let hi = bucket_start(b + 1, 2).min(raw.end());
    let (s, e) = (pos(lo - raw.first), pos(hi - raw.first));
    let mut high = f64::NEG_INFINITY;
    let mut low = f64::INFINITY;
    let mut volume = 0.0;
    // Gaps (NaN) are skipped: `max`/`min` ignore NaN, and open/close are the
    // first and last finite ones (NaN if the bucket is all gaps).
    let mut open = f64::NAN;
    let mut close = f64::NAN;
    for p in s..e {
        high = high.max(raw.high[p]);
        low = low.min(raw.low[p]);
        if open.is_nan() {
            open = raw.open[p];
        }
        if !raw.close[p].is_nan() {
            close = raw.close[p];
        }
        if let Some(v) = raw.volume
            && !v[p].is_nan()
        {
            volume += v[p];
        }
    }
    Bucket {
        open: open as f32,
        high: f32_up(high),
        low: f32_down(low),
        close: close as f32,
        volume: volume as f32,
    }
}

/// Bucket `b` of level `k + 1`, computed from its children at level `k`.
fn child_bucket(child: &LodLevel, b: i64) -> Bucket {
    let (c0, c1) = (2 * b, 2 * b + 1);
    match (child.contains(c0), child.contains(c1)) {
        (true, true) => {
            let (x, y) = (child.get(c0), child.get(c1));
            Bucket {
                open: if x.open.is_nan() { y.open } else { x.open },
                high: x.high.max(y.high),
                low: x.low.min(y.low),
                close: if y.close.is_nan() { x.close } else { y.close },
                volume: (f64::from(x.volume) + f64::from(y.volume)) as f32,
            }
        }
        (true, false) => child.get(c0),
        (false, true) => child.get(c1),
        (false, false) => Bucket::default(),
    }
}

/// The LOD pyramid of one OHLC series. `levels[i]` is level `i + 2`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Lod {
    levels: Vec<LodLevel>,
}

impl Lod {
    /// Full build from the raw data. O(n).
    pub fn build(raw: &Raw<'_>) -> Result<Self> {
        let mut lod = Self::default();
        if raw.open.is_empty() {
            return Ok(lod);
        }
        let has_volume = raw.volume.is_some();
        let first = bucket_of(raw.first, 2);
        let end = bucket_of(raw.end() - 1, 2) + 1;
        lod.levels.try_reserve(24).map_err(|_| Error::OutOfMemory)?;
        lod.levels
            .push(LodLevel::build(first, end, has_volume, |b| {
                raw_bucket(raw, b)
            })?);
        while lod.add_level_if_needed(raw.end() - 1)? {}
        Ok(lod)
    }

    /// Number of built levels; the highest level is `MIN_LOD_LEVEL + count - 1`.
    #[cfg(test)]
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Level `k` (`k >= 2`), if built.
    pub fn level(&self, k: u8) -> Option<&LodLevel> {
        let i = usize::from(k.checked_sub(MIN_LOD_LEVEL)?);
        self.levels.get(i)
    }

    /// Highest built level, or `None` if there are no levels.
    pub fn max_level(&self) -> Option<u8> {
        let n = u8::try_from(self.levels.len()).ok()?;
        n.checked_sub(1).map(|top| top + MIN_LOD_LEVEL)
    }

    /// Adds one level above the current top if the top still has more than two
    /// buckets. Returns whether a level was added.
    fn add_level_if_needed(&mut self, last_index: i64) -> Result<bool> {
        let Some(top) = self.levels.last() else {
            return Ok(false);
        };
        if top.len() <= 2 {
            return Ok(false);
        }
        let k = u32::try_from(self.levels.len()).unwrap_or(u32::MAX) + 2; // new level
        let first = top.first >> 1;
        let end = bucket_of(last_index, k) + 1;
        let has_volume = top.volume.is_some();
        let level = LodLevel::build(first, end, has_volume, |b| child_bucket(top, b))?;
        self.levels.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.levels.push(level);
        Ok(true)
    }

    /// Reserves room so that [`update`](Self::update) for data covering
    /// `new_first..new_end` cannot fail.
    pub fn reserve(&mut self, new_first: i64, new_end: i64) -> Result<()> {
        for (i, level) in self.levels.iter_mut().enumerate() {
            let k = u32::try_from(i).unwrap_or(u32::MAX) + 2;
            let first = bucket_of(new_first, k);
            let end = bucket_of(new_end - 1, k) + 1;
            level.reserve(pos(level.first - first), pos(end - level.end()))?;
        }
        Ok(())
    }

    /// Updates the pyramid after the slots in `changed` were written. `raw` is the
    /// data after the change and may extend beyond the previous data on either
    /// side (append/prepend). O(`changed.len()` + log n).
    ///
    /// Must be preceded by a successful [`reserve`](Self::reserve) for the new
    /// extent; the update itself then cannot fail. Adding a new top level may
    /// allocate; if that fails the pyramid simply keeps fewer levels.
    pub fn update(&mut self, raw: &Raw<'_>, changed: IdxSpan) {
        if raw.open.is_empty() || changed.is_empty() {
            return;
        }
        if self.levels.is_empty() {
            if let Ok(lod) = Self::build(raw) {
                *self = lod;
            }
            return;
        }
        let last = raw.end() - 1;
        for i in 0..self.levels.len() {
            let k = u32::try_from(i).unwrap_or(u32::MAX) + 2;
            let (below, rest) = self.levels.split_at_mut(i);
            let level = &mut rest[0];
            level.cover(bucket_of(raw.first, k), bucket_of(last, k) + 1);
            let from = bucket_of(changed.start.max(raw.first), k);
            let to = bucket_of((changed.end - 1).min(last), k) + 1;
            for b in from..to {
                let x = match below.last() {
                    None => raw_bucket(raw, b),
                    Some(child) => child_bucket(child, b),
                };
                level.set(b, x);
            }
        }
        while let Ok(true) = self.add_level_if_needed(last) {}
    }

    /// Drops the buckets past the data after bars were removed from its end
    /// (`raw` is the data after the removal) and recomputes the last bucket of
    /// every level. Keeps the capacity; never allocates.
    pub fn truncate(&mut self, raw: &Raw<'_>) {
        if raw.open.is_empty() {
            self.levels.clear();
            return;
        }
        let last = raw.end() - 1;
        for i in 0..self.levels.len() {
            let k = u32::try_from(i).unwrap_or(u32::MAX) + 2;
            let (below, rest) = self.levels.split_at_mut(i);
            let level = &mut rest[0];
            let b = bucket_of(last, k);
            let extra = pos(level.end() - (b + 1));
            for c in level.columns_mut() {
                c.truncate_ends(0, extra);
            }
            if level.contains(b) {
                let x = match below.last() {
                    None => raw_bucket(raw, b),
                    Some(child) => child_bucket(child, b),
                };
                level.set(b, x);
            }
        }
    }

    /// Minimum low and maximum high over logical indices `from..to` (clamped to the
    /// data), or `None` if that range holds no data. O(log n): raw bars at both
    /// ends, the largest aligned buckets in the middle. The result always contains
    /// the true extremes (directed rounding).
    pub fn extremes(&self, raw: &Raw<'_>, from: i64, to: i64) -> Option<(f64, f64)> {
        let a = from.max(raw.first);
        let b = to.min(raw.end());
        if a >= b {
            return None;
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut scan = |s: i64, e: i64| {
            for p in pos(s - raw.first)..pos(e - raw.first) {
                lo = lo.min(raw.low[p]);
                hi = hi.max(raw.high[p]);
            }
        };
        let a2 = bucket_start(bucket_of(a + 3, 2), 2); // align up to a level-2 boundary
        let b2 = bucket_start(bucket_of(b, 2), 2); // align down
        if self.levels.is_empty() || a2 >= b2 {
            scan(a, b);
            return Some((lo, hi));
        }
        scan(a, a2);
        scan(b2, b);

        let mut take = |level: &LodLevel, x: i64| {
            let bucket = level.get(x);
            lo = lo.min(f64::from(bucket.low));
            hi = hi.max(f64::from(bucket.high));
        };
        let (mut l, mut r) = (bucket_of(a2, 2), bucket_of(b2, 2));
        for (i, level) in self.levels.iter().enumerate() {
            if i + 1 == self.levels.len() {
                for x in l..r {
                    take(level, x);
                }
                break;
            }
            if l & 1 == 1 {
                take(level, l);
                l += 1;
            }
            if r & 1 == 1 {
                r -= 1;
                take(level, r);
            }
            if l >= r {
                break;
            }
            l >>= 1;
            r >>= 1;
        }
        Some((lo, hi))
    }

    /// Heap bytes held by the level columns.
    pub fn heap_bytes(&self) -> usize {
        self.levels.iter().map(LodLevel::heap_bytes).sum()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact values are the point of these tests
mod tests {
    use super::*;
    use proptest::prelude::*;

    struct Data {
        first: i64,
        o: Vec<f64>,
        h: Vec<f64>,
        l: Vec<f64>,
        c: Vec<f64>,
        v: Vec<f64>,
    }

    impl Data {
        fn raw(&self) -> Raw<'_> {
            Raw {
                first: self.first,
                open: &self.o,
                high: &self.h,
                low: &self.l,
                close: &self.c,
                volume: Some(&self.v),
            }
        }
    }

    fn data(first: i64, prices: &[(f64, f64)]) -> Data {
        // (center, half-range) -> bar
        let mut d = Data {
            first,
            o: vec![],
            h: vec![],
            l: vec![],
            c: vec![],
            v: vec![],
        };
        for &(m, r) in prices {
            d.o.push(m - r / 3.0);
            d.c.push(m + r / 4.0);
            d.h.push(m + r);
            d.l.push(m - r);
            d.v.push(r + 0.5);
        }
        d
    }

    fn bars() -> impl Strategy<Value = (i64, Vec<(f64, f64)>)> {
        (
            -300i64..300,
            prop::collection::vec((-1e6f64..1e6, 0.0f64..1e3), 0..600),
        )
    }

    /// Checks every bucket of every level against the raw data.
    fn check_pyramid(lod: &Lod, d: &Data) -> std::result::Result<(), TestCaseError> {
        let raw = d.raw();
        for (i, level) in lod.levels.iter().enumerate() {
            let k = u32::try_from(i).unwrap() + 2;
            prop_assert_eq!(level.first, bucket_of(raw.first, k));
            prop_assert_eq!(level.end(), bucket_of(raw.end() - 1, k) + 1);
            for b in level.first..level.end() {
                let s = pos(bucket_start(b, k).max(raw.first) - raw.first);
                let e = pos(bucket_start(b + 1, k).min(raw.end()) - raw.first);
                let max = d.h[s..e].iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let min = d.l[s..e].iter().copied().fold(f64::INFINITY, f64::min);
                let x = level.get(b);
                prop_assert!(f64::from(x.high) >= max && f64::from(x.high.next_down()) < max);
                prop_assert!(f64::from(x.low) <= min && f64::from(x.low.next_up()) > min);
                prop_assert_eq!(x.open, d.o[s] as f32);
                prop_assert_eq!(x.close, d.c[e - 1] as f32);
                prop_assert!(x.low <= x.open.min(x.close) && x.open.max(x.close) <= x.high);
            }
        }
        if let Some(top) = lod.levels.last() {
            prop_assert!(top.len() <= 2);
        }
        Ok(())
    }

    #[test]
    fn directed_rounding() {
        let x = 0.1_f64; // not representable in f32
        assert!(f64::from(f32_up(x)) > x && f64::from(f32_down(x)) < x);
        assert_eq!(f32_up(0.5), 0.5);
        assert_eq!(f32_down(0.5), 0.5);
        assert_eq!(f32_up(f64::MAX), f32::INFINITY);
    }

    #[test]
    fn tiny_series_has_one_level() {
        let d = data(0, &[(10.0, 1.0)]);
        let lod = Lod::build(&d.raw()).unwrap();
        assert_eq!(lod.level_count(), 1);
        assert_eq!(lod.max_level(), Some(2));
        assert_eq!(lod.extremes(&d.raw(), -5, 5), Some((9.0, 11.0)));
        assert_eq!(lod.extremes(&d.raw(), 1, 5), None);
    }

    proptest! {
        #[test]
        fn full_build_is_correct((first, prices) in bars()) {
            let d = data(first, &prices);
            let lod = Lod::build(&d.raw()).unwrap();
            check_pyramid(&lod, &d)?;
        }

        /// Incremental appends, updates and prepends give exactly the full build.
        #[test]
        fn incremental_equals_full(
            (first, mut prices) in bars(),
            steps in prop::collection::vec((0u8..3, 1usize..40), 0..30),
        ) {
            let n = prices.len();
            if n == 0 { return Ok(()); }
            // Start from a middle slice, then grow it on both sides.
            let mut lo = n / 2;
            let mut hi = lo + 1;
            let mut cur = data(first + i64::try_from(lo).unwrap(), &prices[lo..hi]);
            let mut lod = Lod::build(&cur.raw()).unwrap();
            for (kind, count) in steps {
                let old = (lo, hi);
                match kind {
                    0 => hi = (hi + count).min(n),
                    1 => lo = lo.saturating_sub(count),
                    _ => {}
                }
                if kind == 2 {
                    // update_last: move the last bar (persistently).
                    prices[hi - 1].0 += 5.0;
                }
                let next = data(first + i64::try_from(lo).unwrap(), &prices[lo..hi]);
                let to_idx = |p: usize| first + i64::try_from(p).unwrap();
                let changed = match kind {
                    0 => IdxSpan { start: to_idx(old.1 - 1), end: to_idx(hi) },
                    1 => IdxSpan { start: to_idx(lo), end: to_idx(old.0) },
                    _ => IdxSpan { start: to_idx(hi - 1), end: to_idx(hi) },
                };
                let raw = next.raw();
                lod.reserve(raw.first, raw.end()).unwrap();
                lod.update(&raw, changed);
                cur = next;
                let full = Lod::build(&cur.raw()).unwrap();
                prop_assert_eq!(lod.levels.len(), full.levels.len());
                for (a, b) in lod.levels.iter().zip(&full.levels) {
                    prop_assert_eq!(a.first, b.first);
                    prop_assert_eq!(&a.high[..], &b.high[..]);
                    prop_assert_eq!(&a.low[..], &b.low[..]);
                    prop_assert_eq!(&a.open[..], &b.open[..]);
                    prop_assert_eq!(&a.close[..], &b.close[..]);
                    prop_assert_eq!(a.volume.as_deref(), b.volume.as_deref());
                }
            }
            check_pyramid(&lod, &cur)?;
        }

        /// The extremes query always contains the true extremes and is at most one
        /// f32 ulp away from them.
        #[test]
        fn extremes_contain_truth((first, prices) in bars(), a in -400i64..1000, len in 0i64..1000) {
            let d = data(first, &prices);
            let lod = Lod::build(&d.raw()).unwrap();
            let raw = d.raw();
            let (s, e) = (a.max(raw.first), (a + len).min(raw.end()));
            let got = lod.extremes(&raw, a, a + len);
            if s >= e {
                prop_assert_eq!(got, None);
            } else {
                let (lo, hi) = got.unwrap();
                let (ps, pe) = (pos(s - raw.first), pos(e - raw.first));
                let max = d.h[ps..pe].iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let min = d.l[ps..pe].iter().copied().fold(f64::INFINITY, f64::min);
                prop_assert!(hi >= max && lo <= min);
                prop_assert!(hi <= f64::from(f32_up(max)) && lo >= f64::from(f32_down(min)));
            }
        }

        /// Prepending never changes existing buckets except the shared boundary one.
        #[test]
        fn prepend_keeps_old_buckets((first, prices) in bars(), cut in 0usize..600) {
            let n = prices.len();
            let cut = cut.min(n);
            if cut == n { return Ok(()); }
            let old = data(first + i64::try_from(cut).unwrap(), &prices[cut..]);
            let new = data(first, &prices);
            let before = Lod::build(&old.raw()).unwrap();
            let after = Lod::build(&new.raw()).unwrap();
            for (i, level) in before.levels.iter().enumerate() {
                let k = u32::try_from(i).unwrap() + 2;
                let boundary = bucket_of(old.first, k);
                for b in (boundary + 1)..level.end() {
                    prop_assert_eq!(level.get(b), after.levels[i].get(b));
                }
            }
        }
    }
}
