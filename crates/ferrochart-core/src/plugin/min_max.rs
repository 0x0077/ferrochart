//! Range extremes for plugins.
//!
//! [`crate::plugin::CustomSeries::extremes`] must be O(log n), because auto
//! scaling queries the visible range every frame. `MinMaxIndex` gives plugins
//! the same structure the built-in series use: level 0 keeps the exact f64
//! values, higher levels keep `(min, max)` of blocks of `2^k` points as f32
//! rounded outward, so a query covers the true extremes.

use crate::data::lod::{f32_down, f32_up};
use crate::error::{Error, Result};

/// `(min, max)` pairs of a growing column with O(log n) range queries,
/// O(log n) `push` and `set_last`.
///
/// Pairs with a NaN bound are empty (a gap) and never contribute.
///
/// # Examples
///
/// ```
/// use ferrochart_core::plugin::MinMaxIndex;
///
/// let mut idx = MinMaxIndex::new();
/// for (lo, hi) in [(1.0, 2.0), (0.5, 3.0), (f64::NAN, f64::NAN), (2.0, 4.0)] {
///     idx.push(lo, hi)?;
/// }
/// assert_eq!(idx.query(0, 2), Some((0.5, 3.0)));
/// idx.set_last(-1.0, 0.0);
/// let (lo, hi) = idx.query(0, 4).unwrap();
/// assert!(lo <= -1.0 && hi >= 3.0);
/// assert_eq!(idx.query(2, 3), None);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct MinMaxIndex {
    /// Exact pairs.
    base: Vec<(f64, f64)>,
    /// `levels[k - 1][i]`: block `i` of `2^k` points, outward-rounded.
    levels: Vec<Vec<(f32, f32)>>,
}

const EMPTY32: (f32, f32) = (f32::INFINITY, f32::NEG_INFINITY);

fn widen(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0.min(b.0), a.1.max(b.1))
}

fn round(p: (f64, f64)) -> (f32, f32) {
    if p.0.is_nan() || p.1.is_nan() {
        EMPTY32
    } else {
        (f32_down(p.0), f32_up(p.1))
    }
}

impl MinMaxIndex {
    /// An empty index.
    pub const fn new() -> Self {
        Self {
            base: Vec::new(),
            levels: Vec::new(),
        }
    }

    /// Number of pairs.
    pub fn len(&self) -> usize {
        self.base.len()
    }

    /// Whether there are no pairs.
    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }

    /// Removes all pairs, keeping the memory.
    pub fn clear(&mut self) {
        self.base.clear();
        for l in &mut self.levels {
            l.clear();
        }
    }

    /// Bytes of heap memory in use.
    pub fn heap_bytes(&self) -> usize {
        self.base.capacity() * 16 + self.levels.iter().map(|l| l.capacity() * 8).sum::<usize>()
    }

    /// Appends a pair (`min`, `max`); NaN in either makes it a gap.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if memory cannot be reserved.
    pub fn push(&mut self, min: f64, max: f64) -> Result<()> {
        self.base.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.base.push((min, max));
        let n = self.base.len();
        let mut k = 1;
        while n > (1 << (k - 1)) {
            if self.levels.len() < k {
                self.levels.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
                self.levels.push(Vec::new());
            }
            let level = &mut self.levels[k - 1];
            let want = (n - 1) / (1 << k) + 1;
            if level.len() < want {
                level.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
                level.push(EMPTY32);
            }
            k += 1;
        }
        self.refresh_last();
        Ok(())
    }

    /// Replaces the last pair (real-time updates); does nothing when empty.
    /// Does not allocate.
    pub fn set_last(&mut self, min: f64, max: f64) {
        if let Some(last) = self.base.last_mut() {
            *last = (min, max);
            self.refresh_last();
        }
    }

    /// Recomputes the blocks containing the last point from their children.
    fn refresh_last(&mut self) {
        let Some(last) = self.base.len().checked_sub(1) else {
            return;
        };
        for k in 1..=self.levels.len() {
            let i = last >> k;
            let child = |j: usize| -> (f32, f32) {
                if k == 1 {
                    self.base.get(j).copied().map_or(EMPTY32, round)
                } else {
                    self.levels[k - 2].get(j).copied().unwrap_or(EMPTY32)
                }
            };
            let v = widen(child(2 * i), child(2 * i + 1));
            if let Some(slot) = self.levels[k - 1].get_mut(i) {
                *slot = v;
            }
        }
    }

    /// Minimum and maximum over pairs `from..to` (clamped to the data);
    /// `None` if they are all gaps. The interior of the range comes from the
    /// f32 levels, so the result may be wider than the exact extremes by one
    /// f32 ulp, never narrower.
    pub fn query(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        let (mut a, mut b) = (from, to.min(self.base.len()));
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut take = |p: (f64, f64)| {
            if !(p.0.is_nan() || p.1.is_nan()) {
                lo = lo.min(p.0);
                hi = hi.max(p.1);
            }
        };
        let mut k = 0;
        while a < b {
            let at = |i: usize| -> (f64, f64) {
                if k == 0 {
                    self.base[i]
                } else {
                    let (l, h) = self.levels[k - 1][i];
                    if l > h {
                        (f64::NAN, f64::NAN)
                    } else {
                        (f64::from(l), f64::from(h))
                    }
                }
            };
            if k > self.levels.len() {
                break;
            }
            if a & 1 == 1 {
                take(at(a));
                a += 1;
            }
            if b & 1 == 1 {
                b -= 1;
                take(at(b));
            }
            if a >= b {
                break;
            }
            if k == self.levels.len() {
                // Top level: the rest is a plain scan of at most two blocks.
                for i in a..b {
                    take(at(i));
                }
                break;
            }
            a >>= 1;
            b >>= 1;
            k += 1;
        }
        (lo <= hi).then_some((lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn queries_cover_the_exact_extremes(
            pairs in prop::collection::vec((-1e6f64..1e6, 0.0f64..1e3, prop::bool::weighted(0.1)), 1..300),
            last in (-1e6f64..1e6, 0.0f64..1e3),
            ranges in prop::collection::vec((0usize..300, 0usize..300), 1..20),
        ) {
            let mut idx = MinMaxIndex::new();
            let mut exact: Vec<(f64, f64)> = Vec::new();
            for &(lo, w, gap) in &pairs {
                let p = if gap { (f64::NAN, f64::NAN) } else { (lo, lo + w) };
                idx.push(p.0, p.1).unwrap();
                exact.push(p);
            }
            idx.set_last(last.0, last.0 + last.1);
            *exact.last_mut().unwrap() = (last.0, last.0 + last.1);
            for (x, y) in ranges {
                let (a, b) = (x.min(y), x.max(y));
                let want = exact.get(a..b.min(exact.len())).unwrap_or(&[])
                    .iter()
                    .filter(|p| !p.0.is_nan())
                    .fold(None, |acc: Option<(f64, f64)>, p| Some(match acc {
                        None => *p,
                        Some(q) => (q.0.min(p.0), q.1.max(p.1)),
                    }));
                let got = idx.query(a, b);
                match (want, got) {
                    (None, None) => {}
                    (Some(w), Some(g)) => {
                        prop_assert!(g.0 <= w.0 && g.1 >= w.1, "{g:?} {w:?}");
                        prop_assert!(f64::from(f32_down(w.0)) <= g.0 && g.1 <= f64::from(f32_up(w.1)), "{g:?} {w:?}");
                    }
                    _ => prop_assert!(false, "{want:?} {got:?}"),
                }
            }
        }
    }
}
