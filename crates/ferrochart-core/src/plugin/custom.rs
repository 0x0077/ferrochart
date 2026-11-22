//! Custom series: a plugin that owns its data, gives
//! core its times and range extremes, and writes immediate primitives for the
//! visible points.

use core::any::Any;
use core::fmt;

use crate::data::{ChangeLog, SeriesMapping, TimeIndex, Timestamp};
use crate::drawing::{DrawingCtx, FrameBuilder, index_of_time, time_of_index};
use crate::error::{Error, Result};
use crate::frame::SeriesStyle;
use crate::geom::Px;

/// How a custom series is placed on the time axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisMode {
    /// Each point's time is a slot of the time axis, like the points of a
    /// built-in series (e.g. footprint columns sharing the candles' slots).
    Slots,
    /// Points are placed between the existing slots by interpolating their
    /// times and add no slots (e.g. 1 s heatmap columns on a 1 min chart).
    Overlay,
}

/// A series that owns its data.
///
/// Core asks it for its times and extremes and lets it draw the visible
/// points; the data is changed through [`crate::Chart::update_custom`], which
/// tells core what changed. Point `i` is element `i` of
/// [`times`](Self::times).
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::{DataChange, Timestamp};
/// use ferrochart_core::drawing::FrameBuilder;
/// use ferrochart_core::geom::RectPx;
/// use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::testing::MonoMeasurer;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// /// A vertical bar from `lo` to `hi` per point.
/// #[derive(Default)]
/// struct Ranges {
///     time: Vec<Timestamp>,
///     range: Vec<(f64, f64)>,
///     index: MinMaxIndex,
/// }
///
/// impl Ranges {
///     fn push(&mut self, t: Timestamp, lo: f64, hi: f64) -> DataChange {
///         self.time.push(t);
///         self.range.push((lo, hi));
///         let _ = self.index.push(lo, hi);
///         DataChange::Append { count: 1 }
///     }
/// }
///
/// impl CustomSeries for Ranges {
///     fn kind(&self) -> &'static str {
///         "example.ranges"
///     }
///     fn axis_mode(&self) -> AxisMode {
///         AxisMode::Slots
///     }
///     fn times(&self) -> &[Timestamp] {
///         &self.time
///     }
///     fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
///         self.index.query(from, to)
///     }
///     fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
///         let w = (ctx.bar_spacing().0 * 0.6).max(1.0);
///         for i in from..to {
///             let (lo, hi) = self.range[i];
///             let (Some(x), Some(y0), Some(y1)) =
///                 (ctx.x_of_point(i), ctx.coords().y_of(hi), ctx.coords().y_of(lo))
///             else {
///                 continue;
///             };
///             let rect = RectPx { x: x - w / 2.0, y: y0, w, h: (y1 - y0).max(1.0) };
///             out.rect(rect, ctx.style().up);
///         }
///     }
/// }
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let id = chart.add_custom_series(
///     chart.main_pane(),
///     Box::new(Ranges::default()),
///     SeriesOptions::default(),
/// )?;
/// chart.update_custom(id, |r: &mut Ranges| r.push(60_000, 1.0, 2.0))?;
/// chart.update_custom(id, |r: &mut Ranges| r.push(120_000, 1.5, 3.0))?;
/// assert_eq!(chart.custom_series::<Ranges>(id).map(|r| r.time.len()), Some(2));
/// chart.build_frame(0.0, &MonoMeasurer);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub trait CustomSeries: Send + Any {
    /// Kind name, e.g. `"orderflow.heatmap"`; renderers that draw it natively
    /// declare this name.
    fn kind(&self) -> &'static str;
    /// How the points are placed on the time axis; read once, when added.
    fn axis_mode(&self) -> AxisMode;
    /// Times of the points, strictly increasing.
    fn times(&self) -> &[Timestamp];
    /// Price extremes of points `from..to`, for auto scaling. Called every
    /// frame, so it must be O(log n) (see [`crate::plugin::MinMaxIndex`]).
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)>;
    /// Portable path: writes the primitives of points `from..to` (the
    /// visible ones and one more on each side). Must not allocate once
    /// warmed up.
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>);
    /// Value of the last-value label; `None` hides it.
    fn last_value(&self) -> Option<f64> {
        None
    }
    /// Value of point `point` for the crosshair and magnet.
    fn crosshair_value(&self, point: usize) -> Option<f64> {
        let _ = point;
        None
    }
    /// Parameters of the native GPU path, e.g. a color-scale range.
    fn native_params(&self, ctx: &SeriesCtx<'_>, params: &mut [f32; 8]) {
        let _ = (ctx, params);
    }
}

/// What a [`CustomSeries`] knows about the chart while it draws.
pub struct SeriesCtx<'a> {
    pub(crate) coords: DrawingCtx<'a>,
    pub(crate) store: &'a CustomStore,
    /// `SeriesOptions::shift` in bars.
    pub(crate) shift: f64,
    pub(crate) visible: (usize, usize),
    pub(crate) style: SeriesStyle,
}

impl<'a> SeriesCtx<'a> {
    /// Time/price conversions of the pane, with this series' price map (on
    /// relative scales each series has its own), fonts and price
    /// formatting.
    pub fn coords(&self) -> &DrawingCtx<'a> {
        &self.coords
    }

    /// x of point `i` (the center of its slot); `None` if it has none.
    pub fn x_of_point(&self, i: usize) -> Option<f32> {
        let index = self.store.x_index(self.coords.index, i)? + self.shift;
        let x = f64::from(self.coords.plot.x) + self.coords.time_scale.index_to_x_f64(index);
        x.is_finite().then_some(x as f32)
    }

    /// The points being drawn, `from..to` (the visible ones and one more on
    /// each side).
    pub fn visible(&self) -> (usize, usize) {
        self.visible
    }

    /// Distance between two slots of the time axis.
    pub fn bar_spacing(&self) -> Px {
        Px(self.coords.time_scale.bar_spacing() as f32)
    }

    /// Colors and line settings resolved from the series options and the
    /// theme (`up`/`down` are `color`/`down_color` or the theme's).
    pub fn style(&self) -> &SeriesStyle {
        &self.style
    }
}

/// A custom series in the chart: the plugin and where its points are.
pub(crate) struct CustomStore {
    pub(crate) plugin: Box<dyn CustomSeries>,
    pub(crate) kind: &'static str,
    pub(crate) mode: AxisMode,
    /// `Slots`: logical index of each point (`Mapped` has `len` offsets).
    mapping: SeriesMapping,
    /// Points core has accepted (the plugin's data may be longer only
    /// between a change and its sync).
    len: usize,
    /// Times of the first and last accepted point.
    bounds: Option<(Timestamp, Timestamp)>,
    /// Changes by point number (`CustomDraw::points` is in point numbers).
    pub(crate) log: ChangeLog,
}

impl fmt::Debug for CustomStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomStore")
            .field("kind", &self.kind)
            .field("mode", &self.mode)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

/// How `Chart::update_custom` applies a change after checking it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CustomChange {
    /// Nothing to do.
    None,
    UpdateLast,
    Append,
    Prepend {
        count: usize,
    },
    Replace,
}

impl CustomStore {
    pub(crate) fn new(plugin: Box<dyn CustomSeries>) -> Self {
        Self {
            kind: plugin.kind(),
            mode: plugin.axis_mode(),
            plugin,
            mapping: SeriesMapping::Aligned { first: 0 },
            len: 0,
            bounds: None,
            log: ChangeLog::new(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The accepted times.
    pub(crate) fn times(&self) -> &[Timestamp] {
        let t = self.plugin.times();
        t.get(..self.len).unwrap_or(t)
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        match &self.mapping {
            SeriesMapping::Aligned { .. } => 0,
            SeriesMapping::Mapped { offsets, .. } => offsets.capacity() * 4,
        }
    }

    /// Logical index of point `i` (`Slots`).
    fn logical(&self, i: usize) -> Option<i64> {
        if i >= self.len {
            return None;
        }
        self.mapping.logical_index(i)
    }

    /// Logical indices `first..end` the points cover (`Slots` only).
    pub(crate) fn span(&self) -> Option<(i64, i64)> {
        if self.mode != AxisMode::Slots || self.len == 0 {
            return None;
        }
        Some((self.logical(0)?, self.logical(self.len - 1)? + 1))
    }

    /// The point in slot `j` (`Slots` only).
    pub(crate) fn point_at(&self, j: i64) -> Option<usize> {
        if self.mode != AxisMode::Slots {
            return None;
        }
        match &self.mapping {
            SeriesMapping::Aligned { first } => {
                let p = usize::try_from(j.checked_sub(*first)?).ok()?;
                (p < self.len).then_some(p)
            }
            SeriesMapping::Mapped { first, offsets } => {
                let off = u32::try_from(j.checked_sub(*first)?).ok()?;
                offsets.get(..self.len)?.binary_search(&off).ok()
            }
        }
    }

    /// Fractional logical index of point `i`.
    pub(crate) fn x_index(&self, index: &TimeIndex, i: usize) -> Option<f64> {
        match self.mode {
            AxisMode::Slots => self.logical(i).map(|j| j as f64),
            AxisMode::Overlay => index_of_time(index, *self.times().get(i)?),
        }
    }

    /// Points in slots `from..to` (`Slots`).
    fn slot_points(&self, from: i64, to: i64) -> (usize, usize) {
        let n = self.len;
        let (a, b) = match &self.mapping {
            SeriesMapping::Aligned { first } => {
                let rel = |j: i64| {
                    usize::try_from(j.saturating_sub(*first).max(0))
                        .unwrap_or(n)
                        .min(n)
                };
                (rel(from), rel(to))
            }
            SeriesMapping::Mapped { first, offsets } => {
                let offsets = offsets.get(..n).unwrap_or(offsets);
                let (lo, hi) = (from.saturating_sub(*first), to.saturating_sub(*first));
                (
                    offsets.partition_point(|&o| i64::from(o) < lo),
                    offsets.partition_point(|&o| i64::from(o) < hi),
                )
            }
        };
        (a, b.max(a))
    }

    /// Points in time range `t0..=t1` (`Overlay`).
    fn time_points(&self, t0: Timestamp, t1: Timestamp) -> (usize, usize) {
        let times = self.times();
        let a = times.partition_point(|&t| t < t0);
        (a, times.partition_point(|&t| t <= t1).max(a))
    }

    /// Exact points positioned in logical range `from..=to`.
    fn points_between(&self, index: &TimeIndex, from: f64, to: f64) -> (usize, usize) {
        if self.len == 0 || from.partial_cmp(&to).is_none_or(core::cmp::Ordering::is_gt) {
            return (0, 0);
        }
        match self.mode {
            AxisMode::Slots => self.slot_points(from.ceil() as i64, to.floor() as i64 + 1),
            AxisMode::Overlay => match (time_of_index(index, from), time_of_index(index, to)) {
                (Some(t0), Some(t1)) => self.time_points(t0, t1),
                _ => (0, 0),
            },
        }
    }

    /// Points positioned in logical range `from..=to`, plus one on each side.
    pub(crate) fn points_in(&self, index: &TimeIndex, from: f64, to: f64) -> (usize, usize) {
        if self.len == 0 {
            return (0, 0);
        }
        let (a, b) = self.points_between(index, from, to);
        (a.saturating_sub(1), (b + 1).min(self.len))
    }

    /// Extremes of the points in logical range `from..to`.
    pub(crate) fn extremes(&self, index: &TimeIndex, from: i64, to: i64) -> Option<(f64, f64)> {
        let (a, b) = self.points_between(index, from as f64, (to - 1) as f64);
        if a >= b {
            return None;
        }
        self.plugin.extremes(a, b)
    }

    /// The crosshair value at slot `j`.
    pub(crate) fn value_at(&self, j: i64) -> Option<f64> {
        let v = self.plugin.crosshair_value(self.point_at(j)?)?;
        v.is_finite().then_some(v)
    }

    /// Checks the plugin's data after `change` and says how to apply it. A
    /// change that does not match the data (wrong count, the last time moved)
    /// is applied as a `Replace`.
    pub(crate) fn classify(&self, change: crate::data::DataChange) -> Result<CustomChange> {
        use crate::data::DataChange as D;
        let times = self.plugin.times();
        let (n, old) = (times.len(), self.len);
        let change = match (change, self.bounds) {
            (D::UpdateLast, Some((_, last))) if n == old && times.last() == Some(&last) => {
                CustomChange::UpdateLast
            }
            (D::Append { count: 0 } | D::Prepend { count: 0 }, _) if n == old => CustomChange::None,
            (D::Append { count }, Some((_, last))) if n == old + count => {
                if times.get(old - 1) == Some(&last) {
                    check_increasing(&times[old - 1..], old - 1)?;
                    CustomChange::Append
                } else {
                    CustomChange::Replace
                }
            }
            (D::Prepend { count }, Some((first, _))) if n == old + count => {
                if times.get(count) == Some(&first) {
                    check_increasing(&times[..=count], 0)?;
                    CustomChange::Prepend { count }
                } else {
                    CustomChange::Replace
                }
            }
            _ => CustomChange::Replace,
        };
        if change == CustomChange::Replace {
            check_increasing(times, 0)?;
        }
        Ok(change)
    }

    /// Accepts all of the plugin's points, with `mapping` (`Slots`).
    pub(crate) fn accept(&mut self, mapping: SeriesMapping) {
        self.mapping = mapping;
        self.take_all();
    }

    /// Accepts all of the plugin's points with the current mapping.
    fn take_all(&mut self) {
        let times = self.plugin.times();
        self.len = times.len();
        self.bounds = times.first().copied().zip(times.last().copied());
    }

    /// Maps all points onto `index` (`Slots`) and accepts them.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if a time is not a slot of `index`;
    /// [`Error::OutOfMemory`].
    pub(crate) fn map_onto(&mut self, index: &TimeIndex) -> Result<()> {
        let mapping = if self.mode == AxisMode::Slots {
            index
                .map_series(self.plugin.times())?
                .ok_or(Error::InvalidArgument(
                    "custom series times are not on the axis",
                ))?
        } else {
            SeriesMapping::Aligned { first: 0 }
        };
        self.accept(mapping);
        Ok(())
    }

    /// Maps the points after the accepted ones onto `index` (whose slots
    /// they already are) and accepts them.
    pub(crate) fn extend_onto(&mut self, index: &TimeIndex) -> Result<()> {
        if self.mode == AxisMode::Slots {
            let times = self.plugin.times();
            let new = times.get(self.len..).unwrap_or(&[]);
            for (p, &t) in (self.len..).zip(new) {
                let j = index.index_of(t).ok_or(Error::InvalidArgument(
                    "custom series times are not on the axis",
                ))?;
                match &mut self.mapping {
                    SeriesMapping::Aligned { first } if p == 0 => *first = j,
                    SeriesMapping::Aligned { first }
                        if Some(j) == first.checked_add(i64::try_from(p).unwrap_or(i64::MAX)) => {}
                    SeriesMapping::Aligned { first } => {
                        let first = *first;
                        let mut offsets = Vec::new();
                        offsets.try_reserve(p + 1).map_err(|_| Error::OutOfMemory)?;
                        offsets.extend((0..p).filter_map(|o| u32::try_from(o).ok()));
                        offsets.push(offset(first, j)?);
                        self.mapping = SeriesMapping::Mapped { first, offsets };
                    }
                    SeriesMapping::Mapped { first, offsets } => {
                        let o = offset(*first, j)?;
                        offsets.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
                        offsets.push(o);
                    }
                }
            }
        }
        self.take_all();
        Ok(())
    }

    /// Drops all points (after invalid data): nothing is drawn until the
    /// next valid change.
    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.bounds = None;
        self.mapping = SeriesMapping::Aligned { first: 0 };
        self.log.replace();
    }
}

fn offset(first: i64, j: i64) -> Result<u32> {
    j.checked_sub(first)
        .and_then(|o| u32::try_from(o).ok())
        .ok_or(Error::InvalidArgument(
            "mapped series spans more than u32::MAX slots",
        ))
}

/// `Err(TimeNotIncreasing)` with the index in the plugin's data (`times`
/// starts at point `base`).
fn check_increasing(times: &[Timestamp], base: usize) -> Result<()> {
    match times.windows(2).position(|w| w[1] <= w[0]) {
        Some(k) => Err(Error::TimeNotIncreasing {
            index: base + k + 1,
        }),
        None => Ok(()),
    }
}
