//! Storage of one OHLC(V) series: `f64` columns, LOD pyramid, generation and
//! change log, with validation at the API boundary.
//!
//! The timestamps live in the global [`TimeIndex`] and are not stored again
//! here: bar `p` is logical index `first + p`, and slots of the axis the
//! series has no bar for hold gap bars (all prices NaN). Every operation validates first and only then
//! mutates, so an error leaves both the series and the index unchanged.
//!

use super::lod::{ANCHOR, Lod, Raw};
use super::{ChangeLog, DeqColumn, IdxSpan, OhlcColumns, TimeIndex, Timestamp};
use crate::error::{Error, Result};

/// How bars that violate `low <= min(open, close) <= max(open, close) <= high` are
/// handled. The default is the same in debug and release builds.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::ValidationMode;
///
/// assert_eq!(ValidationMode::default(), ValidationMode::Reject);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationMode {
    /// Return [`Error::InvalidOhlc`].
    #[default]
    Reject,
    /// Widen `high`/`low` to include `open`/`close` and report the number of
    /// repaired bars in a `ChartEvent::Warning`. Non-finite values are still
    /// rejected.
    Repair,
}

/// A data change, as seen by downstream consumers (LOD, indicators).
///
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::DataChange;
///
/// let change = DataChange::Append { count: 2 };
/// assert_ne!(change, DataChange::UpdateLast);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataChange {
    /// Everything was replaced.
    Replace,
    /// `count` bars were added at the end.
    Append {
        /// Number of new bars.
        count: usize,
    },
    /// The last bar was modified (the main real-time path).
    UpdateLast,
    /// `count` bars were added at the beginning (history loading).
    Prepend {
        /// Number of new bars.
        count: usize,
    },
}

/// A real-time update for `Chart::update_last`.
///
/// If `time` equals the time of the last bar, that bar is replaced; if it is
/// later, a new bar is appended.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::BarUpdate;
///
/// let tick = BarUpdate { time: 60_000, open: 10.0, high: 11.0, low: 9.5, close: 10.5, volume: Some(3.0) };
/// assert!(tick.low <= tick.close);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarUpdate {
    /// Bar open time (UTC milliseconds).
    pub time: Timestamp,
    /// Open price.
    pub open: f64,
    /// High price.
    pub high: f64,
    /// Low price.
    pub low: f64,
    /// Close price.
    pub close: f64,
    /// Volume; must be present exactly when the series has volume.
    pub volume: Option<f64>,
}

/// A read-only column of one LOD level: `f64` at level 0, `f32` above.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::Column;
///
/// let col = Column::F32(&[1.5, 2.5]);
/// assert_eq!(col.get(1), Some(2.5));
/// assert_eq!(col.len(), 2);
/// ```
#[derive(Clone, Copy, Debug)]
pub enum Column<'a> {
    /// Level 0: original data.
    F64(&'a [f64]),
    /// LOD levels: `f32` with directed rounding.
    F32(&'a [f32]),
}

impl Column<'_> {
    /// Number of values.
    pub fn len(&self) -> usize {
        match self {
            Self::F64(s) => s.len(),
            Self::F32(s) => s.len(),
        }
    }

    /// Whether the column is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Value at position `i` (not a logical index), widened to `f64`.
    pub fn get(&self, i: usize) -> Option<f64> {
        match self {
            Self::F64(s) => s.get(i).copied(),
            Self::F32(s) => s.get(i).copied().map(f64::from),
        }
    }
}

/// Read-only view of one level of an OHLC series.
///
/// At level 0 element `p` has logical index `first + p`; at level `k >= 2` it is
/// bucket number `first + p`, covering logical indices
/// `ANCHOR + b * 2^k .. ANCHOR + (b + 1) * 2^k` (see [`ANCHOR`](super::ANCHOR)).
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::{Column, OhlcLevelView};
///
/// let v = OhlcLevelView {
///     level: 0, first: 10,
///     open: Column::F64(&[1.0]), high: Column::F64(&[2.0]),
///     low: Column::F64(&[0.5]), close: Column::F64(&[1.5]), volume: None,
/// };
/// assert_eq!((v.len(), v.end()), (1, 11));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct OhlcLevelView<'a> {
    /// LOD level (0 = original data).
    pub level: u8,
    /// Logical index (level 0) or bucket number (level >= 2) of the first element.
    pub first: i64,
    /// Open values.
    pub open: Column<'a>,
    /// High values.
    pub high: Column<'a>,
    /// Low values.
    pub low: Column<'a>,
    /// Close values.
    pub close: Column<'a>,
    /// Volumes (summed in LOD levels), if the series has volume.
    pub volume: Option<Column<'a>>,
}

impl OhlcLevelView<'_> {
    /// Number of elements.
    pub fn len(&self) -> usize {
        self.open.len()
    }

    /// Whether the view is empty.
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// One past the last element's index.
    pub fn end(&self) -> i64 {
        self.first + i64::try_from(self.len()).unwrap_or(i64::MAX)
    }
}

/// Logical indices are kept within `±2^39` so that `index - ANCHOR` never
/// overflows and stays non-negative.
const INDEX_LIMIT: i64 = 1 << 39;

#[derive(Clone, Copy, Debug)]
struct ExtremesCache {
    from: i64,
    to: i64,
    generation: u64,
    result: Option<(f64, f64)>,
}

/// Storage of one OHLC series (gap bars allowed).
#[derive(Clone, Debug, Default)]
pub(crate) struct OhlcStore {
    first: i64,
    open: DeqColumn<f64>,
    high: DeqColumn<f64>,
    low: DeqColumn<f64>,
    close: DeqColumn<f64>,
    volume: Option<DeqColumn<f64>>,
    lod: Lod,
    log: ChangeLog,
    extremes_cache: Option<ExtremesCache>,
}

macro_rules! raw {
    ($s:expr) => {
        Raw {
            first: $s.first,
            open: &$s.open,
            high: &$s.high,
            low: &$s.low,
            close: &$s.close,
            volume: $s.volume.as_deref(),
        }
    };
}

/// All columns must have the length of `open`; `time` too unless it is empty
/// (secondary series, whose times the caller has already mapped).
fn check_lengths(c: &OhlcColumns) -> Result<()> {
    let n = c.open.len();
    if !c.time.is_empty() && c.time.len() != n {
        return Err(Error::LengthMismatch { column: "open" });
    }
    let cols = [
        ("open", c.open.len()),
        ("high", c.high.len()),
        ("low", c.low.len()),
        ("close", c.close.len()),
        ("volume", c.volume.as_ref().map_or(n, Vec::len)),
    ];
    match cols.iter().find(|(_, len)| *len != n) {
        Some(&(column, _)) => Err(Error::LengthMismatch { column }),
        None => Ok(()),
    }
}

/// Whether a bar is a gap: open, high, low and close all NaN (a slot of the
/// time axis that the series does not cover).
pub(crate) fn is_gap_bar(o: f64, h: f64, l: f64, c: f64) -> bool {
    o.is_nan() && h.is_nan() && l.is_nan() && c.is_nan()
}

/// Validates (and in `Repair` mode fixes) the bars. Returns the number of
/// repaired bars. Gap bars are allowed; their volume is set to 0. In `Reject`
/// mode only gap volumes are modified.
fn check_bars(
    o: &[f64],
    h: &mut [f64],
    l: &mut [f64],
    c: &[f64],
    mut v: Option<&mut [f64]>,
    mode: ValidationMode,
) -> Result<usize> {
    let mut repaired = 0;
    for i in 0..o.len() {
        if is_gap_bar(o[i], h[i], l[i], c[i]) {
            if let Some(v) = v.as_deref_mut() {
                v[i] = 0.0;
            }
            continue;
        }
        let vol_ok = v.as_deref().is_none_or(|v| v[i].is_finite());
        if !(o[i].is_finite() && h[i].is_finite() && l[i].is_finite() && c[i].is_finite() && vol_ok)
        {
            return Err(Error::InvalidOhlc { index: i });
        }
        let top = o[i].max(c[i]);
        let bottom = o[i].min(c[i]);
        if h[i] < top || l[i] > bottom {
            match mode {
                ValidationMode::Reject => return Err(Error::InvalidOhlc { index: i }),
                ValidationMode::Repair => {
                    h[i] = h[i].max(top);
                    l[i] = l[i].min(bottom);
                    repaired += 1;
                }
            }
        }
    }
    Ok(repaired)
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Checks the column lengths and validates (in `Repair` mode fixes) the bars,
/// before they are rearranged. Returns the number of repaired bars.
pub(crate) fn check_columns(cols: &mut OhlcColumns, mode: ValidationMode) -> Result<usize> {
    check_lengths(cols)?;
    check_bars(
        &cols.open,
        &mut cols.high,
        &mut cols.low,
        &cols.close,
        cols.volume.as_deref_mut(),
        mode,
    )
}

impl OhlcStore {
    pub fn len(&self) -> usize {
        self.open.len()
    }

    pub fn first(&self) -> i64 {
        self.first
    }

    pub fn end(&self) -> i64 {
        self.first + to_i64(self.len())
    }

    pub fn has_volume(&self) -> bool {
        self.volume.is_some()
    }

    pub fn log(&self) -> &ChangeLog {
        &self.log
    }

    pub fn lod(&self) -> &Lod {
        &self.lod
    }

    /// View of level `level` (0 or a built LOD level).
    pub fn level_view(&self, level: u8) -> Option<OhlcLevelView<'_>> {
        if level == 0 {
            return Some(OhlcLevelView {
                level,
                first: self.first,
                open: Column::F64(&self.open),
                high: Column::F64(&self.high),
                low: Column::F64(&self.low),
                close: Column::F64(&self.close),
                volume: self.volume.as_deref().map(Column::F64),
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
            volume: l.volume.as_deref().map(Column::F32),
        })
    }

    /// Replaces all data and the time index. Returns the number of repaired bars.
    pub fn replace(
        &mut self,
        index: &mut TimeIndex,
        mut cols: OhlcColumns,
        mode: ValidationMode,
    ) -> Result<usize> {
        check_lengths(&cols)?;
        if cols.time.len() != cols.open.len() {
            return Err(Error::LengthMismatch { column: "open" });
        }
        let new_index = TimeIndex::from_times(core::mem::take(&mut cols.time))?;
        let repaired = self.replace_at(new_index.first_index(), cols, mode)?;
        *index = new_index;
        index.shrink_to_fit();
        Ok(repaired)
    }

    /// Replaces all data of a secondary series without touching the time
    /// index: bar `p` goes to logical index `first + p` (`cols.time` is
    /// ignored; the caller has mapped it). Returns the number of repaired bars.
    pub fn replace_at(
        &mut self,
        first: i64,
        mut cols: OhlcColumns,
        mode: ValidationMode,
    ) -> Result<usize> {
        check_lengths(&cols)?;
        if to_i64(cols.len()) >= INDEX_LIMIT {
            return Err(Error::InvalidArgument("too many bars"));
        }
        let repaired = check_bars(
            &cols.open,
            &mut cols.high,
            &mut cols.low,
            &cols.close,
            cols.volume.as_deref_mut(),
            mode,
        )?;
        let mut next = Self {
            first,
            open: DeqColumn::from_vec(cols.open),
            high: DeqColumn::from_vec(cols.high),
            low: DeqColumn::from_vec(cols.low),
            close: DeqColumn::from_vec(cols.close),
            volume: cols.volume.map(DeqColumn::from_vec),
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
        for c in [
            &mut next.open,
            &mut next.high,
            &mut next.low,
            &mut next.close,
        ] {
            c.shrink_to_fit();
        }
        if let Some(v) = next.volume.as_mut() {
            v.shrink_to_fit();
        }
        next.log.replace();
        *self = next;
        Ok(repaired)
    }

    fn check_volume_presence(&self, has: bool) -> Result<()> {
        if has == self.has_volume() {
            Ok(())
        } else {
            Err(Error::LengthMismatch { column: "volume" })
        }
    }

    fn columns_mut(&mut self) -> impl Iterator<Item = &mut DeqColumn<f64>> {
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

    /// Appends bars after the last one. Returns the number of repaired bars.
    /// With `index`, the bars' times are appended to it;
    /// without, the caller has checked that they occupy the slots right after
    /// the last bar.
    pub fn append(
        &mut self,
        index: Option<&mut TimeIndex>,
        mut cols: OhlcColumns,
        mode: ValidationMode,
    ) -> Result<usize> {
        if self.len() == 0 {
            return match index {
                Some(index) => self.replace(index, cols, mode),
                None => self.replace_at(self.first, cols, mode),
            };
        }
        check_lengths(&cols)?;
        self.check_volume_presence(cols.volume.is_some())?;
        let m = cols.open.len();
        if index.is_some() && cols.time.len() != m {
            return Err(Error::LengthMismatch { column: "open" });
        }
        let new_end = self.end().saturating_add(to_i64(m));
        if new_end >= INDEX_LIMIT {
            return Err(Error::InvalidArgument("too many bars"));
        }
        let repaired = check_bars(
            &cols.open,
            &mut cols.high,
            &mut cols.low,
            &cols.close,
            cols.volume.as_deref_mut(),
            mode,
        )?;
        for c in self.columns_mut() {
            c.reserve_back(m)?;
        }
        self.lod.reserve(self.first, new_end)?;
        if let Some(index) = index {
            index.reserve_back(m)?;
            index.append(&cols.time)?; // validates the times, atomic
        }

        // Reserved above: these cannot fail.
        self.open.extend_back(&cols.open)?;
        self.high.extend_back(&cols.high)?;
        self.low.extend_back(&cols.low)?;
        self.close.extend_back(&cols.close)?;
        if let (Some(v), Some(src)) = (self.volume.as_mut(), cols.volume.as_deref()) {
            v.extend_back(src)?;
        }
        let span = IdxSpan {
            start: new_end - to_i64(m),
            end: new_end,
        };
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(repaired)
    }

    /// Inserts bars before the first one. Returns the number of repaired bars.
    /// `index` as in [`append`](Self::append).
    pub fn prepend(
        &mut self,
        index: Option<&mut TimeIndex>,
        mut cols: OhlcColumns,
        mode: ValidationMode,
    ) -> Result<usize> {
        if self.len() == 0 {
            if let Some(index) = index {
                return self.replace(index, cols, mode);
            }
            let first = self.first - to_i64(cols.open.len());
            return self.replace_at(first, cols, mode);
        }
        check_lengths(&cols)?;
        self.check_volume_presence(cols.volume.is_some())?;
        let m = cols.open.len();
        if index.is_some() && cols.time.len() != m {
            return Err(Error::LengthMismatch { column: "open" });
        }
        let new_first = self.first.saturating_sub(to_i64(m));
        if new_first <= -INDEX_LIMIT || new_first <= ANCHOR {
            return Err(Error::InvalidArgument("too many bars"));
        }
        let repaired = check_bars(
            &cols.open,
            &mut cols.high,
            &mut cols.low,
            &cols.close,
            cols.volume.as_deref_mut(),
            mode,
        )?;
        for c in self.columns_mut() {
            c.reserve_front(m)?;
        }
        self.lod.reserve(new_first, self.end())?;
        if let Some(index) = index {
            index.reserve_front(m)?;
            index.prepend(&cols.time)?; // validates the times, atomic
        }

        self.open.extend_front(&cols.open)?;
        self.high.extend_front(&cols.high)?;
        self.low.extend_front(&cols.low)?;
        self.close.extend_front(&cols.close)?;
        if let (Some(v), Some(src)) = (self.volume.as_mut(), cols.volume.as_deref()) {
            v.extend_front(src)?;
        }
        let span = IdxSpan {
            start: new_first,
            end: self.first,
        };
        self.first = new_first;
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(repaired)
    }

    /// Validates (and in `Repair` mode fixes) one bar. Returns the bar and
    /// whether it was repaired.
    pub(crate) fn check_one(bar: BarUpdate, mode: ValidationMode) -> Result<(BarUpdate, usize)> {
        let one = |x| [x];
        let mut high = one(bar.high);
        let mut low = one(bar.low);
        let mut volume = bar.volume.map(one);
        let repaired = check_bars(
            &one(bar.open),
            &mut high,
            &mut low,
            &one(bar.close),
            volume.as_mut().map(<[f64; 1]>::as_mut_slice),
            mode,
        )?;
        Ok((
            BarUpdate {
                high: high[0],
                low: low[0],
                volume: volume.map(|v| v[0]),
                ..bar
            },
            repaired,
        ))
    }

    /// Overwrites the last bar in place (no allocation); `bar.time` is not
    /// checked. Returns the number of repaired bars (0 or 1).
    pub fn set_last(&mut self, bar: BarUpdate, mode: ValidationMode) -> Result<usize> {
        self.set_at(self.end() - 1, bar, mode)
    }

    /// Overwrites the bar at logical index `j` in place (no allocation);
    /// `bar.time` is not checked. Returns the number of repaired bars (0 or 1).
    pub fn set_at(&mut self, j: i64, bar: BarUpdate, mode: ValidationMode) -> Result<usize> {
        let (bar, repaired) = Self::check_one(bar, mode)?;
        self.check_volume_presence(bar.volume.is_some())?;
        let Some(p) = j
            .checked_sub(self.first)
            .and_then(|p| usize::try_from(p).ok())
            .filter(|&p| p < self.len())
        else {
            return Err(Error::InvalidArgument("no bar at this index"));
        };
        self.open[p] = bar.open;
        self.high[p] = bar.high;
        self.low[p] = bar.low;
        self.close[p] = bar.close;
        if let (Some(v), Some(x)) = (self.volume.as_mut(), bar.volume) {
            v[p] = x;
        }
        let span = IdxSpan {
            start: j,
            end: j + 1,
        };
        self.lod.update(&raw!(self), span);
        self.log.record(span);
        Ok(repaired)
    }

    /// Removes the last `n` bars (all if fewer), keeping the capacity; never
    /// allocates. The new last bar is recorded as changed (its LOD buckets
    /// were recomputed).
    pub fn truncate_back(&mut self, n: usize) {
        let n = n.min(self.len());
        if n == 0 {
            return;
        }
        for c in self.columns_mut() {
            c.truncate_ends(0, n);
        }
        self.lod.truncate(&raw!(self));
        self.extremes_cache = None;
        let end = self.end();
        // Empty: only the generation moves.
        let start = if self.len() == 0 { end } else { end - 1 };
        self.log.record(IdxSpan { start, end });
    }

    /// One bar as columns (for appending it).
    pub fn bar_columns(bar: BarUpdate, with_time: bool) -> OhlcColumns {
        OhlcColumns {
            time: if with_time {
                vec![bar.time]
            } else {
                Vec::new()
            },
            open: vec![bar.open],
            high: vec![bar.high],
            low: vec![bar.low],
            close: vec![bar.close],
            volume: bar.volume.map(|v| vec![v]),
        }
    }

    /// Real-time update: replaces the last bar if `bar.time` equals its time
    /// (no allocation), appends a bar if it is later. Returns the change that was
    /// applied and the number of repaired bars (0 or 1). The chart goes
    /// through `series::Op::update_last` instead, which handles the union axis.
    #[cfg(test)]
    pub fn update_last(
        &mut self,
        index: &mut TimeIndex,
        bar: BarUpdate,
        mode: ValidationMode,
    ) -> Result<(DataChange, usize)> {
        let last_time = index.times().last().copied();
        match last_time {
            Some(t) if bar.time == t && self.len() > 0 => {
                let repaired = self.set_last(bar, mode)?;
                Ok((DataChange::UpdateLast, repaired))
            }
            Some(t) if bar.time < t => Err(Error::TimeNotIncreasing { index: 0 }),
            _ => {
                let (bar, repaired) = Self::check_one(bar, mode)?;
                // Already validated/repaired; Reject cannot fail again.
                self.append(
                    Some(index),
                    Self::bar_columns(bar, true),
                    ValidationMode::Reject,
                )?;
                Ok((DataChange::Append { count: 1 }, repaired))
            }
        }
    }

    /// Values of the bar at logical index `j`; `None` outside the data and
    /// for gap bars.
    pub fn bar_at(&self, j: i64) -> Option<[f64; 4]> {
        let p = usize::try_from(j.checked_sub(self.first)?).ok()?;
        let bar = [
            *self.open.get(p)?,
            *self.high.get(p)?,
            *self.low.get(p)?,
            *self.close.get(p)?,
        ];
        (!is_gap_bar(bar[0], bar[1], bar[2], bar[3])).then_some(bar)
    }

    /// Volume of the bar at logical index `j`.
    pub fn volume_at(&self, j: i64) -> Option<f64> {
        let p = usize::try_from(j.checked_sub(self.first)?).ok()?;
        self.volume.as_ref()?.get(p).copied()
    }

    /// Minimum low and maximum high over logical indices `from..to`, cached by
    /// `(range, generation)`.
    pub fn extremes(&mut self, from: i64, to: i64) -> Option<(f64, f64)> {
        let generation = self.log.generation();
        if let Some(c) = self.extremes_cache
            && (c.from, c.to, c.generation) == (from, to, generation)
        {
            return c.result;
        }
        // Ranges of gap bars only have no extremes.
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

    /// Heap bytes held by the series (columns and LOD).
    pub fn heap_bytes(&self) -> usize {
        let cols = 4 + usize::from(self.has_volume());
        cols * self.len() * core::mem::size_of::<f64>() + self.lod.heap_bytes()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact values are the point of these tests
mod tests {
    use super::*;

    fn cols(times: &[i64], closes: &[f64]) -> OhlcColumns {
        OhlcColumns {
            time: times.to_vec(),
            open: closes.to_vec(),
            high: closes.iter().map(|c| c + 1.0).collect(),
            low: closes.iter().map(|c| c - 1.0).collect(),
            close: closes.to_vec(),
            volume: Some(vec![1.0; closes.len()]),
        }
    }

    /// Every value of every level of `a` and `b`, as bits.
    fn levels(s: &OhlcStore) -> Vec<(u8, i64, Vec<u64>)> {
        let top = s.lod().max_level().unwrap_or(0);
        (0..=top)
            .filter_map(|k| s.level_view(k))
            .map(|v| {
                let mut all = Vec::new();
                for c in [&v.open, &v.high, &v.low, &v.close] {
                    all.extend((0..c.len()).map(|i| c.get(i).unwrap_or(0.0).to_bits()));
                }
                if let Some(c) = &v.volume {
                    all.extend((0..c.len()).map(|i| c.get(i).unwrap_or(0.0).to_bits()));
                }
                (v.level, v.first, all)
            })
            .collect()
    }

    #[test]
    fn truncate_back_equals_a_fresh_store() {
        let closes: Vec<f64> = (0..300).map(|i| 100.0 + f64::from(i % 37) * 0.75).collect();
        let times: Vec<i64> = (0..300).collect();
        for keep in [1, 2, 3, 4, 5, 63, 64, 65, 128, 200, 299] {
            let mut s = OhlcStore::default();
            s.replace_at(10, cols(&times, &closes), ValidationMode::Reject)
                .unwrap();
            let gen0 = s.log().generation();
            s.truncate_back(300 - keep);
            assert!(s.log().generation() > gen0);
            let mut fresh = OhlcStore::default();
            fresh
                .replace_at(
                    10,
                    cols(&times[..keep], &closes[..keep]),
                    ValidationMode::Reject,
                )
                .unwrap();
            assert_eq!(s.len(), keep);
            for (a, b) in levels(&s).iter().zip(levels(&fresh).iter()) {
                assert_eq!(a, b, "keep {keep}");
            }
            assert_eq!(
                s.extremes(10, 10 + keep as i64),
                fresh.extremes(10, 10 + keep as i64)
            );
            // Writing over the end again works.
            let bar = BarUpdate {
                time: 0,
                open: 1.0,
                high: 500.0,
                low: 0.5,
                close: 2.0,
                volume: Some(1.0),
            };
            s.append(
                None,
                OhlcStore::bar_columns(bar, false),
                ValidationMode::Reject,
            )
            .unwrap();
            assert_eq!(s.extremes(10, 11 + keep as i64).map(|e| e.1), Some(500.0));
        }
        let mut s = OhlcStore::default();
        s.replace_at(0, cols(&times, &closes), ValidationMode::Reject)
            .unwrap();
        s.truncate_back(1000);
        assert_eq!(s.len(), 0);
        assert_eq!(s.lod().max_level(), None);
    }

    fn setup() -> (OhlcStore, TimeIndex) {
        let mut s = OhlcStore::default();
        let mut index = TimeIndex::new();
        s.replace(
            &mut index,
            cols(&[10, 20, 30], &[1.0, 2.0, 3.0]),
            ValidationMode::Reject,
        )
        .unwrap();
        (s, index)
    }

    fn snapshot(s: &OhlcStore, index: &TimeIndex) -> (Vec<f64>, Vec<i64>, i64, u64) {
        (
            s.close.to_vec(),
            index.times().to_vec(),
            s.first,
            s.log.generation(),
        )
    }

    #[test]
    fn validation_errors_leave_store_unchanged() {
        let (mut s, mut index) = setup();
        let before = snapshot(&s, &index);
        let mut bad = cols(&[40], &[4.0]);
        bad.high[0] = 3.0; // below close
        assert_eq!(
            s.append(Some(&mut index), bad.clone(), ValidationMode::Reject),
            Err(Error::InvalidOhlc { index: 0 })
        );
        let mut nan = cols(&[40], &[4.0]);
        nan.open[0] = f64::NAN;
        assert_eq!(
            s.append(Some(&mut index), nan, ValidationMode::Repair),
            Err(Error::InvalidOhlc { index: 0 })
        );
        let mut short = cols(&[40, 50], &[4.0, 5.0]);
        short.low.pop();
        assert_eq!(
            s.append(Some(&mut index), short, ValidationMode::Reject),
            Err(Error::LengthMismatch { column: "low" })
        );
        let mut novol = cols(&[40], &[4.0]);
        novol.volume = None;
        assert_eq!(
            s.append(Some(&mut index), novol, ValidationMode::Reject),
            Err(Error::LengthMismatch { column: "volume" })
        );
        assert_eq!(
            s.append(
                Some(&mut index),
                cols(&[30], &[4.0]),
                ValidationMode::Reject
            ),
            Err(Error::TimeNotIncreasing { index: 0 })
        );
        assert_eq!(
            s.prepend(
                Some(&mut index),
                cols(&[5, 10], &[4.0, 5.0]),
                ValidationMode::Reject
            ),
            Err(Error::TimeNotIncreasing { index: 1 })
        );
        assert_eq!(snapshot(&s, &index), before);
        // Repair mode fixes the bar and reports it.
        assert_eq!(
            s.append(Some(&mut index), bad, ValidationMode::Repair),
            Ok(1)
        );
        assert_eq!(s.high[3], 4.0);
    }

    #[test]
    fn update_last_and_append_and_prepend() {
        let (mut s, mut index) = setup();
        let mut tick = BarUpdate {
            time: 30,
            open: 3.0,
            high: 9.0,
            low: 2.0,
            close: 8.0,
            volume: Some(2.0),
        };
        assert_eq!(
            s.update_last(&mut index, tick, ValidationMode::Reject),
            Ok((DataChange::UpdateLast, 0))
        );
        assert_eq!(s.close[2], 8.0);
        tick.time = 40;
        assert_eq!(
            s.update_last(&mut index, tick, ValidationMode::Reject),
            Ok((DataChange::Append { count: 1 }, 0))
        );
        tick.time = 35;
        assert_eq!(
            s.update_last(&mut index, tick, ValidationMode::Reject),
            Err(Error::TimeNotIncreasing { index: 0 })
        );
        s.prepend(
            Some(&mut index),
            cols(&[1, 2], &[0.5, 0.6]),
            ValidationMode::Reject,
        )
        .unwrap();
        assert_eq!(s.first(), -2);
        assert_eq!(index.first_index(), -2);
        assert_eq!(s.len(), 6);
        assert_eq!(s.extremes(-10, 10), Some((-0.5, 9.0)));
        assert_eq!(s.extremes(-10, 10), Some((-0.5, 9.0))); // cached
    }

    #[test]
    fn replace_resets_change_log() {
        let (mut s, mut index) = setup();
        let synced = s.log().generation();
        s.replace(&mut index, cols(&[1], &[1.0]), ValidationMode::Reject)
            .unwrap();
        assert_eq!(s.log().changes_since(synced), super::super::DataDelta::Full);
    }
}
