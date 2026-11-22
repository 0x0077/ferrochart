//! The global, index-based time axis shared by all series of a chart.
//!
//! Every distinct timestamp occupies one equal-width slot, so market closures,
//! weekends and gaps are skipped naturally. Series refer to slots by **logical
//! index**. Logical indices of existing timestamps never change on `append` or
//! `prepend`; only [`TimeIndex::merge`] (a series that is not a subset of the
//! time between existing slots) can shift them, and that requires a full rebuild.
//!

use super::{DeqColumn, Timestamp};
use crate::error::{Error, Result};

/// Sorted union of the timestamps of all series, with stable logical indices.
///
/// The slot of `times()[k]` has logical index `first_index() + k`. A new index
/// starts at logical index 0; `prepend` moves `first_index()` down so that the
/// existing timestamps keep their indices.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::TimeIndex;
///
/// let mut index = TimeIndex::from_times(vec![100, 200, 300])?;
/// index.append(&[400])?;
/// index.prepend(&[0, 50])?;
///
/// // Existing timestamps keep their logical index; history gets negative ones.
/// assert_eq!(index.index_of(100), Some(0));
/// assert_eq!(index.index_of(0), Some(-2));
/// assert_eq!(index.time_at(3), Some(400));
/// assert_eq!((index.first_index(), index.end_index()), (-2, 4));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimeIndex {
    times: DeqColumn<Timestamp>,
    /// Logical index of `times[0]`. Decreases on `prepend`.
    first: i64,
}

/// How the points of one series map to logical indices of the [`TimeIndex`].
///
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::{SeriesMapping, TimeIndex};
///
/// let index = TimeIndex::from_times(vec![10, 20, 30, 40])?;
///
/// // A contiguous run of the index (e.g. an indicator computed from the main series).
/// let aligned = index.map_series(&[20, 30, 40])?;
/// assert_eq!(aligned, Some(SeriesMapping::Aligned { first: 1 }));
///
/// // A sparse subset needs a lookup table.
/// let mapped = index.map_series(&[10, 40])?.unwrap();
/// assert!(!mapped.is_aligned());
/// assert_eq!(mapped.logical_index(1), Some(3));
///
/// // Not a subset: the caller has to merge.
/// assert_eq!(index.map_series(&[15])?, None);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeriesMapping {
    /// Fast path: point `i` of the series is at logical index `first + i`. No
    /// per-point memory.
    Aligned {
        /// Logical index of the series' first point.
        first: i64,
    },
    /// General path: point `i` is at logical index `first + offsets[i]`.
    /// `offsets` is strictly increasing and starts at 0. 4 bytes per point.
    Mapped {
        /// Logical index of the series' first point.
        first: i64,
        /// Offset of each point from `first`.
        offsets: Vec<u32>,
    },
}

impl SeriesMapping {
    /// Whether this is the [`Aligned`](Self::Aligned) fast path.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::SeriesMapping;
    ///
    /// assert!(SeriesMapping::Aligned { first: 0 }.is_aligned());
    /// ```
    pub fn is_aligned(&self) -> bool {
        matches!(self, Self::Aligned { .. })
    }

    /// Logical index of point `i` of the series. For `Mapped`, `None` if `i` is
    /// out of range; for `Aligned`, `None` only if the index overflows `i64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::SeriesMapping;
    ///
    /// let m = SeriesMapping::Mapped { first: -5, offsets: vec![0, 2, 7] };
    /// assert_eq!(m.logical_index(1), Some(-3));
    /// assert_eq!(m.logical_index(3), None);
    /// ```
    pub fn logical_index(&self, i: usize) -> Option<i64> {
        match self {
            Self::Aligned { first } => first.checked_add(i64::try_from(i).ok()?),
            Self::Mapped { first, offsets } => offsets.get(i).map(|&off| first + i64::from(off)),
        }
    }
}

impl TimeIndex {
    /// Creates an empty index. Its first slot will get logical index 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// assert!(TimeIndex::new().is_empty());
    /// ```
    pub const fn new() -> Self {
        Self {
            times: DeqColumn::new(),
            first: 0,
        }
    }

    /// Builds an index from timestamps without copying them.
    /// Logical indices start at 0.
    ///
    /// # Errors
    ///
    /// [`Error::TimeNotIncreasing`] if `times` is not strictly increasing.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Error, data::TimeIndex};
    ///
    /// assert_eq!(TimeIndex::from_times(vec![1, 2, 2]), Err(Error::TimeNotIncreasing { index: 2 }));
    /// assert_eq!(TimeIndex::from_times(vec![1, 2, 3])?.len(), 3);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn from_times(times: Vec<Timestamp>) -> Result<Self> {
        check_increasing(&times, None)?;
        Ok(Self {
            times: DeqColumn::from_vec(times),
            first: 0,
        })
    }

    /// Number of slots.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// assert_eq!(TimeIndex::from_times(vec![5, 6])?.len(), 2);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Whether there are no slots.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// assert!(TimeIndex::default().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// All timestamps, sorted, as one slice. `times()[k]` has logical index
    /// `first_index() + k`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let index = TimeIndex::from_times(vec![5, 6])?;
    /// assert_eq!(index.times(), &[5, 6]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn times(&self) -> &[Timestamp] {
        &self.times
    }

    /// Logical index of the first slot.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::from_times(vec![5, 6])?;
    /// index.prepend(&[1, 2, 3])?;
    /// assert_eq!(index.first_index(), -3);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn first_index(&self) -> i64 {
        self.first
    }

    /// Logical index one past the last slot (`first_index() + len()`).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// assert_eq!(TimeIndex::from_times(vec![5, 6])?.end_index(), 2);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn end_index(&self) -> i64 {
        self.first + len_i64(self.times.len())
    }

    /// Timestamp of the slot with logical index `index`, or `None` outside the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let index = TimeIndex::from_times(vec![5, 6])?;
    /// assert_eq!(index.time_at(1), Some(6));
    /// assert_eq!(index.time_at(2), None);
    /// assert_eq!(index.time_at(-1), None);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn time_at(&self, index: i64) -> Option<Timestamp> {
        let pos = usize::try_from(index.checked_sub(self.first)?).ok()?;
        self.times.get(pos).copied()
    }

    /// Logical index of `time` if it is in the index. O(log n).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let index = TimeIndex::from_times(vec![5, 6])?;
    /// assert_eq!(index.index_of(6), Some(1));
    /// assert_eq!(index.index_of(7), None);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn index_of(&self, time: Timestamp) -> Option<i64> {
        let pos = self.times.binary_search(&time).ok()?;
        Some(self.first + len_i64(pos))
    }

    /// Logical index of the first slot whose time is `>= time`; `end_index()` if
    /// there is none. O(log n).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let index = TimeIndex::from_times(vec![10, 20, 30])?;
    /// assert_eq!(index.lower_bound(20), 1);
    /// assert_eq!(index.lower_bound(21), 2);
    /// assert_eq!(index.lower_bound(99), 3);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn lower_bound(&self, time: Timestamp) -> i64 {
        self.first + len_i64(self.times.partition_point(|&t| t < time))
    }

    /// Appends new slots after the last one. Existing logical indices are unchanged.
    ///
    /// # Errors
    ///
    /// - [`Error::TimeNotIncreasing`] if `times` is not strictly increasing or its
    ///   first element is not after the current last timestamp (`index` refers to
    ///   `times`);
    /// - [`Error::OutOfMemory`] if the allocation fails.
    ///
    /// The index is unchanged on error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Error, data::TimeIndex};
    ///
    /// let mut index = TimeIndex::from_times(vec![10, 20])?;
    /// assert_eq!(index.append(&[20]), Err(Error::TimeNotIncreasing { index: 0 }));
    /// index.append(&[30, 40])?;
    /// assert_eq!(index.times(), &[10, 20, 30, 40]);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn append(&mut self, times: &[Timestamp]) -> Result<()> {
        check_increasing(times, self.times.last().copied())?;
        self.times.extend_back(times)
    }

    /// Inserts older slots before the first one (loading history). Existing
    /// timestamps keep their logical indices; the new ones get the indices just
    /// below `first_index()`.
    ///
    /// # Errors
    ///
    /// - [`Error::TimeNotIncreasing`] if `times` is not strictly increasing or its
    ///   last element is not before the current first timestamp (`index` refers
    ///   to `times`);
    /// - [`Error::InvalidArgument`] if the logical index would overflow `i64`;
    /// - [`Error::OutOfMemory`] if the allocation fails.
    ///
    /// The index is unchanged on error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::from_times(vec![30, 40])?;
    /// index.prepend(&[10, 20])?;
    /// assert_eq!(index.times(), &[10, 20, 30, 40]);
    /// assert_eq!(index.index_of(30), Some(0));
    /// assert_eq!(index.index_of(10), Some(-2));
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn prepend(&mut self, times: &[Timestamp]) -> Result<()> {
        check_increasing(times, None)?;
        if let (Some(&last_new), Some(&first_old)) = (times.last(), self.times.first())
            && last_new >= first_old
        {
            return Err(Error::TimeNotIncreasing {
                index: times.len() - 1,
            });
        }
        let first = self
            .first
            .checked_sub(len_i64(times.len()))
            .ok_or(Error::InvalidArgument("logical index overflows i64"))?;
        self.times.extend_front(times)?;
        self.first = first;
        Ok(())
    }

    /// Maps a series' timestamps onto the index.
    ///
    /// Returns `Some(Aligned)` if `times` equals a contiguous run of the index,
    /// `Some(Mapped)` if it is a sparse subset, and `None` if it contains a
    /// timestamp that is not in the index or is not strictly increasing (the
    /// caller then has to [`merge`](Self::merge)). An empty
    /// series maps to `Aligned { first: end_index() }`.
    ///
    /// Cost: O(m) when aligned, O(m log n) otherwise (m = `times.len()`).
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] if a mapped series spans more than `u32::MAX`
    ///   slots;
    /// - [`Error::OutOfMemory`] if the offset table cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::{SeriesMapping, TimeIndex};
    ///
    /// let index = TimeIndex::from_times(vec![10, 20, 30])?;
    /// assert_eq!(index.map_series(&[10, 20, 30])?, Some(SeriesMapping::Aligned { first: 0 }));
    /// assert_eq!(
    ///     index.map_series(&[10, 30])?,
    ///     Some(SeriesMapping::Mapped { first: 0, offsets: vec![0, 2] }),
    /// );
    /// assert_eq!(index.map_series(&[30, 10])?, None); // not increasing
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn map_series(&self, times: &[Timestamp]) -> Result<Option<SeriesMapping>> {
        let Some(&t0) = times.first() else {
            return Ok(Some(SeriesMapping::Aligned {
                first: self.end_index(),
            }));
        };
        let Ok(start) = self.times.binary_search(&t0) else {
            return Ok(None);
        };
        let first = self.first + len_i64(start);

        // Fast path: a contiguous run (one memcmp).
        if self.times.get(start..start + times.len()) == Some(times) {
            return Ok(Some(SeriesMapping::Aligned { first }));
        }

        // General path: each next timestamp must be found strictly after the
        // previous match, which also rejects unsorted or duplicate input.
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(times.len())
            .map_err(|_| Error::OutOfMemory)?;
        let mut pos = start;
        for &t in times {
            let rest = &self.times[pos..];
            let step = rest.partition_point(|&x| x < t);
            if rest.get(step) != Some(&t) {
                return Ok(None);
            }
            pos += step;
            let offset = u32::try_from(pos - start).map_err(|_| {
                Error::InvalidArgument("mapped series spans more than u32::MAX slots")
            })?;
            offsets.push(offset);
            pos += 1;
        }
        Ok(Some(SeriesMapping::Mapped { first, offsets }))
    }

    /// Merges timestamps that are not yet in the index, keeping it sorted, and
    /// returns how many slots were inserted.
    ///
    /// Timestamps before the current first one get logical indices below
    /// `first_index()`, so those slots behave like a prepend. Any timestamp
    /// inserted **between** existing ones shifts the logical index of every later
    /// slot: the caller must then rebuild everything that stores logical indices
    /// (series mappings, LOD, GPU buffers) and report a warning. O(n + m).
    ///
    /// # Errors
    ///
    /// - [`Error::TimeNotIncreasing`] if `times` is not strictly increasing;
    /// - [`Error::InvalidArgument`] if the logical index would overflow `i64`;
    /// - [`Error::OutOfMemory`] if the allocation fails.
    ///
    /// The index is unchanged on error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::from_times(vec![10, 20, 30])?;
    /// assert_eq!(index.merge(&[5, 20, 25])?, 2);
    /// assert_eq!(index.times(), &[5, 10, 20, 25, 30]);
    /// assert_eq!(index.index_of(10), Some(0)); // before the first insertion: unchanged
    /// assert_eq!(index.index_of(30), Some(3)); // after an inner insertion: shifted
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn merge(&mut self, times: &[Timestamp]) -> Result<usize> {
        check_increasing(times, None)?;
        let old = self.times.as_slice();
        let before_first = old
            .first()
            .map_or(0, |&f| times.partition_point(|&t| t < f));

        let mut merged = Vec::new();
        merged
            .try_reserve_exact(old.len() + times.len())
            .map_err(|_| Error::OutOfMemory)?;
        let (mut i, mut j) = (0, 0);
        while i < old.len() && j < times.len() {
            let (a, b) = (old[i], times[j]);
            merged.push(a.min(b));
            i += usize::from(a <= b);
            j += usize::from(b <= a);
        }
        merged.extend_from_slice(&old[i..]);
        merged.extend_from_slice(&times[j..]);

        let inserted = merged.len() - old.len();
        if inserted == 0 {
            return Ok(0);
        }
        let first = self
            .first
            .checked_sub(len_i64(before_first))
            .ok_or(Error::InvalidArgument("logical index overflows i64"))?;
        self.times = DeqColumn::from_vec(merged);
        self.first = first;
        Ok(inserted)
    }

    /// Reserves room for `additional` slots after the last one, so that a later
    /// [`append`](Self::append) of that many cannot fail with `OutOfMemory`.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::new();
    /// index.reserve_back(10)?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn reserve_back(&mut self, additional: usize) -> Result<()> {
        self.times.reserve_back(additional)
    }

    /// Reserves room for `additional` slots before the first one, so that a later
    /// [`prepend`](Self::prepend) of that many cannot fail with `OutOfMemory`.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfMemory`] if the allocation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::new();
    /// index.reserve_front(10)?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn reserve_front(&mut self, additional: usize) -> Result<()> {
        self.times.reserve_front(additional)
    }

    /// Releases growth slack (called after `set_data`).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::TimeIndex;
    ///
    /// let mut index = TimeIndex::new();
    /// index.append(&[1, 2, 3])?;
    /// index.shrink_to_fit();
    /// assert_eq!(index.len(), 3);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn shrink_to_fit(&mut self) {
        self.times.shrink_to_fit();
    }

    /// Removes `front` slots at the front and `back` at the back (undoes a
    /// `prepend`/`append` whose series operation then failed).
    pub(crate) fn truncate_ends(&mut self, front: usize, back: usize) {
        let front = front.min(self.times.len());
        self.times.truncate_ends(front, back);
        self.first += len_i64(front);
    }

    /// Moves the logical indices so that the first slot gets index `first`.
    pub(crate) fn rebase(&mut self, first: i64) {
        self.first = first;
    }
}

/// Checks that `times` is strictly increasing and, if given, starts after `prev`.
fn check_increasing(times: &[Timestamp], prev: Option<Timestamp>) -> Result<()> {
    if let (Some(p), Some(&t0)) = (prev, times.first())
        && t0 <= p
    {
        return Err(Error::TimeNotIncreasing { index: 0 });
    }
    match times.windows(2).position(|w| w[1] <= w[0]) {
        Some(k) => Err(Error::TimeNotIncreasing { index: k + 1 }),
        None => Ok(()),
    }
}

/// Lengths and positions fit in `i64`: a `Vec` never exceeds `isize::MAX` elements.
fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    fn sorted(max_len: usize) -> impl Strategy<Value = Vec<i64>> {
        prop::collection::btree_set(-1000i64..1000, 0..max_len)
            .prop_map(|s| s.into_iter().collect())
    }

    #[test]
    fn empty_index() {
        let index = TimeIndex::new();
        assert_eq!((index.first_index(), index.end_index()), (0, 0));
        assert_eq!(index.time_at(0), None);
        assert_eq!(index.index_of(0), None);
        assert_eq!(index.lower_bound(0), 0);
    }

    #[test]
    fn append_and_prepend_on_empty_index() {
        let mut index = TimeIndex::new();
        index.prepend(&[1, 2]).unwrap();
        assert_eq!(index.first_index(), -2);
        index.append(&[3]).unwrap();
        assert_eq!(index.index_of(3), Some(0));
    }

    #[test]
    fn errors_leave_index_unchanged() {
        let mut index = TimeIndex::from_times(vec![10, 20, 30]).unwrap();
        let before = index.clone();
        assert_eq!(
            index.append(&[40, 40]),
            Err(Error::TimeNotIncreasing { index: 1 })
        );
        assert_eq!(
            index.prepend(&[0, 10]),
            Err(Error::TimeNotIncreasing { index: 1 })
        );
        assert_eq!(
            index.prepend(&[5, 1]),
            Err(Error::TimeNotIncreasing { index: 1 })
        );
        assert_eq!(
            index.merge(&[1, 1]),
            Err(Error::TimeNotIncreasing { index: 1 })
        );
        assert_eq!(index, before);
    }

    #[test]
    fn prepend_overflow_is_an_error() {
        let mut index = TimeIndex::from_times(vec![10]).unwrap();
        index.first = i64::MIN + 1;
        assert!(matches!(
            index.prepend(&[1, 2]),
            Err(Error::InvalidArgument(_))
        ));
        assert_eq!(index.first_index(), i64::MIN + 1);
    }

    #[test]
    fn map_series_rejects_duplicates() {
        let index = TimeIndex::from_times(vec![10, 20, 30]).unwrap();
        assert_eq!(index.map_series(&[10, 10]).unwrap(), None);
        assert_eq!(index.map_series(&[10, 30, 30]).unwrap(), None);
    }

    #[test]
    fn mapping_survives_prepend() {
        let mut index = TimeIndex::from_times(vec![10, 20, 30, 40]).unwrap();
        let mapping = index.map_series(&[20, 40]).unwrap().unwrap();
        index.prepend(&[1, 2, 3]).unwrap();
        index.append(&[50]).unwrap();
        assert_eq!(index.time_at(mapping.logical_index(0).unwrap()), Some(20));
        assert_eq!(index.time_at(mapping.logical_index(1).unwrap()), Some(40));
    }

    #[test]
    fn merge_without_new_times_is_a_no_op() {
        let mut index = TimeIndex::from_times(vec![10, 20]).unwrap();
        index.prepend(&[5]).unwrap();
        let before = index.clone();
        assert_eq!(index.merge(&[5, 20]).unwrap(), 0);
        assert_eq!(index, before);
    }

    proptest! {
        /// Appends and prepends never change the logical index of existing times.
        #[test]
        fn logical_indices_are_stable(base in sorted(50), older in sorted(50), newer in sorted(50)) {
            let older: Vec<i64> = older.into_iter().map(|t| t - 3000).collect();
            let newer: Vec<i64> = newer.into_iter().map(|t| t + 3000).collect();
            let mut index = TimeIndex::from_times(base.clone()).unwrap();
            let before: Vec<Option<i64>> = base.iter().map(|&t| index.index_of(t)).collect();
            index.prepend(&older).unwrap();
            index.append(&newer).unwrap();
            for (&t, &idx) in base.iter().zip(&before) {
                prop_assert_eq!(index.index_of(t), idx);
            }
            for k in index.first_index()..index.end_index() {
                let t = index.time_at(k).unwrap();
                prop_assert_eq!(index.index_of(t), Some(k));
            }
        }

        /// A mapping always points back at the series' own timestamps, and it is
        /// `Aligned` exactly when the series is a contiguous run of the index.
        #[test]
        fn map_series_round_trips(all in sorted(200), picks in prop::collection::vec(any::<bool>(), 200)) {
            let index = TimeIndex::from_times(all.clone()).unwrap();
            let series: Vec<i64> = all.iter().zip(&picks).filter(|(_, p)| **p).map(|(t, _)| *t).collect();
            let mapping = index.map_series(&series).unwrap().unwrap();
            for (i, &t) in series.iter().enumerate() {
                prop_assert_eq!(index.time_at(mapping.logical_index(i).unwrap()), Some(t));
            }
            let contiguous = series.is_empty() || {
                let s = all.binary_search(&series[0]).unwrap();
                all[s..].starts_with(&series)
            };
            prop_assert_eq!(mapping.is_aligned(), contiguous);
        }

        /// `map_series` returns `None` exactly when some timestamp is not in the index.
        #[test]
        fn map_series_detects_non_subsets(all in sorted(100), series in sorted(20)) {
            let index = TimeIndex::from_times(all.clone()).unwrap();
            let is_subset = series.iter().all(|t| all.binary_search(t).is_ok());
            prop_assert_eq!(index.map_series(&series).unwrap().is_some(), is_subset);
        }

        /// `merge` produces the sorted union and keeps indices of timestamps before
        /// the first inner insertion.
        #[test]
        fn merge_is_sorted_union(base in sorted(100), extra in sorted(100)) {
            let mut index = TimeIndex::from_times(base.clone()).unwrap();
            let inserted = index.merge(&extra).unwrap();
            let union: BTreeSet<i64> = base.iter().chain(&extra).copied().collect();
            let union: Vec<i64> = union.into_iter().collect();
            prop_assert_eq!(index.times(), &union[..]);
            prop_assert_eq!(inserted, union.len() - base.len());
            if let Some(&b0) = base.first() {
                prop_assert_eq!(index.index_of(b0), Some(0));
            }
        }
    }
}
