//! `Chart`: the public object that owns data, scales and layout and builds frames.
//!
//! Scope so far: any number of panes holding series of the built-in
//! kinds. All series share one time axis, the union of their times;
//! volume series read the volume column of an OHLC series.
//!
//! `build_frame` is a hot path: once the frame pools and scratch buffers have
//! warmed up, panning, zooming and real-time updates do not allocate.
//!

use crate::data::lod::{bucket_of, bucket_start};
use core::any::Any;

use crate::data::{
    BandId, BarUpdate, ChangeLog, Column, DataChange, DataDelta, DrawingId, IdxSpan, IndicatorId,
    LayerPluginId, LogicalIndex, OhlcLevelView, PaneId, PriceLineId, SeriesId, TimeIndex,
    Timestamp, ValidationMode,
};
use crate::drawing::{DrawingCtx, DrawingManager, FrameBuilder};
use crate::error::{Error, Result};
use crate::format::NumberFormat;
use crate::format::auto_decimals;
use crate::frame::{
    CustomDraw, DrawCmd, FontId, FontStyleRef, Frame, Invalidation, LabelBg, LayerId, LineDash,
    LineInst, Reveal, SeriesDraw, SeriesKind, SeriesStyle, StrokeStyle, TextAlign, TextRun,
    XTransform, YTransform,
};
use crate::geom::{Css, PointCss, PointPx, Px, RectPx, Rgba, SizeCss, SizePx, Span};
use crate::interaction::{CrosshairMode, Drag, Interaction};
use crate::pane::{PRICE_MARGINS, Pane, PaneLayout, PaneOptions, SEPARATOR_PX, stack};
use crate::plugin::{
    AxisMode, CustomChange, CustomSeries, CustomStore, Indicator, IndicatorInput, IndicatorOutputs,
    LayerCtx, LayerPlugin, LayerPosition, OutputKind, Placement, SeriesCtx, SeriesTransform,
};
use crate::price_scale::{Normalize, PriceScale, ScaleSide};
use crate::series::{Op, Store, is_ohlc_kind, is_value_kind};
use crate::sync::{SyncMessage, SyncState};
use crate::text::{StackStr, TextMeasurer, text_width};
use crate::theme::Theme;
use crate::time_scale::{
    TimeScale, TimeTick, TimeZoneProvider, Utc, collect_ticks, format_crosshair_time, format_tick,
};
use crate::transform::store::{TransformStore, input_columns};

/// Options of a new chart.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::theme::Theme;
///
/// let chart = Chart::new(ChartOptions { theme: Theme::dark(), ..ChartOptions::default() });
/// # let _ = chart;
/// ```
pub struct ChartOptions {
    /// Colors and sizes.
    pub theme: Theme,
    /// Time zone for time labels.
    pub time_zone: Box<dyn TimeZoneProvider>,
    /// Handling of bars that violate the OHLC constraints.
    pub ohlc_validation: ValidationMode,
    /// Initial bar spacing.
    pub bar_spacing: Css,
    /// Lower bound of the bar spacing (further limited by the data size).
    pub min_bar_spacing: Css,
    /// Upper bound of the bar spacing.
    pub max_bar_spacing: Css,
    /// Initial empty space right of the last bar, in bars.
    pub right_offset: f64,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            theme: Theme::light(),
            time_zone: Box::new(Utc),
            ohlc_validation: ValidationMode::Reject,
            bar_spacing: Css(6.0),
            min_bar_spacing: Css(0.5),
            max_bar_spacing: Css(50.0),
            right_offset: 3.0,
        }
    }
}

pub use crate::series::SeriesData;

/// Options of a series. Colors left at `None` come from the theme.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::SeriesOptions;
/// use ferrochart_core::data::SeriesId;
/// use ferrochart_core::geom::{Css, Rgba};
///
/// let opts = SeriesOptions::volume_of(SeriesId(0));
/// assert_eq!(opts.source, Some(SeriesId(0)));
///
/// let line = SeriesOptions {
///     color: Some(Rgba::hex(0x00FF_6D00)),
///     line_width: Css(1.0),
///     ..SeriesOptions::default()
/// };
/// # let _ = line;
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesOptions {
    /// Decimal places of price labels; `None` (the default) is automatic:
    /// at least 2 decimals and 4 significant digits of the visible values,
    /// with a subscript count of zeros for small prices (`0.0₄1234`).
    pub price_precision: Option<u8>,
    /// For `SeriesKind::Volume`: the OHLC series whose volume is drawn.
    pub source: Option<SeriesId>,
    /// Main color: the line of `Line`/`Area`, the bars of `Histogram`, up
    /// candles and bars, the line of `Baseline` above its base.
    pub color: Option<Rgba>,
    /// Down candles and bars; the line of `Baseline` below its base.
    pub down_color: Option<Rgba>,
    /// Fill at the top of an `Area`; fill above the base of a `Baseline`.
    pub top_fill: Option<Rgba>,
    /// Fill at the bottom of an `Area`; fill below the base of a `Baseline`.
    pub bottom_fill: Option<Rgba>,
    /// Line width of `Line`, `Area` and `Baseline`.
    pub line_width: Css,
    /// Dash pattern of the line.
    pub line_style: LineDash,
    /// Step line: horizontal to the next point's time, then vertical.
    pub step: bool,
    /// Base value of `Baseline` (fill boundary) and `Histogram` (bar base).
    pub base_value: f64,
    /// Label with the last value on the price axis. Not drawn for
    /// volume series.
    pub last_value_visible: bool,
    /// Dotted horizontal line at the last value. Not drawn for volume
    /// series.
    pub price_line_visible: bool,
    /// Display shift in bars: the point at logical index `j` is drawn at
    /// `j + shift` (Ichimoku's leading spans and lagging line).
    /// Autoscale and the crosshair follow the drawn positions.
    pub shift: i32,
    /// Most markers drawn on one side of a bar (r21): beyond it the
    /// last place shows a "+k" badge for the `k` markers not drawn. `None`
    /// (the default) draws all; `Some(0)` is invalid.
    pub marker_stack_limit: Option<u16>,
    /// Cross-bar clustering of markers (r22): with `Some(d)`, bars
    /// fall into buckets of `ceil(d / bar spacing)` logical indices
    /// (aligned to the index, so panning does not regroup them); a side of
    /// a bucket whose markers sit on two or more bars shows its first marker
    /// and a "+k" badge, on the bar with the most of them. `AtPrice` markers
    /// are not clustered. `None` (the default) does not cluster; non-finite
    /// or <= 0 is invalid.
    pub marker_cluster: Option<Css>,
    /// Color of bars flagged [`BarFlags::FILLED`] (r22), drawn
    /// hollow; `None` (the default) is the main color at 40% opacity.
    pub filled_bar_color: Option<Rgba>,
}

impl Default for SeriesOptions {
    fn default() -> Self {
        Self {
            price_precision: None,
            source: None,
            color: None,
            down_color: None,
            top_fill: None,
            bottom_fill: None,
            line_width: Css(2.0),
            line_style: LineDash::Solid,
            step: false,
            base_value: 0.0,
            last_value_visible: true,
            price_line_visible: true,
            shift: 0,
            marker_stack_limit: None,
            marker_cluster: None,
            filled_bar_color: None,
        }
    }
}

/// Colors of a band between two series.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::{BandOptions, SeriesData, SeriesOptions};
/// use ferrochart_core::data::ValueColumns;
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::geom::Rgba;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let pane = chart.main_pane();
/// let a = chart.add_series(pane, SeriesKind::Line, SeriesOptions::default())?;
/// let b = chart.add_series(pane, SeriesKind::Line, SeriesOptions::default())?;
/// let line = |v: Vec<f64>| SeriesData::Value(ValueColumns { time: vec![0, 60_000, 120_000], value: v, color: None });
/// chart.set_data(a, line(vec![2.0, 3.0, 1.0]))?;
/// chart.set_data(b, line(vec![1.0, 1.0, 2.0]))?;
/// // Green where a is above b, red where it is below.
/// let band = chart.add_band(a, b, BandOptions {
///     fill: Some(Rgba::hex(0x0026_A69A).with_alpha(0x40)),
///     fill_crossed: Some(Rgba::hex(0x00EF_5350).with_alpha(0x40)),
/// })?;
/// assert!(chart.band_options(band).is_some());
/// chart.remove_band(band)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BandOptions {
    /// Fill where the upper series is at or above the lower one; `None`: the
    /// upper series' color at 15% opacity.
    pub fill: Option<Rgba>,
    /// Fill where the upper series is below the lower one; `None`: as `fill`.
    pub fill_crossed: Option<Rgba>,
}

/// Whether a series is drawn on the pane's volume scale: volume series and
/// lines placed with `Placement::Volume`.
fn volume_like(e: &SeriesEntry) -> bool {
    e.kind == SeriesKind::Volume || e.volume_scale
}

/// A marker drawn in the last frame, for click and hover hit tests.
#[derive(Clone, Copy, Debug)]
struct MarkerSpot {
    /// `Marker` or `MarkerOverflow`.
    target: ClickTarget,
    /// The bars it stands for.
    bars: IdxSpan,
    pane: PaneId,
    pos: PointPx,
    radius: f32,
}

/// Hit tolerance of lines (CSS px).
const HIT_TOLERANCE: f32 = 4.0;

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: PointPx, a: PointPx, b: PointPx) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + dx * t)).hypot(p.y - (a.y + dy * t))
}

/// A band.
#[derive(Clone, Copy, Debug)]
struct BandEntry {
    id: BandId,
    upper: SeriesId,
    lower: SeriesId,
    opts: BandOptions,
}

/// A horizontal line at a price of a series, with an optional label on the
/// price axis.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::{PriceLineOptions, SeriesOptions};
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let id = chart.add_series(chart.main_pane(), SeriesKind::Line, SeriesOptions::default())?;
/// let stop = chart.create_price_line(id, PriceLineOptions {
///     price: 95.0,
///     title: "stop".into(),
///     ..PriceLineOptions::default()
/// })?;
/// chart.remove_price_line(stop)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct PriceLineOptions {
    /// Price, in the series' units.
    pub price: f64,
    /// Line and label color; `None` uses the series' main color.
    pub color: Option<Rgba>,
    /// Line width.
    pub line_width: Css,
    /// Dash pattern.
    pub line_style: LineDash,
    /// Whether the price is labelled on the price axis.
    pub axis_label_visible: bool,
    /// Text drawn above the line at the right edge of the plot; empty for none.
    pub title: String,
}

impl Default for PriceLineOptions {
    fn default() -> Self {
        Self {
            price: 0.0,
            color: None,
            line_width: Css(1.0),
            line_style: LineDash::Dashed,
            axis_label_visible: true,
            title: String::new(),
        }
    }
}

/// Where a [`Marker`] sits relative to its bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkerPosition {
    /// Above the bar's high (above the value for single-value series).
    #[default]
    AboveBar,
    /// Below the bar's low.
    BelowBar,
    /// Centered on [`Marker::price`] at the bar's x (r22): not stacked,
    /// folded or clustered; its text is on the right.
    AtPrice,
}

/// Shape of a [`Marker`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkerShape {
    /// Filled circle.
    #[default]
    Circle,
    /// Filled square.
    Square,
    /// Arrow pointing up.
    ArrowUp,
    /// Arrow pointing down.
    ArrowDown,
}

impl MarkerShape {
    fn icon(self) -> u16 {
        use crate::transform_helpers::{ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CIRCLE, ICON_SQUARE};
        match self {
            Self::Circle => ICON_CIRCLE,
            Self::Square => ICON_SQUARE,
            Self::ArrowUp => ICON_ARROW_UP,
            Self::ArrowDown => ICON_ARROW_DOWN,
        }
    }
}

/// An icon with optional text above or below a bar, e.g. a buy or sell
/// signal. Anchored by time; markers on the same bar
/// and side stack away from it in the order given.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::{Marker, MarkerPosition, MarkerShape, SeriesOptions};
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
/// chart.set_markers(id, vec![Marker {
///     time: 1_700_000_000_000,
///     position: MarkerPosition::BelowBar,
///     shape: MarkerShape::ArrowUp,
///     text: "buy".into(),
///     ..Marker::default()
/// }])?;
/// assert_eq!(chart.markers(id).map(<[Marker]>::len), Some(1));
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// Time of the bar. A time that is not on the time axis snaps to the next
    /// bar (the last one for times after the end).
    pub time: Timestamp,
    /// Above or below the bar.
    pub position: MarkerPosition,
    /// Icon.
    pub shape: MarkerShape,
    /// Icon and text color; `None` uses the series' main color.
    pub color: Option<Rgba>,
    /// Text next to the icon, on the side away from the bar; empty for none.
    pub text: String,
    /// Diameter (side of a square), 4..=64 CSS px; `None` for 10 (r21).
    pub size: Option<Css>,
    /// Image clipped to the shape, over the fill (circles and squares; r21).
    /// An unknown or removed image draws the shape alone.
    pub icon: Option<crate::icon::IconId>,
    /// Short text centered in the shape (circles and squares, at most 8
    /// characters, not with `icon`; r21), e.g. `"DB"`.
    pub label: String,
    /// Border color, inside the shape's edge (r21).
    pub border: Option<Rgba>,
    /// Border width; default 2 CSS px. Used when `border` is set.
    pub border_width: Css,
    /// The host's own identifier, 0 for none (r22). Reported back in
    /// [`MarkerHit`] and [`ClickTarget`]: their `index` is the position after
    /// sorting by time, which the host cannot map back reliably. Not checked
    /// for uniqueness.
    pub id: u64,
    /// Price of a [`MarkerPosition::AtPrice`] marker, in series units
    /// (normalized with the series on relative scales, like price lines);
    /// `None` for the other positions (r22).
    pub price: Option<f64>,
}

impl Default for Marker {
    fn default() -> Self {
        Self {
            time: 0,
            position: MarkerPosition::default(),
            shape: MarkerShape::default(),
            color: None,
            text: String::new(),
            size: None,
            icon: None,
            label: String::new(),
            border: None,
            border_width: Css(2.0),
            id: 0,
            price: None,
        }
    }
}

/// Quality of a bar (r22), set sparsely with
/// [`Chart::set_bar_flags`]. A bar without trades is a gap bar (all prices
/// NaN), not a flag.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::BarFlags;
///
/// let f = BarFlags::FILLED | BarFlags::CLIPPED;
/// assert!(f.contains(BarFlags::CLIPPED));
/// assert!(!BarFlags::NONE.contains(BarFlags::FILLED));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarFlags(pub u8);

impl BarFlags {
    /// A real bar.
    pub const NONE: Self = Self(0);
    /// Filled in by the data source (no real trades, usually O=H=L=C):
    /// drawn hollow in `SeriesOptions::filled_bar_color`.
    pub const FILLED: Self = Self(1);
    /// The host clipped an outlier wick; the bar holds the clipped values.
    /// A short tick marks each end of the wick.
    pub const CLIPPED: Self = Self(2);
    /// All flags.
    pub const ALL: Self = Self(3);

    /// Whether all flags of `other` are set.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for BarFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Most bar flags per series.
const MAX_BAR_FLAGS: usize = 1_000_000;

/// Marker icon size and gap to the bar or the next marker, in CSS pixels.
const MARKER_SIZE: f32 = 10.0;
const MARKER_GAP: f32 = 4.0;
/// Smallest and largest marker size, CSS px (r21).
const MARKER_SIZE_RANGE: core::ops::RangeInclusive<f32> = 4.0..=64.0;
/// Most characters of a marker label.
const MARKER_LABEL_CHARS: usize = 8;
/// Smallest label font, CSS px.
const MARKER_LABEL_MIN_FONT: f32 = 7.0;

/// Markers of one side of a bucket of bars, while they are drawn:
/// a bucket is one bar unless `marker_cluster` is set.
#[derive(Clone, Copy)]
struct MarkerGroup {
    /// Bucket: logical index `div_euclid` bars per bucket.
    key: i64,
    /// Markers of the side on bars that are not gaps.
    total: u32,
    /// The markers sit on two or more bars of a clustering bucket.
    clustered: bool,
    /// The bar drawn on: the one with the most markers (the earliest of
    /// equals).
    at: i64,
    /// The bars of the markers.
    bars: IdxSpan,
    /// Markers drawn so far.
    drawn: u32,
    /// Stack offset taken so far, px.
    off: f32,
}

impl MarkerGroup {
    const EMPTY: Self = Self {
        key: i64::MIN,
        total: 0,
        clustered: false,
        at: 0,
        bars: IdxSpan { start: 0, end: 0 },
        drawn: 0,
        off: 0.0,
    };

    /// A group of one bar `j` whose markers need not be counted (no
    /// clustering, no stack limit).
    fn single(j: i64) -> Self {
        Self {
            key: j,
            at: j,
            bars: IdxSpan {
                start: j,
                end: j + 1,
            },
            ..Self::EMPTY
        }
    }

    /// The group of side `side` starting at `markers[0]` (sorted by time,
    /// on bar `j0`): counts forward through the markers of bucket `key`.
    /// `ix` maps times to bars (markers snap to the next bar) and back, and
    /// tells gaps; bars are looked up only when the time passes the current
    /// one.
    fn scan(markers: &[Marker], side: usize, kb: i64, j0: i64, ix: &MarkerBars<'_>) -> Self {
        let key = j0.div_euclid(kb);
        // Markers up to this time snap into the bucket.
        let t_last = ix.last_time((key + 1) * kb - 1);
        let mut g = Self { key, ..Self::EMPTY };
        let (mut j, mut t_j, mut ok) = (j0, ix.time(j0), ix.has_bar(j0));
        let (mut run_bar, mut run, mut best, mut distinct) = (i64::MIN, 0_u32, 0_u32, 0_u32);
        for m in markers {
            if m.time > t_last {
                break;
            }
            if m.time > t_j {
                j = ix.bar_of(m.time);
                (t_j, ok) = (ix.time(j), ix.has_bar(j));
            }
            if !ok || stack_side(m.position) != Some(side) {
                continue;
            }
            if j != run_bar {
                (run_bar, run) = (j, 0);
                distinct += 1;
                if distinct == 1 {
                    g.bars.start = j;
                }
            }
            run += 1;
            if run > best {
                (best, g.at) = (run, j);
            }
            g.bars.end = j + 1;
            g.total += 1;
        }
        g.clustered = kb > 1 && distinct >= 2;
        g
    }
}

/// Bars of markers: times to bars (snapping to the next bar, the
/// last one after the end), bar times, and gaps.
struct MarkerBars<'a> {
    index: &'a TimeIndex,
    store: &'a Store,
    end: i64,
}

impl MarkerBars<'_> {
    fn bar_of(&self, t: Timestamp) -> i64 {
        self.index.lower_bound(t).min(self.end - 1)
    }

    /// Time of bar `j`; the last bar takes every later time.
    fn time(&self, j: i64) -> Timestamp {
        if j >= self.end - 1 {
            return Timestamp::MAX;
        }
        self.index.time_at(j).unwrap_or(Timestamp::MAX)
    }

    /// The last time that snaps to bar `j` or before.
    fn last_time(&self, j: i64) -> Timestamp {
        self.time(j.min(self.end - 1))
    }

    fn has_bar(&self, j: i64) -> bool {
        self.store.bar_at(j).is_some()
    }
}

/// The stacking side of a marker position (0 above, 1 below); `None` for
/// `AtPrice`, which does not stack (r22).
fn stack_side(p: MarkerPosition) -> Option<usize> {
    match p {
        MarkerPosition::AboveBar => Some(0),
        MarkerPosition::BelowBar => Some(1),
        MarkerPosition::AtPrice => None,
    }
}

/// Checks the r21 and r22 marker fields.
fn check_marker(m: &Marker) -> Result<()> {
    if let Some(s) = m.size
        && !(s.0.is_finite() && MARKER_SIZE_RANGE.contains(&s.0))
    {
        return Err(Error::InvalidArgument("marker size must be 4..=64 CSS px"));
    }
    let arrow = matches!(m.shape, MarkerShape::ArrowUp | MarkerShape::ArrowDown);
    if arrow && (m.icon.is_some() || !m.label.is_empty()) {
        return Err(Error::InvalidArgument(
            "marker icon and label need a circle or square shape",
        ));
    }
    if m.icon.is_some() && !m.label.is_empty() {
        return Err(Error::InvalidArgument(
            "a marker has an icon or a label, not both",
        ));
    }
    if m.label.chars().count() > MARKER_LABEL_CHARS {
        return Err(Error::InvalidArgument(
            "marker label is at most 8 characters",
        ));
    }
    if !(m.border_width.0.is_finite() && m.border_width.0 >= 0.0) {
        return Err(Error::InvalidArgument(
            "marker border width must be finite and >= 0",
        ));
    }
    match (m.position, m.price) {
        (MarkerPosition::AtPrice, Some(p)) if p.is_finite() => {}
        (MarkerPosition::AtPrice, _) => {
            return Err(Error::InvalidArgument(
                "an atPrice marker needs a finite price",
            ));
        }
        (_, Some(_)) => {
            return Err(Error::InvalidArgument("only atPrice markers have a price"));
        }
        (_, None) => {}
    }
    Ok(())
}

/// Checks the options that have a restricted domain.
fn check_series_options(o: &SeriesOptions) -> Result<()> {
    if o.marker_stack_limit == Some(0) {
        return Err(Error::InvalidArgument(
            "marker_stack_limit must be at least 1",
        ));
    }
    if o.marker_cluster
        .is_some_and(|d| !(d.0.is_finite() && d.0 > 0.0))
    {
        return Err(Error::InvalidArgument(
            "marker_cluster must be finite and > 0",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct PriceLineEntry {
    id: PriceLineId,
    series: SeriesId,
    opts: PriceLineOptions,
}

/// A label on the price axis (last values, price lines).
#[derive(Clone, Copy, Debug)]
struct AxisLabel {
    /// Wanted center y.
    want: f32,
    /// Placed center y.
    at: f32,
    text: Span,
    bg: Rgba,
    fg: Rgba,
}

impl SeriesOptions {
    /// Options of a volume series that draws `source`'s volume column.
    pub fn volume_of(source: SeriesId) -> Self {
        Self {
            source: Some(source),
            ..Self::default()
        }
    }
}

/// Non-fatal problems reported through [`ChartEvent::Warning`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WarningKind {
    /// `ValidationMode::Repair` widened high/low of `bars` bars.
    OhlcRepaired {
        /// The series.
        series: SeriesId,
        /// Number of repaired bars.
        bars: usize,
    },
    /// Times of `series` fell between bars of the time axis, so the axis was
    /// rebuilt as the union of all series' times and the logical
    /// indices after the first inserted bar moved.
    TimeAxisMerged {
        /// The series whose data was added.
        series: SeriesId,
        /// Number of bars inserted between existing ones.
        slots: usize,
    },
    /// The data of `series` could not be moved onto a rebuilt time axis (out
    /// of memory) and was cleared.
    DataDropped {
        /// The series.
        series: SeriesId,
    },
}

/// OHLCV values of one bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarValues {
    /// Open.
    pub open: f64,
    /// High.
    pub high: f64,
    /// Low.
    pub low: f64,
    /// Close.
    pub close: f64,
    /// Volume, if the series has one.
    pub volume: Option<f64>,
}

/// Where the crosshair points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrosshairPoint {
    /// Logical index of the bar under the crosshair (may be outside the data).
    pub logical_index: i64,
    /// Time of that bar, `None` in the empty space beyond the data.
    pub time: Option<Timestamp>,
    /// The pane under the pointer.
    pub pane: PaneId,
    /// Price at the crosshair's y, on that pane's price scale.
    pub price: f64,
    /// Values of the first OHLC series that has a bar there.
    pub bar: Option<BarValues>,
}

/// What a click hit: the first in priority order (drawing, marker, series).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClickTarget {
    /// A drawing (its body or a handle).
    Drawing(DrawingId),
    /// Marker `index` of `series`, in the time order [`Chart::markers`]
    /// returns them.
    Marker {
        /// The series the marker belongs to.
        series: SeriesId,
        /// Its index in [`Chart::markers`].
        index: usize,
        /// Its [`Marker::id`] (r22).
        id: u64,
    },
    /// The "+k" badge of a folded marker stack (r21): `count` markers
    /// on its side of the bars in `bars` are not drawn, the first of them
    /// being index `first` of [`Chart::markers`].
    MarkerOverflow {
        /// The series the markers belong to.
        series: SeriesId,
        /// The first marker not drawn.
        first: usize,
        /// How many are not drawn.
        count: u32,
        /// [`Marker::id`] of marker `first` (r22).
        id: u64,
        /// Logical indices of the bars whose markers the badge stands for
        /// (r22): one bar for a folded stack.
        bars: IdxSpan,
    },
    /// A series: a candle's body or wick, within 4 CSS px of a line, inside
    /// a histogram bar, or on a dot.
    Series(SeriesId),
}

/// Events for the host, pulled with [`Chart::drain_events`].
///
/// Pending `CrosshairMove` and `VisibleRangeChanged` events are coalesced: if
/// the host has not drained the previous one, it is replaced, so the queue
/// does not grow (and does not allocate) while the user interacts.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ChartEvent {
    /// The crosshair moved; `None` when it left the plot or is hidden.
    CrosshairMove {
        /// The new position.
        point: Option<CrosshairPoint>,
    },
    /// A press and release in a plot without moving more than the drag
    /// threshold (3 CSS px); not sent while a drawing is being created.
    Click {
        /// Where, in CSS pixels from the chart's top left.
        pos: PointCss,
        /// The pane clicked.
        pane: PaneId,
        /// Logical index at `pos` (fractional).
        logical_index: f64,
        /// Time of the bar nearest to `logical_index`; `None` beyond the data.
        time: Option<Timestamp>,
        /// Price at `pos`, on the pane's price scale.
        price: f64,
        /// What was clicked; `None` for empty space.
        target: Option<ClickTarget>,
    },
    /// The visible logical range changed.
    VisibleRangeChanged {
        /// Left edge.
        from: LogicalIndex,
        /// Right edge.
        to: LogicalIndex,
    },
    /// The left edge of the view is within one screen of the first bar: the
    /// host should `prepend` older data. Sent once per first-bar position.
    NeedMoreHistory {
        /// Bars between the first bar and the left edge (negative when the
        /// view already shows empty space before the data).
        bars_before_start: i64,
    },
    /// A drawing was created (by the user, `DrawingManager::add`, or an
    /// undo/redo that brought it back).
    DrawingCreated(DrawingId),
    /// A drawing was moved or restyled.
    DrawingModified(DrawingId),
    /// A drawing was removed.
    DrawingRemoved(DrawingId),
    /// The selection changed.
    DrawingSelected(Option<DrawingId>),
    /// A non-fatal problem.
    Warning(WarningKind),
    /// The chart posted a message to its sync group. Hosts that sync
    /// charts across workers forward it with [`Chart::apply_sync`]; charts
    /// in the same [`SyncGroup`](crate::sync::SyncGroup) need nothing.
    Sync(SyncMessage),
    /// The chart sent drawing sync operations; hosts that forward
    /// them to charts in other workers take them with
    /// [`Chart::take_drawing_sync_ops`]. Reported once until they are taken.
    DrawingSync,
    /// The marker under the pointer changed (r21): entered, left or
    /// another one. Coalesced like `CrosshairMove`. Hosts show their own
    /// tooltip.
    MarkerHover(Option<MarkerHit>),
}

/// The marker under the pointer, in [`ChartEvent::MarkerHover`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerHit {
    /// The series the marker belongs to.
    pub series: SeriesId,
    /// Its index in [`Chart::markers`]; for a "+k" badge the first marker
    /// not drawn.
    pub index: usize,
    /// [`Marker::id`] of that marker (r22).
    pub id: u64,
    /// For a "+k" badge, `k`; 0 for a marker.
    pub overflow: u32,
    /// Logical indices of the bars it stands for (r22): the marker's bar,
    /// or those of the markers a "+k" badge hides (on its side).
    pub bars: IdxSpan,
    /// Center, CSS px from the chart's top left.
    pub center: PointCss,
    /// Diameter, CSS px.
    pub size: Css,
}

/// Rectangles of the last built layout, in physical pixels. Per-pane
/// rectangles are available from [`Chart::pane_layout`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    /// The plot area of the main (top) pane.
    pub plot: RectPx,
    /// The right price axis, over all panes.
    pub price_axis: RectPx,
    /// The time axis at the bottom.
    pub time_axis: RectPx,
}

#[derive(Debug)]
struct SeriesEntry {
    id: SeriesId,
    pane: PaneId,
    kind: SeriesKind,
    opts: SeriesOptions,
    store: Store,
    /// Price-to-display map of the last frame (relative price scales);
    /// `None` when the series has no usable base value.
    norm: Option<Normalize>,
    /// Markers, sorted by time.
    markers: Vec<Marker>,
    /// Bar quality flags, sorted by time, one per time (r22).
    bar_flags: Vec<(Timestamp, BarFlags)>,
    /// An output of an indicator: follows its source, so it is left
    /// out of the time-axis union and recomputed after a rebuild.
    derived: bool,
    /// Drawn on the pane's volume scale (`Placement::Volume`).
    volume_scale: bool,
    /// Decimals of automatic precision from the last visible values.
    auto_decimals: u8,
}

/// The new data of the series being rebuilt.
enum Target<'a> {
    Data(SeriesData),
    /// A custom series, taken out of its entry: its plugin has the data.
    Custom(&'a mut CustomStore),
}

impl Target<'_> {
    fn times(&self) -> &[Timestamp] {
        match self {
            Self::Data(d) => d.times(),
            Self::Custom(c) => c.plugin.times(),
        }
    }
}

/// Segments of a Point & Figure O.
const CIRCLE_SEGMENTS: u16 = 16;

const NOT_CUSTOM: Error = Error::InvalidArgument("not a custom series");
const NEXT_TO_TRANSFORM: Error =
    Error::InvalidArgument("a transform series cannot share the chart with other series");

/// A layer plugin attached to the chart.
struct LayerEntry {
    id: LayerPluginId,
    /// `None`: the whole chart.
    pane: Option<PaneId>,
    position: LayerPosition,
    plugin: Box<dyn LayerPlugin>,
}

/// An indicator attached to the chart.
struct IndicatorEntry {
    id: IndicatorId,
    source: SeriesId,
    ind: Box<dyn Indicator>,
    /// The output series, in the order of `Indicator::outputs`.
    series: Vec<SeriesId>,
    /// The pane `add_indicator` created, removed with the indicator if empty.
    own_pane: Option<PaneId>,
    /// Bands of `OutputSpec::fill_to`.
    bands: Vec<BandId>,
}

/// Runs the indicator of `entry` for `change` into `out` (reads only the
/// chart, so several can run in parallel). Returns the change actually
/// computed (`Replace` when an incremental one does not fit) and the first
/// slot written.
fn compute_indicator(
    entry: &mut IndicatorEntry,
    source: crate::series::Source<'_>,
    index: &crate::data::TimeIndex,
    change: DataChange,
    out: &mut IndicatorOutputs,
) -> (DataChange, usize) {
    let input = IndicatorInput::new(source, index);
    let len = input.len();
    let change = match change {
        DataChange::Append { count } if count <= len => change,
        DataChange::UpdateLast if len > 0 => change,
        _ => DataChange::Replace,
    };
    let start = match change {
        DataChange::Append { count } => len - count,
        DataChange::UpdateLast => len - 1,
        _ => 0,
    };
    out.reset(entry.ind.outputs().len(), start, len);
    entry.ind.update(&input, change, out);
    (change, start)
}

/// Colors of indicator outputs without a default color.
const INDICATOR_PALETTE: [u32; 6] = [
    0x0029_62FF,
    0x00FF_9800,
    0x009C_27B0,
    0x0000_BCD4,
    0x00E9_1E63,
    0x0079_5548,
];

/// A placed time label.
#[derive(Clone, Copy, Debug)]
struct PlacedTick {
    x: f32,
    width: f32,
    tick: TimeTick,
}

/// A financial chart.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::chart::{SeriesData, SeriesOptions};
/// use ferrochart_core::frame::{Invalidation, SeriesKind};
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::{random_walk, MonoMeasurer};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// let pane = chart.main_pane();
/// let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
/// chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(candles))?;
/// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 500, 0, 60_000)?))?;
///
/// assert_ne!(chart.needs_redraw(), Invalidation::None);
/// chart.build_frame(0.0, &MonoMeasurer);
/// assert!(!chart.frame().layers[1].cmds.is_empty());
/// assert_eq!(chart.needs_redraw(), Invalidation::None);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub struct Chart {
    theme: Theme,
    tz: Box<dyn TimeZoneProvider>,
    validation: ValidationMode,
    default_right_offset: f64,
    pub(crate) default_bar_spacing: Css,
    size: SizeCss,
    pub(crate) dpr: f32,
    pub(crate) panes: Vec<Pane>,
    next_pane: u32,
    pub(crate) time_index: TimeIndex,
    series: Vec<SeriesEntry>,
    next_id: u32,
    pub(crate) time_scale: TimeScale,
    pub(crate) invalidation: Invalidation,
    revisions: [u64; 3],
    frame: Frame,
    pub(crate) layout: Layout,
    pub(crate) input: Interaction,
    last_range: (f64, f64),
    last_crosshair: Option<CrosshairPoint>,
    history_requested: Option<i64>,
    axis_width: f32,
    events: Vec<ChartEvent>,
    price_lines: Vec<PriceLineEntry>,
    indicators: Vec<IndicatorEntry>,
    next_indicator: u32,
    /// Colors handed out to indicator outputs so far.
    palette_next: usize,
    /// Scratch of indicator updates (kept small between full updates, so the
    /// real-time path does not allocate).
    ind_out: IndicatorOutputs,
    /// Set by `rebuild`: every indicator must be recomputed.
    axis_rebuilt: bool,
    next_price_line: u32,
    /// Axis labels of the pane being built (scratch).
    axis_labels: Vec<AxisLabel>,
    time_ticks: Vec<TimeTick>,
    placed: Vec<PlacedTick>,
    pub(crate) drawings: DrawingManager,
    /// Text metrics of the last frame, for hit tests between frames.
    pub(crate) text_cache: CachedText,
    /// Handles of the selected drawing (reused: no allocation per frame or
    /// hit test).
    pub(crate) handle_buf: core::cell::RefCell<Vec<PointPx>>,
    /// Multi-chart sync.
    pub(crate) sync: SyncState,
    /// Layer plugins, in the order they were added.
    layer_plugins: Vec<LayerEntry>,
    next_layer: u32,
    /// Custom series kinds the renderer draws natively.
    native_kinds: Vec<&'static str>,
    /// Bands between two series, drawn in this order.
    bands: Vec<BandEntry>,
    next_band: u32,
    /// Markers of the last frame (click hit tests).
    marker_hits: Vec<MarkerSpot>,
    /// The marker of the last `MarkerHover` event.
    last_marker_hover: Option<MarkerHit>,
    /// Registered images (r21).
    icons: crate::icon::IconStore,
    /// Gradual display of the data; `None`: everything.
    reveal: Option<Reveal>,
}

/// The ASCII and subscript advances and line height of the last frame's
/// font: hit tests of drawings run between frames, without the host's
/// measurer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CachedText {
    advances: [f32; 128],
    subscripts: [f32; 10],
    /// Line height per px of font size.
    line: f32,
}

impl Default for CachedText {
    fn default() -> Self {
        Self {
            advances: [0.6; 128],
            subscripts: [0.6; 10],
            line: 1.2,
        }
    }
}

impl TextMeasurer for CachedText {
    fn ascii_advances(&self, _: FontStyleRef) -> &[f32; 128] {
        &self.advances
    }

    fn subscript_advances(&self, _: FontStyleRef) -> Option<&[f32; 10]> {
        Some(&self.subscripts)
    }

    fn measure(&self, text: &str, style: FontStyleRef) -> SizePx {
        SizePx {
            w: text.chars().count() as f32 * 0.6 * style.size,
            h: self.line * style.size,
        }
    }

    fn line_height(&self, style: FontStyleRef) -> f32 {
        self.line * style.size
    }
}

/// Diameter of drawing handles (CSS px).
pub(crate) const HANDLE_CSS: f32 = 9.0;

/// Pool lengths before a drawing is built: what it adds is its bounds.
#[derive(Clone, Copy)]
struct PoolMark {
    lines: usize,
    rects: usize,
    points: usize,
    runs: usize,
}

impl PoolMark {
    fn of(frame: &Frame) -> Self {
        let p = &frame.pools;
        Self {
            lines: p.lines.len(),
            rects: p.rects.len(),
            points: p.points.len(),
            runs: p.text_runs.len(),
        }
    }

    /// Bounds of everything added since the mark (empty if nothing).
    fn bounds(self, frame: &Frame, ctx: &DrawingCtx<'_>) -> RectPx {
        let p = &frame.pools;
        let (mut x0, mut y0, mut x1, mut y1) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        let mut add = |x: f32, y: f32| {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        };
        for l in p.lines.get(self.lines..).unwrap_or(&[]) {
            add(l.from.x, l.from.y);
            add(l.to.x, l.to.y);
        }
        for r in p.rects.get(self.rects..).unwrap_or(&[]) {
            add(r.rect.x, r.rect.y);
            add(r.rect.right(), r.rect.bottom());
        }
        for q in p.points.get(self.points..).unwrap_or(&[]) {
            add(q.x, q.y);
        }
        for run in p.text_runs.get(self.runs..).unwrap_or(&[]) {
            let w = ctx.text_width(p.text_of(run.text), run.style);
            let h = ctx.line_height(run.style) * 0.5 + run.bg.map_or(0.0, |b| b.padding.0);
            let x = match run.align {
                TextAlign::Left => run.pos.x,
                TextAlign::Center => run.pos.x - w * 0.5,
                TextAlign::Right => run.pos.x - w,
            };
            add(x - h, run.pos.y - h);
            add(x + w + h, run.pos.y + h);
        }
        if x0 > x1 {
            return RectPx::default();
        }
        RectPx {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

/// Time tick candidates are sampled at least this far apart (physical px).
const TICK_SAMPLE_PX: f64 = 8.0;

impl Chart {
    /// Creates an empty chart with one pane.
    pub fn new(options: ChartOptions) -> Self {
        let dpr = 1.0;
        let mut frame = Frame::default();
        frame.reserve();
        Self {
            theme: options.theme,
            tz: options.time_zone,
            validation: options.ohlc_validation,
            default_right_offset: options.right_offset,
            default_bar_spacing: options.bar_spacing,
            size: SizeCss::default(),
            dpr,
            panes: vec![Pane::new(PaneId(0), 1.0)],
            next_pane: 1,
            time_index: TimeIndex::new(),
            series: Vec::new(),
            next_id: 0,
            time_scale: TimeScale::new(
                f64::from(options.bar_spacing.to_px(dpr).0),
                options.right_offset,
                f64::from(options.min_bar_spacing.to_px(dpr).0),
                f64::from(options.max_bar_spacing.to_px(dpr).0),
            ),
            invalidation: Invalidation::Full,
            revisions: [0; 3],
            frame,
            layout: Layout::default(),
            input: Interaction::default(),
            last_range: (f64::NAN, f64::NAN),
            last_crosshair: None,
            history_requested: None,
            axis_width: 0.0,
            events: Vec::with_capacity(64),
            price_lines: Vec::new(),
            indicators: Vec::new(),
            next_indicator: 0,
            palette_next: 0,
            ind_out: IndicatorOutputs::default(),
            axis_rebuilt: false,
            next_price_line: 0,
            axis_labels: Vec::with_capacity(64),
            time_ticks: Vec::with_capacity(2048),
            placed: Vec::with_capacity(256),
            drawings: DrawingManager::new(PaneId(0)),
            text_cache: CachedText::default(),
            handle_buf: core::cell::RefCell::new(Vec::new()),
            sync: SyncState::default(),
            layer_plugins: Vec::new(),
            next_layer: 0,
            bands: Vec::new(),
            next_band: 0,
            marker_hits: Vec::new(),
            last_marker_hover: None,
            icons: crate::icon::IconStore::default(),
            reveal: None,
            native_kinds: Vec::new(),
        }
    }

    /// Sets the size in CSS pixels and the device pixel ratio. All geometry is
    /// computed in physical pixels (`size × dpr`). Non-finite or non-positive
    /// values are ignored.
    pub fn resize(&mut self, size: SizeCss, dpr: f32) {
        let finite = size.w.is_finite() && size.h.is_finite() && dpr.is_finite();
        if !finite || dpr <= 0.0 || size.w < 0.0 || size.h < 0.0 {
            return;
        }
        if (dpr - self.dpr).abs() > f32::EPSILON {
            self.time_scale.rescale(f64::from(dpr / self.dpr));
            self.dpr = dpr;
        }
        self.size = size;
        self.invalidation = Invalidation::Full;
    }

    /// The top pane (the pane every chart starts with, unless it was removed).
    pub fn main_pane(&self) -> PaneId {
        self.panes.first().map_or(PaneId(0), |p| p.id)
    }

    /// Adds a pane below the existing ones.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Chart, ChartOptions};
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::SizeCss;
    /// use ferrochart_core::pane::PaneOptions;
    /// use ferrochart_core::testing::{random_walk, MonoMeasurer};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.resize(SizeCss { w: 800.0, h: 600.0 }, 1.0);
    /// let candles = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
    /// // Volume in its own pane, a quarter of the height of the main pane.
    /// let below = chart.add_pane(PaneOptions { stretch_factor: 0.25 });
    /// chart.add_series(below, SeriesKind::Volume, SeriesOptions::volume_of(candles))?;
    /// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 500, 0, 60_000)?))?;
    /// chart.build_frame(0.0, &MonoMeasurer);
    ///
    /// let main = chart.pane_layout(chart.main_pane()).unwrap();
    /// let vol = chart.pane_layout(below).unwrap();
    /// assert!(vol.plot.y > main.plot.bottom());
    /// assert!((main.plot.h / vol.plot.h - 4.0).abs() < 0.1);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_pane(&mut self, opts: PaneOptions) -> PaneId {
        let id = PaneId(self.next_pane);
        self.next_pane = self.next_pane.wrapping_add(1);
        self.panes.push(Pane::new(id, opts.stretch_factor));
        self.drawings.pane_added(id);
        self.invalidation = Invalidation::Full;
        id
    }

    /// Removes a pane and the series in it (removing the candlestick series
    /// also removes its volume series, wherever it is).
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the pane does not exist,
    /// [`Error::InvalidArgument`] for the last remaining pane.
    pub fn remove_pane(&mut self, id: PaneId) -> Result<()> {
        let i = self.pane_index(id).ok_or(Error::UnknownId)?;
        if self.panes.len() == 1 {
            return Err(Error::InvalidArgument("a chart keeps at least one pane"));
        }
        while let Some(sid) = self.series.iter().find(|e| e.pane == id).map(|e| e.id) {
            self.remove_series(sid)?;
        }
        self.panes.remove(i);
        self.drawings.pane_removed(id);
        self.layer_plugins.retain(|l| l.pane != Some(id));
        if self.input.drag.involves(id) {
            self.input.drag = Drag::Idle;
        }
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// The panes from top to bottom.
    pub fn panes(&self) -> impl Iterator<Item = PaneId> + '_ {
        self.panes.iter().map(|p| p.id)
    }

    /// Rectangles of a pane in the last built layout; `None` for unknown ids.
    pub fn pane_layout(&self, id: PaneId) -> Option<PaneLayout> {
        let pane = self.panes.get(self.pane_index(id)?)?;
        Some(PaneLayout {
            plot: pane.plot,
            price_axis: RectPx {
                x: self.layout.price_axis.x,
                y: pane.plot.y,
                w: self.layout.price_axis.w,
                h: pane.plot.h,
            },
        })
    }

    /// The stretch factor of a pane; `None` for unknown ids.
    pub fn pane_stretch(&self, id: PaneId) -> Option<f32> {
        Some(self.panes.get(self.pane_index(id)?)?.stretch)
    }

    /// Sets the stretch factor of a pane (dragging a separator changes the
    /// factors of the two panes next to it).
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the pane does not exist,
    /// [`Error::InvalidArgument`] if `stretch` is not finite and positive.
    pub fn set_pane_stretch(&mut self, id: PaneId, stretch: f32) -> Result<()> {
        let i = self.pane_index(id).ok_or(Error::UnknownId)?;
        if !(stretch.is_finite() && stretch > 0.0) {
            return Err(Error::InvalidArgument(
                "a stretch factor must be finite and positive",
            ));
        }
        if let Some(p) = self.panes.get_mut(i) {
            p.stretch = stretch;
        }
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Prices at the bottom and the top of pane `id`'s plot area in the last
    /// built layout (display values in the relative modes); `None` for an
    /// unknown pane or before the first frame. Hosts copy it to a
    /// `ferrochart-plot` side panel that shares the price axis.
    ///
    /// ```
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::SizeCss;
    /// use ferrochart_core::testing::{random_walk, MonoMeasurer};
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let pane = chart.main_pane();
    /// assert_eq!(chart.visible_price_range(pane), None);
    /// let id = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_data(id, SeriesData::Ohlc(random_walk(1, 100, 0, 60_000)?))?;
    /// chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
    /// chart.build_frame(0.0, &MonoMeasurer);
    /// let (lo, hi) = chart.visible_price_range(pane).unwrap();
    /// assert!(lo < hi);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn visible_price_range(&self, id: PaneId) -> Option<(f64, f64)> {
        let pane = self.panes.get(self.pane_index(id)?)?;
        let h = f64::from(pane.plot.h);
        if !(pane.has_content && h > 0.0) {
            return None;
        }
        let a = pane.price_scale.y_to_price(h, 0.0, h);
        let b = pane.price_scale.y_to_price(0.0, 0.0, h);
        (a.is_finite() && b.is_finite()).then_some((a.min(b), a.max(b)))
    }

    pub(crate) fn pane_index(&self, id: PaneId) -> Option<usize> {
        self.panes.iter().position(|p| p.id == id)
    }

    pub(crate) fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    fn entry(&self, id: SeriesId) -> Result<&SeriesEntry> {
        self.series
            .iter()
            .find(|e| e.id == id)
            .ok_or(Error::UnknownId)
    }

    fn entry_index(&self, id: SeriesId) -> Result<usize> {
        self.series
            .iter()
            .position(|e| e.id == id)
            .ok_or(Error::UnknownId)
    }

    /// Adds a candlestick series on the main pane with the default options:
    /// `add_series(main_pane(), SeriesKind::Candlestick, SeriesOptions::default())`.
    ///
    /// # Errors
    ///
    /// As [`Chart::add_series`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::testing::random_walk;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let candles = chart.add_candlestick()?;
    /// chart.add_volume(candles)?;
    /// chart.set_data(candles, random_walk(1, 500, 0, 60_000)?)?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_candlestick(&mut self) -> Result<SeriesId> {
        self.add_series(
            self.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
    }

    /// Adds a volume histogram of `source`'s volume column on the main pane:
    /// `add_series(main_pane(), SeriesKind::Volume, SeriesOptions::volume_of(source))`.
    ///
    /// # Errors
    ///
    /// As [`Chart::add_series`] ([`Error::UnknownId`] if `source` does not
    /// exist).
    pub fn add_volume(&mut self, source: SeriesId) -> Result<SeriesId> {
        self.add_series(
            self.main_pane(),
            SeriesKind::Volume,
            SeriesOptions::volume_of(source),
        )
    }

    /// Adds a series to a pane.
    ///
    /// All series share one time axis, the union of their times. A
    /// series that skips bars of the axis is drawn across them (line-like
    /// series) or leaves gaps (histograms, OHLC series).
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] if `pane` does not exist;
    /// - [`Error::InvalidArgument`] for a volume series whose `opts.source` is
    ///   not an OHLC or transform series of this chart, for `Custom`, `Kagi`
    ///   and `PointFigure` (added with their own methods), and for any
    ///   series but volume when the chart has a transform series.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::{Chart, ChartOptions};
    /// use ferrochart_core::chart::SeriesOptions;
    /// use ferrochart_core::data::ValueColumns;
    /// use ferrochart_core::chart::SeriesData;
    /// use ferrochart_core::frame::SeriesKind;
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let line = chart.add_series(chart.main_pane(), SeriesKind::Line, SeriesOptions::default())?;
    /// chart.set_data(line, SeriesData::Value(ValueColumns {
    ///     time: vec![0, 60_000, 120_000],
    ///     value: vec![1.0, f64::NAN, 3.0], // NaN: a gap
    ///     color: None,
    /// }))?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_series(
        &mut self,
        pane: PaneId,
        kind: SeriesKind,
        opts: SeriesOptions,
    ) -> Result<SeriesId> {
        if self.transform_entry().is_some() && kind != SeriesKind::Volume {
            return Err(NEXT_TO_TRANSFORM);
        }
        self.insert_series(pane, kind, opts)
    }

    /// `add_series` without the transform-series check (indicator outputs).
    fn insert_series(
        &mut self,
        pane: PaneId,
        kind: SeriesKind,
        opts: SeriesOptions,
    ) -> Result<SeriesId> {
        if self.pane_index(pane).is_none() {
            return Err(Error::UnknownId);
        }
        if kind == SeriesKind::Custom {
            return Err(Error::InvalidArgument(
                "custom series are added with add_custom_series",
            ));
        }
        if matches!(kind, SeriesKind::Kagi | SeriesKind::PointFigure) {
            return Err(Error::InvalidArgument(
                "Kagi and Point & Figure series are added with add_transform_series",
            ));
        }
        if kind == SeriesKind::Band {
            return Err(Error::InvalidArgument("bands are added with add_band"));
        }
        check_series_options(&opts)?;
        if kind == SeriesKind::Volume {
            let source_ok = opts
                .source
                .and_then(|s| self.entry(s).ok())
                .is_some_and(|e| is_ohlc_kind(e.kind) || e.store.transform().is_some());
            if !source_ok {
                return Err(Error::InvalidArgument(
                    "a volume series needs an OHLC series of this chart as its source",
                ));
            }
            if !self.series.iter().any(|e| e.pane == pane)
                && let Some(p) = self.pane_mut(pane)
            {
                // Bars stand on the bottom of a pane of their own.
                let _ = p.price_scale.set_margins(PRICE_MARGINS.0, 0.0);
            }
        }
        let id = SeriesId(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.series.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.series.push(SeriesEntry {
            id,
            pane,
            kind,
            opts,
            store: Store::for_kind(kind),
            norm: None,
            markers: Vec::new(),
            bar_flags: Vec::new(),
            derived: false,
            volume_scale: false,
            auto_decimals: 2,
        });
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// The options of a series; `None` for an unknown id.
    pub fn series_options(&self, id: SeriesId) -> Option<&SeriesOptions> {
        self.entry(id).ok().map(|e| &e.opts)
    }

    /// The kind of a series; `None` for an unknown id.
    pub fn series_kind(&self, id: SeriesId) -> Option<SeriesKind> {
        self.entry(id).ok().map(|e| e.kind)
    }

    /// The pane of a series; `None` for an unknown id.
    pub fn series_pane(&self, id: SeriesId) -> Option<PaneId> {
        self.entry(id).ok().map(|e| e.pane)
    }

    /// Changes the options of a series (colors, line style, labels, shift,
    /// …), e.g. for `applyOptions` of the lightweight-charts compatibility
    /// layer. The data is kept.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] if the series does not exist;
    /// - [`Error::InvalidArgument`] if `opts.source` differs from the
    ///   series' source (the source of a volume series is fixed).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::SeriesOptions;
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::Rgba;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let line = chart.add_series(chart.main_pane(), SeriesKind::Line, SeriesOptions::default())?;
    /// let mut opts = *chart.series_options(line).unwrap();
    /// opts.color = Some(Rgba::hex(0x00FF_6D00));
    /// chart.set_series_options(line, opts)?;
    /// assert_eq!(chart.series_options(line).unwrap().color, Some(Rgba::hex(0x00FF_6D00)));
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn set_series_options(&mut self, id: SeriesId, opts: SeriesOptions) -> Result<()> {
        let k = self.entry_index(id)?;
        if opts.source != self.series[k].opts.source {
            return Err(Error::InvalidArgument(
                "the source of a series cannot change",
            ));
        }
        check_series_options(&opts)?;
        self.series[k].opts = opts;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Removes a series and its data. Removing an OHLC series also removes the
    /// volume series that read it.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the series does not exist.
    pub fn remove_series(&mut self, id: SeriesId) -> Result<()> {
        if self.entry(id)?.derived {
            return Err(Error::InvalidArgument(
                "an indicator output; remove the indicator instead",
            ));
        }
        self.drop_indicators_of(id);
        self.series
            .retain(|e| e.id != id && e.opts.source != Some(id));
        self.price_lines
            .retain(|l| self.series.iter().any(|e| e.id == l.series));
        self.prune_bands();
        // The other series keep the time axis; without data it is reset.
        if self.series.iter().all(|e| e.store.is_empty()) {
            self.time_index = TimeIndex::new();
            self.time_scale.set_extent(0, 0);
        }
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    fn after_change(&mut self, id: SeriesId, repaired: usize) {
        self.time_scale
            .set_extent(self.time_index.first_index(), self.time_index.end_index());
        self.invalidation = self.invalidation.max(Invalidation::Data);
        if repaired > 0 {
            self.events
                .push(ChartEvent::Warning(WarningKind::OhlcRepaired {
                    series: id,
                    bars: repaired,
                }));
        }
    }

    /// Index of series `id`, which must take data of its own.
    fn data_entry(&self, id: SeriesId) -> Result<usize> {
        let k = self.entry_index(id)?;
        if self.series[k].store.takes_data() {
            Ok(k)
        } else if self.series[k].kind == SeriesKind::Custom {
            Err(Error::InvalidArgument(
                "a custom series owns its data; use update_custom",
            ))
        } else {
            Err(Error::InvalidArgument(
                "this series has no data of its own; use its source series",
            ))
        }
    }

    /// Runs a data operation on series `k` with the current time axis. If it
    /// fails, the `(front, back)` slots `cover` added are removed again.
    fn run_op<T>(
        &mut self,
        k: usize,
        added: (usize, usize),
        f: impl FnOnce(&Op<'_>, &mut Store) -> Result<T>,
    ) -> Result<T> {
        let entry = &mut self.series[k];
        let op = Op {
            kind: entry.kind,
            index: &self.time_index,
            mode: self.validation,
        };
        let out = f(&op, &mut entry.store);
        if out.is_err() {
            self.time_index.truncate_ends(added.0, added.1);
        }
        out
    }

    /// Makes `times` slots of the time axis where that keeps every logical
    /// index: times before the first slot are prepended, times after
    /// the last appended. Returns the `(front, back)` numbers added, or `None`
    /// if a time falls between existing slots (the axis must be rebuilt).
    fn cover(&mut self, times: &[Timestamp]) -> Result<Option<(usize, usize)>> {
        let index = &mut self.time_index;
        let (Some(&t0), Some(&tn)) = (times.first(), times.last()) else {
            return Ok(Some((0, 0)));
        };
        let (Some(&first), Some(&last)) = (index.times().first(), index.times().last()) else {
            index.append(times)?;
            return Ok(Some((0, times.len())));
        };
        if t0 > last {
            index.append(times)?;
            return Ok(Some((0, times.len())));
        }
        if tn < first {
            index.prepend(times)?;
            return Ok(Some((times.len(), 0)));
        }
        if let [t] = times {
            // The real-time path: no allocation.
            return Ok(index.index_of(*t).map(|_| (0, 0)));
        }
        let head = times.partition_point(|&t| t < first);
        let body_end = times.partition_point(|&t| t <= last).max(head);
        if index.map_series(&times[head..body_end])?.is_none() {
            return Ok(None);
        }
        index.prepend(&times[..head])?;
        if let Err(e) = index.append(&times[body_end..]) {
            index.truncate_ends(head, 0);
            return Err(e);
        }
        Ok(Some((head, times.len() - body_end)))
    }

    /// Logical-index spans of the series other than `k` that have data,
    /// sorted and merged.
    fn other_spans(&self, k: usize) -> Vec<(i64, i64)> {
        let mut spans: Vec<(i64, i64)> = self
            .series
            .iter()
            .enumerate()
            .filter(|&(j, e)| j != k && !e.derived)
            .filter_map(|(_, e)| e.store.span())
            .collect();
        spans.sort_unstable();
        let mut merged: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
        for (a, b) in spans {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        merged
    }

    /// Whether a slot of the axis outside `others` (sorted, disjoint spans)
    /// is not one of `times` (which are all slots of the axis): such a slot
    /// would belong to no series after the operation.
    fn has_orphans(&self, others: &[(i64, i64)], times: &[Timestamp]) -> bool {
        let index = &self.time_index;
        let end = index.end_index();
        let mut at = index.first_index();
        for &(a, b) in others.iter().chain(core::iter::once(&(end, end))) {
            if at < a {
                let (Some(ta), Some(tb)) = (index.time_at(at), index.time_at(a - 1)) else {
                    return true;
                };
                let lo = times.partition_point(|&t| t < ta);
                let hi = times.partition_point(|&t| t <= tb);
                if i64::try_from(hi - lo).ok() != Some(a - at) {
                    return true;
                }
            }
            at = at.max(b);
        }
        false
    }

    /// Rebuilds the time axis as the union of `target`'s times (the complete
    /// new data of series `k`) and the slots the `others` spans cover, then
    /// sets every series' data on it. Logical indices are kept for the
    /// first slot the old and new axis share. Reports a
    /// [`WarningKind::TimeAxisMerged`] if times were inserted between
    /// existing slots. The chart is unchanged if `target` is invalid.
    fn rebuild(&mut self, k: usize, target: Target<'_>, others: &[(i64, i64)]) -> Result<usize> {
        let old = &self.time_index;
        let mut kept = Vec::new();
        for &(a, b) in others {
            let from = usize::try_from(a - old.first_index()).unwrap_or(0);
            let to = usize::try_from(b - old.first_index()).unwrap_or(0);
            kept.extend_from_slice(old.times().get(from..to).unwrap_or(&[]));
        }
        let mut index = TimeIndex::from_times(kept)?;
        index.merge(target.times())?;
        let inserted = match (old.times().first(), old.times().last()) {
            (Some(&first), Some(&last)) => target
                .times()
                .iter()
                .filter(|&&t| t > first && t < last && old.index_of(t).is_none())
                .count(),
            _ => 0,
        };
        let shared = index
            .times()
            .iter()
            .enumerate()
            .find_map(|(p, &t)| Some((old.index_of(t)?, p)));
        if let Some((j, p)) = shared
            && let Some(first) = j.checked_sub(i64::try_from(p).unwrap_or(i64::MAX))
        {
            index.rebase(first);
        }
        let snapshots: Vec<(usize, SeriesData)> = self
            .series
            .iter()
            .enumerate()
            .filter(|&(j, e)| j != k && !e.store.is_empty() && !e.derived)
            .filter_map(|(j, e)| Some((j, e.store.snapshot(old)?)))
            .collect();
        // The target first: its errors leave the chart unchanged.
        let entry = &mut self.series[k];
        let repaired = match target {
            Target::Data(data) => Op {
                kind: entry.kind,
                index: &index,
                mode: self.validation,
            }
            .set(&mut entry.store, data)?,
            Target::Custom(c) => {
                c.map_onto(&index)?;
                c.log.replace();
                0
            }
        };
        let id = entry.id;
        for (j, snap) in snapshots {
            let entry = &mut self.series[j];
            let op = Op {
                kind: entry.kind,
                index: &index,
                mode: ValidationMode::Repair,
            };
            // Only an allocation failure can make a snapshot not fit.
            if op.set(&mut entry.store, snap).is_err() {
                entry.store = Store::for_kind(entry.kind);
                self.events
                    .push(ChartEvent::Warning(WarningKind::DataDropped {
                        series: entry.id,
                    }));
            }
        }
        // Custom series are re-mapped from their own times.
        for e in &mut self.series {
            if let Store::Custom(c) = &mut e.store
                && c.mode == AxisMode::Slots
                && c.len() > 0
            {
                c.log.replace();
                if c.map_onto(&index).is_err() {
                    c.clear();
                    self.events
                        .push(ChartEvent::Warning(WarningKind::DataDropped {
                            series: e.id,
                        }));
                }
            }
        }
        // Indicator outputs are recomputed on the new axis (`after_data`).
        for e in self.series.iter_mut().filter(|e| e.derived) {
            e.store = Store::for_kind(e.kind);
        }
        self.axis_rebuilt = true;
        self.time_index = index;
        self.time_index.shrink_to_fit();
        if inserted > 0 {
            self.events
                .push(ChartEvent::Warning(WarningKind::TimeAxisMerged {
                    series: id,
                    slots: inserted,
                }));
        }
        Ok(repaired)
    }

    /// The complete data of series `k` after adding `new` before (`prepend`)
    /// or after its current points, for a rebuild.
    fn combined(&self, k: usize, new: SeriesData, prepend: bool) -> Result<SeriesData> {
        let store = &self.series[k].store;
        let Some(old) = store
            .snapshot(&self.time_index)
            .filter(|_| !store.is_empty())
        else {
            return Ok(new);
        };
        let (Some(&new_first), Some(&new_last)) = (new.times().first(), new.times().last()) else {
            return Ok(old);
        };
        let (Some(&old_first), Some(&old_last)) = (old.times().first(), old.times().last()) else {
            return Ok(new);
        };
        if prepend {
            if new_last >= old_first {
                return Err(Error::TimeNotIncreasing {
                    index: new.times().len() - 1,
                });
            }
            new.concat(old)
        } else {
            if new_first <= old_last {
                return Err(Error::TimeNotIncreasing { index: 0 });
            }
            old.concat(new)
        }
    }

    /// Replaces all data of a series.
    ///
    /// The time axis becomes the union of the new times and the times of the
    /// other series: times the other series do not have are added (a
    /// time between existing bars rebuilds the axis and reports
    /// [`WarningKind::TimeAxisMerged`]), and bars only this series had are
    /// removed.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`], [`Error::InvalidArgument`] (wrong series or data
    /// kind), and the data validation errors
    /// ([`Error::LengthMismatch`], [`Error::InvalidOhlc`],
    /// [`Error::TimeNotIncreasing`]). The chart is unchanged on error.
    pub fn set_data(&mut self, id: SeriesId, data: impl Into<SeriesData>) -> Result<()> {
        self.set_data_data(id, data.into())
    }

    fn set_data_data(&mut self, id: SeriesId, data: SeriesData) -> Result<()> {
        let k = self.data_entry(id)?;
        if self.series[k].store.transform().is_some() {
            let (input, repaired) = input_columns(data, self.validation)?;
            self.run_transform(k, repaired, |t, index| {
                t.set(index, input).map(|()| Some(DataChange::Replace))
            })?;
            return Ok(());
        }
        let others = self.other_spans(k);
        let repaired = if others.is_empty() {
            // The only series with data: its times are the axis.
            let (times, data) = data.take_times()?;
            let index = TimeIndex::from_times(times)?;
            let entry = &mut self.series[k];
            let repaired = Op {
                kind: entry.kind,
                index: &index,
                mode: self.validation,
            }
            .set_aligned(&mut entry.store, index.first_index(), data)?;
            self.time_index = index;
            self.time_index.shrink_to_fit();
            // A new axis: indicator outputs are recomputed on it.
            self.axis_rebuilt = true;
            repaired
        } else if self.time_index.map_series(data.times())?.is_some()
            && !self.has_orphans(&others, data.times())
        {
            self.run_op(k, (0, 0), |op, store| op.set(store, data))?
        } else {
            self.rebuild(k, Target::Data(data), &others)?
        };
        self.after_change(id, repaired);
        self.after_data(id, DataChange::Replace);
        Ok(())
    }

    /// Appends points after the last one. Times the axis does not have yet
    /// are added to it.
    ///
    /// # Errors
    ///
    /// As [`set_data`](Self::set_data); additionally the times must come after
    /// the last point, and volume must be present exactly when the series has it.
    pub fn append(&mut self, id: SeriesId, data: impl Into<SeriesData>) -> Result<()> {
        self.append_data(id, data.into())
    }

    fn append_data(&mut self, id: SeriesId, data: SeriesData) -> Result<()> {
        let k = self.data_entry(id)?;
        if self.series[k].store.transform().is_some() {
            let (input, repaired) = input_columns(data, self.validation)?;
            self.run_transform(k, repaired, |t, index| t.append(index, input))?;
            return Ok(());
        }
        let old_end = self.series[k].store.span().map(|(_, e)| e);
        let repaired = if let Some(added) = self.cover(data.times())? {
            self.run_op(k, added, |op, store| op.append(store, data))?
        } else {
            let all = self.combined(k, data, false)?;
            let others = self.other_spans(k);
            self.rebuild(k, Target::Data(all), &others)?
        };
        self.after_change(id, repaired);
        let change = match (old_end, self.series[k].store.span()) {
            (Some(old), Some((_, end))) => DataChange::Append {
                count: usize::try_from(end - old).unwrap_or(0),
            },
            _ => DataChange::Replace,
        };
        self.after_data(id, change);
        Ok(())
    }

    /// Inserts older points before the first one (history loading). Existing
    /// points keep their logical indices unless a time falls between
    /// existing bars of the axis.
    ///
    /// # Errors
    ///
    /// As [`append`](Self::append), with times before the first point.
    pub fn prepend(&mut self, id: SeriesId, data: impl Into<SeriesData>) -> Result<()> {
        self.prepend_data(id, data.into())
    }

    fn prepend_data(&mut self, id: SeriesId, data: SeriesData) -> Result<()> {
        let k = self.data_entry(id)?;
        if self.series[k].store.transform().is_some() {
            let (input, repaired) = input_columns(data, self.validation)?;
            self.run_transform(k, repaired, |t, index| {
                t.prepend(index, input).map(|()| Some(DataChange::Replace))
            })?;
            return Ok(());
        }
        let repaired = if let Some(added) = self.cover(data.times())? {
            self.run_op(k, added, |op, store| op.prepend(store, data))?
        } else {
            let all = self.combined(k, data, true)?;
            let others = self.other_spans(k);
            self.rebuild(k, Target::Data(all), &others)?
        };
        self.after_change(id, repaired);
        self.after_data(id, DataChange::Replace);
        Ok(())
    }

    /// Real-time update. Replaces the last point if `bar.time` equals its time
    /// (no allocation), appends one if it is later. Returns the applied change.
    /// Value series (line, area, baseline, histogram) take `bar.close` as the
    /// value and ignore the other fields. For a transform series `bar` is an
    /// input and the result is the change of the output (`Append { count: 0
    /// }` if the output did not change); replacing the last input does not
    /// allocate while the number of forming elements stays the same.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`], [`Error::InvalidArgument`] (a volume series),
    /// [`Error::TimeNotIncreasing`] (time before the last point), and the
    /// validation errors.
    pub fn update_last(&mut self, id: SeriesId, bar: BarUpdate) -> Result<DataChange> {
        let k = self.data_entry(id)?;
        if self.series[k].store.transform().is_some() {
            let (bar, repaired) = crate::data::ohlc::OhlcStore::check_one(bar, self.validation)?;
            return self.run_transform(k, repaired, |t, index| t.update_last(index, bar));
        }
        let (change, repaired) = if let Some(added) = self.cover(&[bar.time])? {
            self.run_op(k, added, |op, store| op.update_last(store, bar))?
        } else {
            let point = SeriesData::point(self.series[k].kind, bar);
            let all = self.combined(k, point, false)?;
            let others = self.other_spans(k);
            (
                DataChange::Append { count: 1 },
                self.rebuild(k, Target::Data(all), &others)?,
            )
        };
        if change == DataChange::UpdateLast {
            self.invalidation = self.invalidation.max(Invalidation::Data);
            if repaired > 0 {
                self.after_change(id, repaired);
            }
        } else {
            self.after_change(id, repaired);
        }
        self.after_data(id, change);
        Ok(change)
    }

    /// Adds a transform series to a pane: the data given to it
    /// with [`set_data`](Self::set_data), [`append`](Self::append),
    /// [`prepend`](Self::prepend) and [`update_last`](Self::update_last) is
    /// its input, and its output elements, at synthetic times, are the time
    /// axis. See [`crate::transform`] for the built-in transforms.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] if `pane` does not exist;
    /// - [`Error::InvalidArgument`] if the chart already has a series other
    ///   than volume (a chart has at most one transform series and no
    ///   time-based series next to it), if the output kind is not
    ///   `Candlestick`, `OhlcBar`, `Kagi` or `PointFigure`, or if a
    ///   `PointFigure` transform has no positive box size.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::data::BarUpdate;
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::testing::{random_walk, MonoMeasurer};
    /// use ferrochart_core::transform::{Kagi, Reversal};
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let pane = chart.main_pane();
    /// let kagi = chart.add_transform_series(
    ///     pane,
    ///     Box::new(Kagi::new(Reversal::Percent(2.0))?),
    ///     SeriesOptions::default(),
    /// )?;
    /// let bars = random_walk(1, 5_000, 0, 60_000)?;
    /// let last = *bars.time.last().unwrap();
    /// chart.set_data(kagi, SeriesData::Ohlc(bars))?;
    /// // Far fewer lines than input bars.
    /// assert!(chart.data_view().time_index().len() < 1_000);
    /// // Real-time input.
    /// let tick = BarUpdate { time: last + 60_000, open: 100.0, high: 100.0, low: 100.0, close: 100.0, volume: Some(1.0) };
    /// chart.update_last(kagi, tick)?;
    /// // No time-based series next to it.
    /// assert!(chart.add_series(pane, SeriesKind::Line, SeriesOptions::default()).is_err());
    /// chart.build_frame(0.0, &MonoMeasurer);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_transform_series(
        &mut self,
        pane: PaneId,
        t: Box<dyn SeriesTransform>,
        opts: SeriesOptions,
    ) -> Result<SeriesId> {
        if self.pane_index(pane).is_none() {
            return Err(Error::UnknownId);
        }
        if self
            .series
            .iter()
            .any(|e| !e.derived && e.kind != SeriesKind::Volume)
        {
            return Err(NEXT_TO_TRANSFORM);
        }
        let kind = t.output_kind();
        if !matches!(
            kind,
            SeriesKind::Candlestick
                | SeriesKind::OhlcBar
                | SeriesKind::Kagi
                | SeriesKind::PointFigure
        ) {
            return Err(Error::InvalidArgument(
                "a transform is drawn as candlesticks, OHLC bars, Kagi or Point & Figure",
            ));
        }
        if kind == SeriesKind::PointFigure
            && !t.box_size().is_some_and(|b| b.is_finite() && b > 0.0)
        {
            return Err(Error::InvalidArgument(
                "a Point & Figure transform needs a positive box size",
            ));
        }
        let id = SeriesId(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.series.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.series.push(SeriesEntry {
            id,
            pane,
            kind,
            opts,
            store: Store::Transform(Box::new(TransformStore::new(t))),
            norm: None,
            markers: Vec::new(),
            bar_flags: Vec::new(),
            derived: false,
            volume_scale: false,
            auto_decimals: 2,
        });
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// The transform series, if the chart has one.
    fn transform_entry(&self) -> Option<usize> {
        self.series
            .iter()
            .position(|e| e.store.transform().is_some())
    }

    /// Runs a data operation on transform series `k`, whose output is the
    /// whole time axis, and passes the output's change on.
    fn run_transform(
        &mut self,
        k: usize,
        repaired: usize,
        f: impl FnOnce(&mut TransformStore, &mut TimeIndex) -> Result<Option<DataChange>>,
    ) -> Result<DataChange> {
        let Store::Transform(t) = &mut self.series[k].store else {
            return Err(Error::InvalidArgument("not a transform series"));
        };
        let out = f(t, &mut self.time_index);
        let id = self.series[k].id;
        let change = match out {
            Ok(c) => c,
            Err(e) => {
                // The output may have been cleared.
                self.after_change(id, 0);
                self.after_data(id, DataChange::Replace);
                return Err(e);
            }
        };
        match change {
            None => {
                if repaired > 0 {
                    self.after_change(id, repaired);
                }
                Ok(DataChange::Append { count: 0 })
            }
            Some(DataChange::UpdateLast) => {
                self.invalidation = self.invalidation.max(Invalidation::Data);
                if repaired > 0 {
                    self.after_change(id, repaired);
                }
                self.after_data(id, DataChange::UpdateLast);
                Ok(DataChange::UpdateLast)
            }
            Some(change) => {
                self.after_change(id, repaired);
                self.after_data(id, change);
                Ok(change)
            }
        }
    }

    /// Adds a custom series to a pane. Its current data is taken as a
    /// `Replace`; later changes go through [`update_custom`](Self::update_custom).
    /// See [`CustomSeries`] for an example.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if `pane` does not exist;
    /// [`Error::InvalidArgument`] if the chart has a transform series;
    /// [`Error::TimeNotIncreasing`] if the plugin's times are not strictly
    /// increasing (the series is not added); [`Error::OutOfMemory`].
    pub fn add_custom_series(
        &mut self,
        pane: PaneId,
        series: Box<dyn CustomSeries>,
        opts: SeriesOptions,
    ) -> Result<SeriesId> {
        if self.pane_index(pane).is_none() {
            return Err(Error::UnknownId);
        }
        if self.transform_entry().is_some() {
            return Err(NEXT_TO_TRANSFORM);
        }
        let id = SeriesId(self.next_id);
        let next = self.next_id.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.series.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        let has_data = !series.times().is_empty();
        self.series.push(SeriesEntry {
            id,
            pane,
            kind: SeriesKind::Custom,
            opts,
            store: Store::Custom(Box::new(CustomStore::new(series))),
            norm: None,
            markers: Vec::new(),
            bar_flags: Vec::new(),
            derived: false,
            volume_scale: false,
            auto_decimals: 2,
        });
        self.next_id = next;
        self.invalidation = Invalidation::Full;
        if has_data && let Err(e) = self.change_custom(id, |_| Ok(DataChange::Replace)) {
            self.series.pop();
            return Err(e);
        }
        Ok(id)
    }

    /// The plugin of custom series `id` as `T`; `None` for an unknown id,
    /// another kind of series or another type.
    pub fn custom_series<T: CustomSeries>(&self, id: SeriesId) -> Option<&T> {
        let any: &dyn Any = self.entry(id).ok()?.store.custom()?.plugin.as_ref();
        any.downcast_ref::<T>()
    }

    /// Changes the data of custom series `id`: `f` modifies the plugin and
    /// returns what changed, and core updates the time axis, generation and
    /// change log. `DataChange::UpdateLast` keeping the last time does
    /// not allocate. A change that does not match the data (wrong count, the
    /// last time moved) is applied as a `Replace`.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`];
    /// - [`Error::InvalidArgument`] if it is not a custom series or its plugin
    ///   is not a `T` (`f` is not called);
    /// - [`Error::TimeNotIncreasing`] if the times are no longer strictly
    ///   increasing: the series then shows nothing until a valid change;
    /// - [`Error::OutOfMemory`].
    pub fn update_custom<T: CustomSeries>(
        &mut self,
        id: SeriesId,
        f: impl FnOnce(&mut T) -> DataChange,
    ) -> Result<()> {
        self.change_custom(id, |c| {
            let any: &mut dyn Any = c.plugin.as_mut();
            let t = any.downcast_mut::<T>().ok_or(Error::InvalidArgument(
                "the custom series is of another type",
            ))?;
            Ok(f(t))
        })
    }

    /// Takes custom series `id` out of its entry, lets `modify` change the
    /// plugin and say what changed, and applies the change.
    fn change_custom(
        &mut self,
        id: SeriesId,
        modify: impl FnOnce(&mut CustomStore) -> Result<DataChange>,
    ) -> Result<()> {
        let k = self.entry_index(id)?;
        let store = core::mem::replace(&mut self.series[k].store, Store::None);
        let Store::Custom(mut c) = store else {
            self.series[k].store = store;
            return Err(NOT_CUSTOM);
        };
        let out = modify(&mut c).and_then(|change| {
            let applied = c
                .classify(change)
                .and_then(|kind| self.apply_custom(k, &mut c, kind));
            if applied.is_err() {
                c.clear();
                self.invalidation = Invalidation::Full;
            }
            applied
        });
        self.series[k].store = Store::Custom(c);
        match out? {
            None => {}
            Some(DataChange::UpdateLast) => {
                self.invalidation = self.invalidation.max(Invalidation::Data);
                self.after_data(id, DataChange::UpdateLast);
            }
            Some(change) => {
                self.after_change(id, 0);
                self.after_data(id, change);
            }
        }
        Ok(())
    }

    /// Brings the time axis and the mapping of custom series `k` (taken out
    /// of its entry as `c`) up to the plugin's data.
    fn apply_custom(
        &mut self,
        k: usize,
        c: &mut CustomStore,
        kind: CustomChange,
    ) -> Result<Option<DataChange>> {
        let old = c.len();
        let n = c.plugin.times().len();
        let span = |a: usize, b: usize| IdxSpan {
            start: i64::try_from(a).unwrap_or(i64::MAX),
            end: i64::try_from(b).unwrap_or(i64::MAX),
        };
        let change = match kind {
            CustomChange::None => return Ok(None),
            CustomChange::UpdateLast => {
                // The real-time path: no allocation.
                c.log.record(span(old.saturating_sub(1), old));
                return Ok(Some(DataChange::UpdateLast));
            }
            CustomChange::Append => DataChange::Append { count: n - old },
            CustomChange::Prepend { count } => DataChange::Prepend { count },
            CustomChange::Replace => DataChange::Replace,
        };
        if c.mode == AxisMode::Overlay {
            c.map_onto(&self.time_index)?;
            if kind == CustomChange::Append {
                c.log.record(span(old, n));
            } else {
                c.log.replace();
            }
            return Ok(Some(change));
        }
        match kind {
            CustomChange::Append | CustomChange::Prepend { .. } => {
                let times = c.plugin.times();
                let new = match kind {
                    CustomChange::Prepend { count } => times.get(..count),
                    _ => times.get(old..),
                };
                if let Some(added) = self.cover(new.unwrap_or(&[]))? {
                    let mapped = if kind == CustomChange::Append {
                        c.extend_onto(&self.time_index)
                    } else {
                        c.map_onto(&self.time_index)
                    };
                    if let Err(e) = mapped {
                        self.time_index.truncate_ends(added.0, added.1);
                        return Err(e);
                    }
                    if kind == CustomChange::Append {
                        c.log.record(span(old, n));
                    } else {
                        // Every point number moved.
                        c.log.replace();
                    }
                } else {
                    let others = self.other_spans(k);
                    self.rebuild(k, Target::Custom(c), &others)?;
                }
            }
            _ => {
                let others = self.other_spans(k);
                let times = c.plugin.times();
                if others.is_empty() {
                    // The only series with data: its times are the axis.
                    let mut own = Vec::new();
                    own.try_reserve_exact(times.len())
                        .map_err(|_| Error::OutOfMemory)?;
                    own.extend_from_slice(times);
                    let index = TimeIndex::from_times(own)?;
                    c.map_onto(&index)?;
                    self.time_index = index;
                    self.axis_rebuilt = true;
                } else if let Some(m) = self.time_index.map_series(times)?
                    && !self.has_orphans(&others, times)
                {
                    c.accept(m);
                } else {
                    self.rebuild(k, Target::Custom(c), &others)?;
                }
                c.log.replace();
            }
        }
        Ok(Some(change))
    }

    /// Adds a price line to a series. Its price is in the series'
    /// units, so on a relative scale it moves with the series.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the series does not exist;
    /// [`Error::InvalidArgument`] for a volume series or a price that is not
    /// finite.
    ///
    /// # Examples
    ///
    /// See [`PriceLineOptions`].
    pub fn create_price_line(
        &mut self,
        series: SeriesId,
        opts: PriceLineOptions,
    ) -> Result<PriceLineId> {
        let e = self.entry(series)?;
        if e.kind == SeriesKind::Volume {
            return Err(Error::InvalidArgument("volume series have no price lines"));
        }
        if !opts.price.is_finite() {
            return Err(Error::InvalidArgument("the price must be finite"));
        }
        let id = PriceLineId(self.next_price_line);
        self.next_price_line += 1;
        self.price_lines.push(PriceLineEntry { id, series, opts });
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(id)
    }

    /// Replaces the options of a price line.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the price line does not exist;
    /// [`Error::InvalidArgument`] for a price that is not finite.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::{PriceLineOptions, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let id = chart.add_series(chart.main_pane(), SeriesKind::Line, SeriesOptions::default())?;
    /// let line = chart.create_price_line(id, PriceLineOptions { price: 1.0, ..Default::default() })?;
    /// chart.update_price_line(line, PriceLineOptions { price: 2.0, ..Default::default() })?;
    /// assert_eq!(chart.price_line(line).map(|o| o.price), Some(2.0));
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn update_price_line(&mut self, id: PriceLineId, opts: PriceLineOptions) -> Result<()> {
        if !opts.price.is_finite() {
            return Err(Error::InvalidArgument("the price must be finite"));
        }
        let line = self
            .price_lines
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or(Error::UnknownId)?;
        line.opts = opts;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Options of a price line; `None` if it does not exist.
    pub fn price_line(&self, id: PriceLineId) -> Option<&PriceLineOptions> {
        self.price_lines
            .iter()
            .find(|l| l.id == id)
            .map(|l| &l.opts)
    }

    /// Removes a price line.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn remove_price_line(&mut self, id: PriceLineId) -> Result<()> {
        let n = self.price_lines.len();
        self.price_lines.retain(|l| l.id != id);
        if self.price_lines.len() == n {
            return Err(Error::UnknownId);
        }
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Attaches an indicator to `source`. Each output becomes a value
    /// series ([`indicator_series`](Self::indicator_series)) on the pane
    /// `placement` names; the chart keeps them up to date as the source
    /// changes, incrementally for `append` and `update_last`.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown source or pane;
    /// [`Error::InvalidArgument`] for a volume source, an indicator without
    /// outputs, or one that needs volume on a source without it.
    ///
    /// # Examples
    ///
    /// A minimal indicator: the close, doubled.
    ///
    /// ```
    /// use ferrochart_core::chart::SeriesOptions;
    /// use ferrochart_core::data::DataChange;
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::plugin::{
    ///     Indicator, IndicatorInput, IndicatorOutputs, OutputSpec, Placement,
    /// };
    /// use ferrochart_core::testing::random_walk;
    /// use ferrochart_core::chart::SeriesData;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// struct Double;
    /// impl Indicator for Double {
    ///     fn name(&self) -> &str { "double" }
    ///     fn outputs(&self) -> &[OutputSpec] {
    ///         const OUT: [OutputSpec; 1] = [OutputSpec::line("double")];
    ///         &OUT
    ///     }
    ///     fn default_placement(&self) -> Placement { Placement::NewPane }
    ///     fn warmup(&self) -> usize { 0 }
    ///     fn update(&mut self, input: &IndicatorInput<'_>, _: DataChange, out: &mut IndicatorOutputs) {
    ///         let start = out.start();
    ///         for (i, v) in out.column(0).iter_mut().enumerate() {
    ///             if let Some(b) = input.bar(start + i) {
    ///                 *v = 2.0 * b.close;
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let candles = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 50, 0, 60_000)?))?;
    /// let id = chart.add_indicator(candles, Box::new(Double), Placement::NewPane)?;
    /// assert_eq!(chart.indicator_series(id).map(<[_]>::len), Some(1));
    /// assert_eq!(chart.panes().count(), 2);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_indicator(
        &mut self,
        source: SeriesId,
        ind: Box<dyn Indicator>,
        placement: Placement,
    ) -> Result<IndicatorId> {
        let src = self.entry(source)?;
        if src.kind == SeriesKind::Volume {
            return Err(Error::InvalidArgument(
                "volume series have no data of their own; use the source series",
            ));
        }
        if src.kind == SeriesKind::Custom {
            return Err(Error::InvalidArgument(
                "indicators take built-in series as their source",
            ));
        }
        if ind.outputs().is_empty() {
            return Err(Error::InvalidArgument("the indicator has no outputs"));
        }
        let n_out = ind.outputs().len();
        if ind
            .outputs()
            .iter()
            .enumerate()
            .any(|(i, o)| o.fill_to.is_some_and(|(to, _)| to >= n_out || to == i))
        {
            return Err(Error::InvalidArgument(
                "fill_to must name another output of the indicator",
            ));
        }
        let has_data = !src.store.is_empty();
        if ind.needs_volume() && (is_value_kind(src.kind) || has_data && !src.store.has_volume()) {
            return Err(Error::InvalidArgument("the indicator needs volume"));
        }
        let src_pane = src.pane;
        let volume_pane = self
            .series
            .iter()
            .find(|e| e.kind == SeriesKind::Volume && e.opts.source == Some(source))
            .map(|e| e.pane);
        let on_volume = placement == Placement::Volume && volume_pane.is_some();
        let (pane, own_pane) = match (placement, volume_pane) {
            (Placement::Volume, Some(p)) => (p, None),
            (Placement::Overlay, _) => (src_pane, None),
            (Placement::Pane(p), _) => {
                if !self.panes.iter().any(|x| x.id == p) {
                    return Err(Error::UnknownId);
                }
                (p, None)
            }
            (Placement::NewPane | Placement::Volume, _) => {
                let p = self.add_pane(PaneOptions::default());
                (p, Some(p))
            }
        };
        let mut series = Vec::with_capacity(ind.outputs().len());
        for spec in ind.outputs() {
            let color = spec.color.unwrap_or_else(|| {
                let c = INDICATOR_PALETTE[self.palette_next % INDICATOR_PALETTE.len()];
                self.palette_next += 1;
                Rgba::hex(c)
            });
            let kind = match spec.kind {
                OutputKind::Line => SeriesKind::Line,
                OutputKind::Histogram => SeriesKind::Histogram,
                OutputKind::Points => SeriesKind::Points,
            };
            let opts = SeriesOptions {
                color: Some(color),
                // Dots are `line_width` across: 1 px would hardly show.
                line_width: Css(if kind == SeriesKind::Points { 3.0 } else { 1.0 }),
                price_line_visible: false,
                shift: spec.shift,
                ..SeriesOptions::default()
            };
            let id = self.insert_series(pane, kind, opts)?;
            if let Some(e) = self.series.iter_mut().find(|e| e.id == id) {
                e.derived = true;
                e.volume_scale = on_volume;
            }
            series.push(id);
        }
        let mut bands = Vec::new();
        for (i, spec) in ind.outputs().iter().enumerate() {
            if let Some((to, opts)) = spec.fill_to
                && let (Some(&upper), Some(&lower)) = (series.get(i), series.get(to))
            {
                bands.push(self.add_band(upper, lower, opts)?);
            }
        }
        let id = IndicatorId(self.next_indicator);
        self.next_indicator += 1;
        self.indicators.push(IndicatorEntry {
            id,
            source,
            ind,
            series,
            own_pane,
            bands,
        });
        self.update_indicator(self.indicators.len() - 1, DataChange::Replace);
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// Removes an indicator, its output series, the indicators attached to
    /// them, and the pane `add_indicator` created for it if it is now empty.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn remove_indicator(&mut self, id: IndicatorId) -> Result<()> {
        let n = self
            .indicators
            .iter()
            .position(|x| x.id == id)
            .ok_or(Error::UnknownId)?;
        self.drop_indicator(n);
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// The output series of an indicator, in the order of its outputs;
    /// `None` if it does not exist.
    pub fn indicator_series(&self, id: IndicatorId) -> Option<&[SeriesId]> {
        self.indicators
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.series.as_slice())
    }

    /// The bands an indicator's `OutputSpec::fill_to` created, e.g.
    /// to change their colors with [`set_band_options`](Self::set_band_options);
    /// `None` if it does not exist. They are removed with the indicator.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::{BandOptions, SeriesData, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::Rgba;
    /// use ferrochart_core::plugin::{Indicator, IndicatorInput, IndicatorOutputs, OutputSpec, Placement};
    /// use ferrochart_core::data::DataChange;
    /// use ferrochart_core::testing::random_walk;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// /// The high and the low, filled between.
    /// struct Range([OutputSpec; 2]);
    /// impl Indicator for Range {
    ///     fn name(&self) -> &str { "range" }
    ///     fn outputs(&self) -> &[OutputSpec] { &self.0 }
    ///     fn default_placement(&self) -> Placement { Placement::Overlay }
    ///     fn warmup(&self) -> usize { 0 }
    ///     fn update(&mut self, input: &IndicatorInput<'_>, _: DataChange, out: &mut IndicatorOutputs) {
    ///         let start = out.start();
    ///         for k in 0..2 {
    ///             for (i, v) in out.column(k).iter_mut().enumerate() {
    ///                 if let Some(b) = input.bar(start + i) {
    ///                     *v = if k == 0 { b.high } else { b.low };
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let candles = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 50, 0, 60_000)?))?;
    /// let high = OutputSpec { fill_to: Some((1, BandOptions::default())), ..OutputSpec::line("high") };
    /// let id = chart.add_indicator(candles, Box::new(Range([high, OutputSpec::line("low")])), Placement::Overlay)?;
    /// let band = chart.indicator_bands(id).unwrap()[0];
    /// chart.set_band_options(band, BandOptions { fill: Some(Rgba::hex(0x0026_A69A).with_alpha(0x30)), fill_crossed: None })?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn indicator_bands(&self, id: IndicatorId) -> Option<&[BandId]> {
        self.indicators
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.bands.as_slice())
    }

    fn drop_indicator(&mut self, n: usize) {
        if n >= self.indicators.len() {
            return;
        }
        let entry = self.indicators.remove(n);
        for &out in &entry.series {
            self.drop_indicators_of(out);
        }
        self.series.retain(|e| !entry.series.contains(&e.id));
        self.price_lines
            .retain(|l| self.series.iter().any(|e| e.id == l.series));
        self.prune_bands();
        if let Some(p) = entry.own_pane
            && self.panes.len() > 1
            && !self.series.iter().any(|e| e.pane == p)
        {
            let _ = self.remove_pane(p);
        }
    }

    /// Drops the indicators whose source is `series`.
    fn drop_indicators_of(&mut self, series: SeriesId) {
        while let Some(n) = self.indicators.iter().position(|x| x.source == series) {
            self.drop_indicator(n);
        }
    }

    /// Brings the indicators up to date after `change` to series `id`.
    fn after_data(&mut self, id: SeriesId, change: DataChange) {
        if core::mem::take(&mut self.axis_rebuilt) {
            // Everything, starting with the indicators on data series.
            let roots = (0..self.indicators.len())
                .filter(|&n| {
                    let src = self.indicators[n].source;
                    !self.indicators.iter().any(|x| x.series.contains(&src))
                })
                .collect();
            self.recompute_levels(roots);
        } else {
            self.run_indicators(id, change);
        }
    }

    /// Updates the indicators on `source`, then those on their outputs.
    fn run_indicators(&mut self, source: SeriesId, change: DataChange) {
        if !matches!(change, DataChange::Append { .. } | DataChange::UpdateLast) {
            let roots = (0..self.indicators.len())
                .filter(|&n| self.indicators[n].source == source)
                .collect();
            self.recompute_levels(roots);
            return;
        }
        for n in 0..self.indicators.len() {
            if self.indicators[n].source == source {
                self.update_indicator(n, change);
                for o in 0..self.indicators[n].series.len() {
                    let out = self.indicators[n].series[o];
                    self.run_indicators(out, change);
                }
            }
        }
    }

    /// Full recomputation of the indicators `level` and then, level by
    /// level, of the indicators on their outputs. With the `rayon`
    /// feature the indicators of a level are computed in parallel; outputs
    /// are written in order, so the result does not depend on the feature.
    fn recompute_levels(&mut self, mut level: Vec<usize>) {
        while !level.is_empty() {
            self.recompute_level(&level);
            let outs: Vec<SeriesId> = level
                .iter()
                .flat_map(|&n| self.indicators[n].series.iter().copied())
                .collect();
            level = (0..self.indicators.len())
                .filter(|&n| outs.contains(&self.indicators[n].source))
                .collect();
        }
    }

    fn recompute_level(&mut self, level: &[usize]) {
        #[cfg(feature = "rayon")]
        if level.len() > 1 {
            use rayon::prelude::*;
            let index = &self.time_index;
            // Sources first: series entries hold plugins that are not `Sync`.
            // Plain sources only; indicators on custom series run after.
            let sources: Vec<Option<crate::series::PlainSource<'_>>> = (0..self.indicators.len())
                .map(|n| {
                    let src = self.indicators[n].source;
                    level
                        .contains(&n)
                        .then(|| self.series.iter().find(|e| e.id == src))
                        .flatten()
                        .and_then(|e| match e.store.source() {
                            crate::series::Source::Plain(p) => Some(p),
                            crate::series::Source::Custom(_) => None,
                        })
                })
                .collect();
            let results: Vec<_> = self
                .indicators
                .par_iter_mut()
                .zip(sources.par_iter())
                .enumerate()
                .filter_map(|(n, (entry, source))| {
                    let mut out = IndicatorOutputs::default();
                    let source = crate::series::Source::Plain((*source)?);
                    let done =
                        compute_indicator(entry, source, index, DataChange::Replace, &mut out);
                    Some((n, done, out))
                })
                .collect();
            let done: Vec<usize> = results.iter().map(|r| r.0).collect();
            for (n, (change, start), out) in results {
                self.write_indicator(n, change, start, &out);
            }
            for &n in level.iter().filter(|n| !done.contains(n)) {
                self.update_indicator(n, DataChange::Replace);
            }
            return;
        }
        for &n in level {
            self.update_indicator(n, DataChange::Replace);
        }
    }

    /// Runs indicator `n` for `change` and writes its outputs.
    fn update_indicator(&mut self, n: usize, change: DataChange) {
        let Some(entry) = self.indicators.get_mut(n) else {
            return;
        };
        let Some(source) = self.series.iter().find(|e| e.id == entry.source) else {
            return;
        };
        let mut out = core::mem::take(&mut self.ind_out);
        let (change, start) = compute_indicator(
            entry,
            source.store.source(),
            &self.time_index,
            change,
            &mut out,
        );
        self.write_indicator(n, change, start, &out);
        if change == DataChange::Replace {
            out.shrink();
        }
        self.ind_out = out;
    }

    /// Writes the outputs `out` of indicator `n` computed for `change` from
    /// slot `start` into its output series.
    fn write_indicator(
        &mut self,
        n: usize,
        change: DataChange,
        start: usize,
        out: &IndicatorOutputs,
    ) {
        let Some(entry) = self.indicators.get(n) else {
            return;
        };
        let Some(k) = self.series.iter().position(|e| e.id == entry.source) else {
            return;
        };
        let input = IndicatorInput::new(self.series[k].store.source(), &self.time_index);
        let len = input.len();
        let first = input.first();
        let outputs = out.columns().len();
        for o in 0..outputs.min(entry.series.len()) {
            let id = entry.series[o];
            let Some(ko) = self.series.iter().position(|e| e.id == id) else {
                continue;
            };
            let col = out.columns().get(o).map_or(&[][..], Vec::as_slice);
            let src = &self.series[k].store;
            let index = &self.time_index;
            // Points at the source's bars; gap slots are left to the value
            // series' rules (lines interpolate, histograms break).
            if change == DataChange::UpdateLast {
                let j = first + i64::try_from(start).unwrap_or(0);
                let (Some(time), Some(&v)) = (index.time_at(j), col.first()) else {
                    continue;
                };
                if src.bar_at(j).is_none() {
                    continue;
                }
                let bar = BarUpdate {
                    time,
                    open: v,
                    high: v,
                    low: v,
                    close: v,
                    volume: None,
                };
                let entry = &mut self.series[ko];
                let op = Op {
                    kind: entry.kind,
                    index,
                    mode: ValidationMode::Repair,
                };
                let _ = op.update_last(&mut entry.store, bar);
                continue;
            }
            let mut time = Vec::new();
            let mut value = Vec::new();
            for (i, &v) in col.iter().enumerate() {
                let j = first + i64::try_from(start + i).unwrap_or(0);
                if src.bar_at(j).is_some()
                    && let Some(t) = index.time_at(j)
                {
                    time.push(t);
                    value.push(v);
                }
            }
            if time.is_empty() && change != DataChange::Replace {
                continue;
            }
            let data = SeriesData::Value(crate::data::ValueColumns {
                time,
                value,
                color: None,
            });
            let entry = &mut self.series[ko];
            let op = Op {
                kind: entry.kind,
                index,
                mode: ValidationMode::Repair,
            };
            let done = if change == DataChange::Replace {
                if len == 0 {
                    entry.store = Store::for_kind(entry.kind);
                    Ok(0)
                } else {
                    op.set(&mut entry.store, data)
                }
            } else {
                op.append(&mut entry.store, data)
            };
            if done.is_err() {
                // Only an allocation failure: show nothing rather than stale values.
                entry.store = Store::for_kind(entry.kind);
            }
        }
    }

    /// Replaces the markers of a series. They are kept sorted by time
    /// (stable, so markers on the same bar stack in the given order).
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the series does not exist;
    /// [`Error::InvalidArgument`] for a volume series, or a marker with a
    /// size outside 4..=64, an icon or label on an arrow, both an icon and
    /// a label, a label over 8 characters or an invalid border width (the
    /// markers are then left as they were).
    ///
    /// # Examples
    ///
    /// See [`Marker`].
    pub fn set_markers(&mut self, series: SeriesId, mut markers: Vec<Marker>) -> Result<()> {
        let k = self.entry_index(series)?;
        if self.series[k].kind == SeriesKind::Volume {
            return Err(Error::InvalidArgument("volume series have no markers"));
        }
        markers.iter().try_for_each(check_marker)?;
        markers.sort_by_key(|m| m.time);
        self.series[k].markers = markers;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Registers an image for markers and plugin icons (r21). It is
    /// resampled to a 64×64 tile once; renderers upload it on their next
    /// frame. Not on the hot path.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for a side outside 1..=256 or a buffer that
    /// is not `width × height × 4` bytes; [`Error::CapacityExceeded`] beyond
    /// 1024 images.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::icon::IconImage;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let red = [255, 0, 0, 255].repeat(16 * 16);
    /// let id = chart.add_icon(IconImage { width: 16, height: 16, rgba: &red })?;
    /// assert_eq!(chart.data_view().icons().len(), 1);
    /// chart.remove_icon(id)?;
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn add_icon(&mut self, image: crate::icon::IconImage<'_>) -> Result<crate::icon::IconId> {
        let id = self.icons.add(image)?;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(id)
    }

    /// Removes an image. Markers that use it draw their shape alone.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if there is no such image.
    pub fn remove_icon(&mut self, id: crate::icon::IconId) -> Result<()> {
        self.icons.remove(id)?;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Markers of a series, sorted by time; `None` if it does not exist.
    pub fn markers(&self, series: SeriesId) -> Option<&[Marker]> {
        self.entry(series).ok().map(|e| e.markers.as_slice())
    }

    /// Replaces the bar quality flags of an OHLC series (r22):
    /// [`BarFlags::FILLED`] bars are drawn hollow, [`BarFlags::CLIPPED`]
    /// bars get a tick at each end of the wick. Sorted by time; of equal
    /// times the last wins; [`BarFlags::NONE`] entries are dropped. Times
    /// that are not bars of the series are ignored when drawing. At an LOD
    /// level a merged bar is `FILLED` when all its bars are, `CLIPPED` when
    /// any is.
    ///
    /// # Errors
    ///
    /// The series' lookup error; [`Error::InvalidArgument`] for a series
    /// that is not candlestick, OHLC bar or Heikin-Ashi, or unknown flag
    /// bits; [`Error::CapacityExceeded`] beyond 1 000 000 flags.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::chart::{BarFlags, SeriesOptions};
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
    /// chart.set_bar_flags(id, vec![(60_000, BarFlags::FILLED), (0, BarFlags::CLIPPED)])?;
    /// assert_eq!(chart.bar_flags(id).map(|f| f[0].0), Some(0));
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn set_bar_flags(
        &mut self,
        series: SeriesId,
        mut flags: Vec<(Timestamp, BarFlags)>,
    ) -> Result<()> {
        let k = self.entry_index(series)?;
        if !is_ohlc_kind(self.series[k].kind) {
            return Err(Error::InvalidArgument("bar flags need an OHLC series"));
        }
        if flags.len() > MAX_BAR_FLAGS {
            return Err(Error::CapacityExceeded("at most 1 000 000 bar flags"));
        }
        if flags.iter().any(|f| !BarFlags::ALL.contains(f.1)) {
            return Err(Error::InvalidArgument("unknown bar flag bits"));
        }
        // Reversed, then stably sorted: the first of equal times is the last given.
        flags.reverse();
        flags.sort_by_key(|f| f.0);
        flags.dedup_by_key(|f| f.0);
        flags.retain(|f| f.1 != BarFlags::NONE);
        self.series[k].bar_flags = flags;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Bar quality flags of a series, sorted by time; `None` if it does not
    /// exist (r22).
    pub fn bar_flags(&self, series: SeriesId) -> Option<&[(Timestamp, BarFlags)]> {
        self.entry(series).ok().map(|e| e.bar_flags.as_slice())
    }

    /// The (global) time scale.
    pub fn time_scale(&mut self) -> &mut TimeScale {
        &mut self.time_scale
    }

    /// A price scale of a pane; `None` if it does not exist (for now every
    /// pane has only its right scale).
    pub fn price_scale(&mut self, pane: PaneId, side: ScaleSide) -> Option<&mut PriceScale> {
        if side != ScaleSide::Right {
            return None;
        }
        self.pane_mut(pane).map(|p| &mut p.price_scale)
    }

    /// Shows all bars.
    pub fn fit_content(&mut self) {
        self.time_scale.fit_content();
    }

    /// Scrolls so that the last bar is visible with the configured
    /// [`ChartOptions::right_offset`].
    pub fn scroll_to_realtime(&mut self) {
        self.time_scale.set_right_offset(self.default_right_offset);
    }

    /// What needs to be redrawn. The host only builds a frame when this is not
    /// `None`; without interaction or new data, nothing is redrawn.
    pub fn needs_redraw(&self) -> Invalidation {
        let mut inv = self.invalidation;
        if self.time_scale.changed
            || self.drawings.dirty
            || self.input.kinetic.is_some()
            || self.panes.iter().any(|p| p.price_scale.changed)
        {
            inv = inv.max(Invalidation::Light);
        }
        inv.max(self.sync_pending())
    }

    /// The last built frame.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// Read-only view of the chart data for renderers.
    pub fn data_view(&self) -> ChartDataView<'_> {
        ChartDataView {
            series: &self.series,
            time_index: &self.time_index,
            icons: self.icons.view(),
        }
    }

    /// The last built layout.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Shows the data gradually: with `Some`, series, indicators,
    /// bands, markers, drawings and pane plugin layers are clipped to the
    /// left `progress` of each pane, while grid and axes are drawn whole and
    /// the price scale still fits all visible data; price lines, value
    /// labels and the crosshair wait for progress 1. `None` (the default)
    /// shows everything. The host moves the progress over time.
    ///
    /// ```
    /// use ferrochart_core::chart::{Chart, ChartOptions};
    /// use ferrochart_core::frame::Reveal;
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.set_reveal(Some(Reveal { progress: 0.5, pen: None }));
    /// assert_eq!(chart.reveal().map(|r| r.progress), Some(0.5));
    /// chart.set_reveal(None);
    /// assert!(chart.reveal().is_none());
    /// ```
    pub fn set_reveal(&mut self, reveal: Option<Reveal>) {
        if self.reveal != reveal {
            self.reveal = reveal;
            self.invalidation = self.invalidation.max(Invalidation::Light);
        }
    }

    /// The gradual display set by [`set_reveal`](Self::set_reveal).
    pub fn reveal(&self) -> Option<Reveal> {
        self.reveal
    }

    /// Where the data of a pane with plot area `plot` is drawn.
    fn data_clip(&self, plot: RectPx) -> RectPx {
        self.reveal.map_or(plot, |r| r.clip(plot))
    }

    /// Whether a reveal is under way: price lines, value labels and the
    /// crosshair wait for its end.
    fn revealing(&self) -> bool {
        self.reveal.is_some_and(|r| !r.is_done())
    }

    /// Heap bytes held by the chart's series data: columns, LOD pyramid and time
    /// index. Used by the memory benchmark (P2).
    pub fn heap_bytes(&self) -> usize {
        self.series
            .iter()
            .map(|e| e.store.heap_bytes())
            .sum::<usize>()
            + self.time_index.len() * core::mem::size_of::<i64>()
    }

    /// Takes all pending events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = ChartEvent> + '_ {
        self.events.append(&mut self.drawings.events);
        self.events.drain(..)
    }

    /// Resolved colors and line settings of a series.
    fn style_of(&self, e: &SeriesEntry) -> SeriesStyle {
        let t = &self.theme;
        let o = &e.opts;
        let mut st = SeriesStyle {
            line_width: Px(o.line_width.to_px(self.dpr).0.round().max(1.0)),
            line_dash: o.line_style,
            step: o.step,
            base_value: o.base_value,
            ..SeriesStyle::default()
        };
        let (up, down) = match e.kind {
            SeriesKind::Line | SeriesKind::Area | SeriesKind::Histogram | SeriesKind::Points => {
                let c = o.color.unwrap_or(if e.kind == SeriesKind::Histogram {
                    t.up
                } else {
                    t.line
                });
                (c, c)
            }
            SeriesKind::Volume => (t.volume_up, t.volume_down),
            _ => (o.color.unwrap_or(t.up), o.down_color.unwrap_or(t.down)),
        };
        st.up = up;
        st.down = down;
        st.up_border = up;
        st.down_border = down;
        let themed_wicks = o.color.is_none() && o.down_color.is_none();
        st.up_wick = if themed_wicks { t.up_wick } else { up };
        st.down_wick = if themed_wicks { t.down_wick } else { down };
        let (top, bottom) = match e.kind {
            SeriesKind::Area => (up.with_alpha(0x66), up.with_alpha(0x05)),
            _ => (up.with_alpha(0x33), down.with_alpha(0x33)),
        };
        st.fill_top = o.top_fill.unwrap_or(top);
        st.fill_bottom = o.bottom_fill.unwrap_or(bottom);
        st
    }

    /// Auto-scales every pane's price scale to the visible data of its series
    /// and chooses its label format.
    fn autoscale(&mut self) {
        let (from, to) = self.time_scale.visible_range();
        let (a, b) = (from.floor() as i64, to.ceil() as i64 + 1);
        let spacing = self.time_scale.bar_spacing();
        for i in 0..self.panes.len() {
            let id = self.panes[i].id;
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            let mut precision = None;
            let mut any_price = false;
            let mut max_volume: Option<f64> = None;
            for k in 0..self.series.len() {
                if self.series[k].pane != id {
                    continue;
                }
                if self.series[k].kind == SeriesKind::Volume {
                    let source = self.series[k]
                        .opts
                        .source
                        .and_then(|s| self.series.iter().find(|e| e.id == s));
                    if let Some(src) = source
                        && src.store.has_volume()
                    {
                        let level = lod_level(src.store.lod(), spacing);
                        if let Some(view) = src.store.level_view(level) {
                            let span = visible_span(&view, from, to);
                            let m = max_volume_in(&view, span);
                            max_volume = Some(max_volume.unwrap_or(0.0).max(m));
                        }
                    }
                    continue;
                }
                if self.series[k].volume_scale {
                    // A line on the volume scale (`Placement::Volume`).
                    let e = &mut self.series[k];
                    e.norm = Some(Normalize::IDENTITY);
                    let s = i64::from(e.opts.shift);
                    if let Some((_, h)) = e.store.extremes(a - s, b - s) {
                        max_volume = Some(max_volume.unwrap_or(0.0).max(h));
                    }
                    continue;
                }
                let mode = self.panes[i].price_scale.mode();
                let e = &mut self.series[k];
                precision.get_or_insert(e.opts.price_precision);
                if e.store.is_empty() {
                    e.norm = None;
                    continue;
                }
                any_price = true;
                let s = i64::from(e.opts.shift);
                let (a, b) = (a - s, b - s);
                let base = mode
                    .is_relative()
                    .then(|| first_value(&e.store, a, b))
                    .flatten();
                e.norm = Normalize::new(mode, base);
                let Some(norm) = e.norm else { continue };
                let extremes = match e.store.custom() {
                    Some(c) => c.extremes(&self.time_index, a, b),
                    None => e.store.extremes(a, b),
                };
                if let Some((l, h)) = extremes {
                    e.auto_decimals = auto_decimals(l.abs().max(h.abs()));
                    // k > 0: the order is kept.
                    lo = lo.min(norm.apply(l));
                    hi = hi.max(norm.apply(h));
                    if e.kind == SeriesKind::Histogram {
                        let base = norm.apply(e.opts.base_value);
                        lo = lo.min(base);
                        hi = hi.max(base);
                    }
                }
            }
            let pane = &mut self.panes[i];
            pane.has_content = any_price || max_volume.is_some();
            pane.labels = match pane.price_scale.mode() {
                _ if !any_price && max_volume.is_some() => NumberFormat::Compact,
                crate::price_scale::PriceScaleMode::Percentage => NumberFormat::Percent(2),
                crate::price_scale::PriceScaleMode::IndexedTo100 => NumberFormat::Fixed(2),
                _ => match precision.flatten() {
                    Some(p) => NumberFormat::Fixed(p),
                    None if lo <= hi => NumberFormat::Auto(auto_decimals(lo.abs().max(hi.abs()))),
                    None => NumberFormat::Auto(2),
                },
            };
            if any_price && lo <= hi {
                pane.price_scale.fit(lo, hi);
            }
            if let Some(m) = max_volume {
                if any_price {
                    pane.volume_scale.fit(0.0, m);
                } else {
                    pane.price_scale.fit(0.0, m);
                }
            }
        }
    }

    /// Whether pane `i` has something to scale (and therefore ticks).
    fn pane_has_content(&self, i: usize) -> bool {
        self.panes.get(i).is_some_and(|p| p.has_content)
    }

    /// Builds the frame for the current state. Call when
    /// [`needs_redraw`](Self::needs_redraw) is not `None`, then read it with
    /// [`frame`](Self::frame) and [`data_view`](Self::data_view).
    ///
    /// `now_ms` drives kinetic scrolling; it must use the same clock as the
    /// `time_ms` of input events (see [`interaction`](crate::interaction)).
    ///
    /// Every layer is always rebuilt, so the frame is complete for any
    /// renderer; only the revisions of layers whose content changed are bumped:
    /// `Cursor` bumps only the overlay.
    pub fn build_frame(&mut self, now_ms: f64, text: &dyn TextMeasurer) {
        self.step_kinetic(now_ms);
        self.pull_sync();
        let inv = self.needs_redraw();
        if inv >= Invalidation::Light {
            self.revisions[LayerId::Background as usize] += 1;
            self.revisions[LayerId::Main as usize] += 1;
        }
        if inv >= Invalidation::Cursor {
            self.revisions[LayerId::Overlay as usize] += 1;
        }
        let w = (self.size.w * self.dpr).round();
        let h = (self.size.h * self.dpr).round();
        self.frame
            .begin(SizePx { w, h }, self.theme.background, inv);
        for (layer, rev) in self.frame.layers.iter_mut().zip(self.revisions) {
            layer.revision = rev;
        }
        self.invalidation = Invalidation::None;
        self.time_scale.changed = false;
        self.drawings.dirty = false;
        for pane in &mut self.panes {
            pane.price_scale.changed = false;
        }
        if w < 1.0 || h < 1.0 {
            return;
        }

        let font = FontStyleRef {
            font: FontId(0),
            size: self.theme.font_size.to_px(self.dpr).0,
        };
        let line_h = text.line_height(font);
        if font.size > 0.0 {
            self.text_cache = CachedText {
                advances: *text.ascii_advances(font),
                subscripts: text.subscript_advances(font).copied().unwrap_or([0.6; 10]),
                line: line_h / font.size,
            };
        }
        let pad = self.theme.axis_padding.to_px(self.dpr).0.round();
        let time_h = (line_h + 2.0 * pad).ceil().min(h);
        let panes_h = (h - time_h).max(0.0);

        // The price axis width (shared by all panes) depends on the labels,
        // which depend on the visible range, which depends on the plot width:
        // iterate (at most twice).
        for _ in 0..2 {
            let plot_w = (w - self.axis_width).max(0.0);
            self.time_scale.set_width(f64::from(plot_w));
            stack(&mut self.panes, 0.0, panes_h, plot_w);
            self.autoscale();
            let mut widest: f32 = 0.0;
            for i in 0..self.panes.len() {
                let has_content = self.pane_has_content(i);
                let pane = &mut self.panes[i];
                if has_content {
                    pane.price_scale.ticks(
                        f64::from(pane.plot.h),
                        f64::from(line_h) * 2.5,
                        &mut pane.ticks,
                    );
                } else {
                    pane.ticks.clear();
                }
                let labels = pane.labels.for_ticks(&pane.ticks);
                for &p in &pane.ticks {
                    let mut s = StackStr::<64>::new();
                    write_label(&mut s, p, labels);
                    widest = widest.max(text_width(text, s.as_str(), font));
                }
            }
            let required = (widest + 2.0 * pad).ceil().min(w);
            if required > self.axis_width || required < self.axis_width * 0.75 {
                self.axis_width = required;
            } else {
                break;
            }
        }
        let plot_w = (w - self.axis_width).max(0.0);
        self.time_scale.set_width(f64::from(plot_w));
        self.time_scale.settle();
        stack(&mut self.panes, 0.0, panes_h, plot_w);
        self.layout = Layout {
            plot: self.panes.first().map_or(RectPx::default(), |p| p.plot),
            price_axis: RectPx {
                x: plot_w,
                y: 0.0,
                w: w - plot_w,
                h: panes_h,
            },
            time_axis: RectPx {
                x: 0.0,
                y: panes_h,
                w,
                h: h - panes_h,
            },
        };

        self.place_time_ticks(text, font);
        self.emit_layers(None, LayerPosition::BelowSeries, text);
        self.marker_hits.clear();
        for i in 0..self.panes.len() {
            self.emit_grid(i);
            self.emit_layers(Some(i), LayerPosition::Background, text);
            self.emit_layers(Some(i), LayerPosition::BelowSeries, text);
            self.emit_series(i, text);
            self.emit_markers(i, line_h, font, text);
            self.emit_layers(Some(i), LayerPosition::AboveSeries, text);
            self.emit_drawings(i, text);
            self.emit_marks(i, line_h, pad, font);
            self.emit_price_labels(i, line_h, pad, font);
        }
        self.emit_layers(None, LayerPosition::Background, text);
        self.emit_layers(None, LayerPosition::AboveSeries, text);
        self.emit_axes(time_h, font);
        for i in 0..self.panes.len() {
            self.emit_layers(Some(i), LayerPosition::Overlay, text);
        }
        self.emit_layers(None, LayerPosition::Overlay, text);
        if let Some(r) = self.reveal {
            for pane in &self.panes {
                r.push_pen(&mut self.frame, LayerId::Overlay, pane.plot, self.dpr);
            }
        }
        let local_crosshair = self.emit_crosshair(time_h, pad, font);
        self.emit_marker_hover();
        self.emit_view_events();
        self.push_sync(local_crosshair.map(|i| i as f64));
    }

    /// The drawings.
    pub fn drawings(&mut self) -> &mut DrawingManager {
        &mut self.drawings
    }

    /// Adds a band: the area between two single-value series of the same
    /// pane, filled with `opts.fill` where `upper` is at or above `lower`
    /// and `opts.fill_crossed` where it is below. Bands are drawn
    /// after the volume and before the other series of the pane, and are
    /// removed with either series. See [`BandOptions`] for an example.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if a series does not exist;
    /// [`Error::InvalidArgument`] unless both are single-value series
    /// (line, area, baseline, histogram, points) on the same pane and
    /// different.
    pub fn add_band(
        &mut self,
        upper: SeriesId,
        lower: SeriesId,
        opts: BandOptions,
    ) -> Result<BandId> {
        let (u, l) = (self.entry(upper)?, self.entry(lower)?);
        if !is_value_kind(u.kind) || !is_value_kind(l.kind) {
            return Err(Error::InvalidArgument(
                "a band is between two single-value series",
            ));
        }
        if u.pane != l.pane || upper == lower {
            return Err(Error::InvalidArgument(
                "a band is between two different series of the same pane",
            ));
        }
        let id = BandId(self.next_band);
        self.next_band += 1;
        self.bands.push(BandEntry {
            id,
            upper,
            lower,
            opts,
        });
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// Changes the colors of a band.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn set_band_options(&mut self, id: BandId, opts: BandOptions) -> Result<()> {
        let b = self
            .bands
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::UnknownId)?;
        b.opts = opts;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// The colors of a band; `None` if it does not exist.
    pub fn band_options(&self, id: BandId) -> Option<&BandOptions> {
        self.bands.iter().find(|b| b.id == id).map(|b| &b.opts)
    }

    /// Removes a band.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn remove_band(&mut self, id: BandId) -> Result<()> {
        let n = self
            .bands
            .iter()
            .position(|b| b.id == id)
            .ok_or(Error::UnknownId)?;
        self.bands.remove(n);
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Reports a click at `p` in `pane`; `drawing` is the drawing
    /// pressed, if any.
    pub(crate) fn emit_click(&mut self, pane: PaneId, p: PointPx, drawing: Option<DrawingId>) {
        let Some(pn) = self.pane_index(pane).and_then(|i| self.panes.get(i)) else {
            return;
        };
        let plot = pn.plot;
        let logical_index = self.time_scale.x_to_index(Px(p.x - plot.x));
        let price = pn
            .price_scale
            .y_to_price(f64::from(p.y), f64::from(plot.y), f64::from(plot.h));
        let time = self.time_index.time_at(logical_index.round() as i64);
        let target = drawing
            .or_else(|| self.hit_drawing(pane, p).map(|(d, _)| d))
            .map(ClickTarget::Drawing)
            .or_else(|| self.hit_marker(pane, p))
            .or_else(|| self.hit_series(pane, p).map(ClickTarget::Series));
        self.events.push(ChartEvent::Click {
            pos: PointCss {
                x: p.x / self.dpr,
                y: p.y / self.dpr,
            },
            pane,
            logical_index,
            time,
            price,
            target,
        });
    }

    /// The marker of `pane` under `p` in the last frame.
    fn hit_marker(&self, pane: PaneId, p: PointPx) -> Option<ClickTarget> {
        let tol = HIT_TOLERANCE * 0.5 * self.dpr;
        self.marker_hits
            .iter()
            .rev()
            .find(|m| m.pane == pane && (p.x - m.pos.x).hypot(p.y - m.pos.y) <= m.radius + tol)
            .map(|m| m.target)
    }

    /// Whether a pointer move can change the hovered marker: markers were
    /// drawn in the last frame, or one is hovered.
    pub(crate) fn tracks_marker_hover(&self) -> bool {
        !self.marker_hits.is_empty() || self.last_marker_hover.is_some()
    }

    /// `MarkerHover` when the marker under the pointer changed.
    fn emit_marker_hover(&mut self) {
        let spot = self
            .input
            .pointer
            .filter(|_| !self.revealing())
            .and_then(|p| {
                let pane = self.panes.get(self.pane_at(p)?)?.id;
                let tol = HIT_TOLERANCE * 0.5 * self.dpr;
                self.marker_hits
                    .iter()
                    .rev()
                    .find(|m| {
                        m.pane == pane && (p.x - m.pos.x).hypot(p.y - m.pos.y) <= m.radius + tol
                    })
                    .copied()
            });
        let hit = spot.and_then(|m| {
            let (series, index, id, overflow) = match m.target {
                ClickTarget::Marker { series, index, id } => (series, index, id, 0),
                ClickTarget::MarkerOverflow {
                    series,
                    first,
                    count,
                    id,
                    ..
                } => (series, first, id, count),
                _ => return None,
            };
            Some(MarkerHit {
                series,
                index,
                id,
                overflow,
                bars: m.bars,
                center: PointCss {
                    x: m.pos.x / self.dpr,
                    y: m.pos.y / self.dpr,
                },
                size: Css(m.radius * 2.0 / self.dpr),
            })
        });
        if hit != self.last_marker_hover {
            self.last_marker_hover = hit;
            self.push_event(ChartEvent::MarkerHover(hit));
        }
    }

    /// The topmost series of `pane` under `p`: on a candle's body or
    /// wick, within the tolerance of a line, inside a bar, on a dot.
    fn hit_series(&self, pane: PaneId, p: PointPx) -> Option<SeriesId> {
        let pn = self.panes.get(self.pane_index(pane)?)?;
        let plot = pn.plot;
        let (top, height) = (f64::from(plot.y), f64::from(plot.h));
        let has_price = self
            .series
            .iter()
            .any(|e| e.pane == pane && !volume_like(e) && !e.store.is_empty());
        let tol = HIT_TOLERANCE * self.dpr;
        let m = crate::transform_helpers::bar_metrics(self.time_scale.bar_spacing() as f32);
        let half_bar = (m.body * 0.5).max(tol * 0.5);
        let center = self.time_scale.x_to_index(Px(p.x - plot.x)).round() as i64;
        let within = |a: f32, b: f32| p.y >= a.min(b) - 1.0 && p.y <= a.max(b) + 1.0;
        for e in self.series.iter().rev().filter(|e| e.pane == pane) {
            let (store, shift) = if e.kind == SeriesKind::Volume {
                let src = e
                    .opts
                    .source
                    .and_then(|s| self.series.iter().find(|x| x.id == s));
                let Some(src) = src else { continue };
                (&src.store, 0)
            } else {
                (&e.store, i64::from(e.opts.shift))
            };
            let x_of = |j: i64| plot.x + self.time_scale.index_to_x((j + shift) as f64).0;
            let y_of = |v: f64| -> Option<f32> {
                let y = if volume_like(e) {
                    let scale = if has_price {
                        &pn.volume_scale
                    } else {
                        &pn.price_scale
                    };
                    scale.price_to_y(v, top, height)
                } else {
                    pn.price_scale.price_to_y(e.norm?.apply(v), top, height)
                };
                y.is_finite().then_some(y as f32)
            };
            let point = |j: i64| -> Option<PointPx> {
                let v = store.bar_at(j)?[3];
                Some(PointPx {
                    x: x_of(j),
                    y: y_of(v)?,
                })
            };
            let j = center - shift;
            let hit = match e.kind {
                k if is_ohlc_kind(k) => (|| {
                    let [o, h, l, c] = store.bar_at(j)?;
                    let dx = (p.x - x_of(j)).abs();
                    let (yo, yh, yl, yc) = (y_of(o)?, y_of(h)?, y_of(l)?, y_of(c)?);
                    Some(
                        (dx <= tol * 0.5 && within(yh, yl))
                            || (dx <= m.body * 0.5 && within(yo, yc)),
                    )
                })(),
                SeriesKind::Volume | SeriesKind::Histogram => (|| {
                    let (v, base) = if e.kind == SeriesKind::Volume {
                        (store.volume_at(j)?, 0.0)
                    } else {
                        (store.bar_at(j)?[3], e.opts.base_value)
                    };
                    let dx = (p.x - x_of(j)).abs();
                    Some(dx <= half_bar && within(y_of(v)?, y_of(base)?))
                })(),
                SeriesKind::Points => point(j).map(|c| {
                    let r = e.opts.line_width.to_px(self.dpr).0 * 0.5;
                    (p.x - c.x).hypot(p.y - c.y) <= r + tol * 0.5
                }),
                SeriesKind::Line | SeriesKind::Area | SeriesKind::Baseline => point(j).map(|c| {
                    [point(j - 1), point(j + 1)]
                        .into_iter()
                        .map(|n| {
                            n.map_or_else(
                                || (p.x - c.x).hypot(p.y - c.y),
                                |n| segment_distance(p, c, n),
                            )
                        })
                        .fold(f32::INFINITY, f32::min)
                        <= tol
                }),
                _ => None,
            };
            if hit == Some(true) {
                return Some(e.id);
            }
        }
        None
    }

    /// Drops the bands of removed series.
    fn prune_bands(&mut self) {
        let series = &self.series;
        self.bands.retain(|b| {
            series.iter().any(|e| e.id == b.upper) && series.iter().any(|e| e.id == b.lower)
        });
    }

    /// Adds a layer plugin to a pane, or to the whole chart (`pane: None`),
    /// drawn after the plugins added before it. See
    /// [`LayerPlugin`] for an example.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if the pane does not exist;
    /// [`Error::OutOfMemory`].
    pub fn add_layer(
        &mut self,
        pane: Option<PaneId>,
        plugin: Box<dyn LayerPlugin>,
    ) -> Result<LayerPluginId> {
        if pane.is_some_and(|p| self.pane_index(p).is_none()) {
            return Err(Error::UnknownId);
        }
        let id = LayerPluginId(self.next_layer);
        self.next_layer = self.next_layer.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.layer_plugins
            .try_reserve(1)
            .map_err(|_| Error::OutOfMemory)?;
        self.layer_plugins.push(LayerEntry {
            id,
            pane,
            position: plugin.position(),
            plugin,
        });
        self.invalidation = self.invalidation.max(Invalidation::Light);
        Ok(id)
    }

    /// Removes a layer plugin. Plugins of a pane are also removed with the
    /// pane.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn remove_layer(&mut self, id: LayerPluginId) -> Result<()> {
        let n = self
            .layer_plugins
            .iter()
            .position(|l| l.id == id)
            .ok_or(Error::UnknownId)?;
        self.layer_plugins.remove(n);
        self.invalidation = self.invalidation.max(Invalidation::Light);
        Ok(())
    }

    /// Requests a redraw after a layer plugin's own content changed.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if it does not exist.
    pub fn invalidate_layer(&mut self, id: LayerPluginId) -> Result<()> {
        if !self.layer_plugins.iter().any(|l| l.id == id) {
            return Err(Error::UnknownId);
        }
        self.invalidation = self.invalidation.max(Invalidation::Light);
        Ok(())
    }

    /// Layer plugins of pane `i` (or chart-wide ones, `None`) at `position`.
    fn emit_layers(
        &mut self,
        pane: Option<usize>,
        position: LayerPosition,
        text: &dyn TextMeasurer,
    ) {
        let pane_id = match pane {
            Some(i) => match self.panes.get(i) {
                Some(p) => Some(p.id),
                None => return,
            },
            None => None,
        };
        if !self
            .layer_plugins
            .iter()
            .any(|l| l.pane == pane_id && l.position == position)
        {
            return;
        }
        let norm = pane.and_then(|i| self.drawing_norm(i));
        let (rect, coords) = match pane {
            Some(i) => {
                let p = &self.panes[i];
                let coords = (!self.time_index.is_empty() && p.has_content)
                    .then_some(norm)
                    .flatten()
                    .map(|norm| DrawingCtx {
                        index: &self.time_index,
                        time_scale: &self.time_scale,
                        price_scale: &p.price_scale,
                        norm,
                        plot: p.plot,
                        dpr: self.dpr,
                        labels: p.labels,
                        text,
                    });
                (p.plot, coords)
            }
            None => (
                RectPx {
                    x: 0.0,
                    y: 0.0,
                    w: self.layout.price_axis.x,
                    h: self.layout.price_axis.h,
                },
                None,
            ),
        };
        let ctx = LayerCtx {
            rect,
            dpr: self.dpr,
            text,
            coords,
        };
        let layer = match position {
            LayerPosition::Background => LayerId::Background,
            LayerPosition::BelowSeries | LayerPosition::AboveSeries => LayerId::Main,
            LayerPosition::Overlay => LayerId::Overlay,
        };
        // Pane layers next to the series are revealed with them.
        let clip = match (pane, position) {
            (Some(_), LayerPosition::BelowSeries | LayerPosition::AboveSeries) => {
                self.data_clip(rect)
            }
            _ => rect,
        };
        let frame = &mut self.frame;
        frame.push_cmd(layer, DrawCmd::PushClip(clip));
        for l in &self.layer_plugins {
            if l.pane == pane_id && l.position == position {
                l.plugin.build(
                    &ctx,
                    &mut FrameBuilder {
                        frame: &mut *frame,
                        layer,
                    },
                );
            }
        }
        frame.push_cmd(layer, DrawCmd::PopClip);
    }

    /// Price map of the drawings of pane `i`: the map of the pane's first
    /// price series; `None` if a relative scale has no base yet.
    pub(crate) fn drawing_norm(&self, i: usize) -> Option<Normalize> {
        let pane = self.panes.get(i)?;
        if !pane.price_scale.mode().is_relative() {
            return Some(Normalize::IDENTITY);
        }
        self.series
            .iter()
            .find(|e| e.pane == pane.id && !e.derived && e.kind != SeriesKind::Volume)
            .and_then(|e| e.norm)
    }

    /// The coordinate context of drawings on pane `i`.
    pub(crate) fn drawing_ctx<'a>(
        &'a self,
        i: usize,
        text: &'a dyn TextMeasurer,
    ) -> Option<DrawingCtx<'a>> {
        let pane = self.panes.get(i)?;
        if self.time_index.is_empty() || !pane.has_content {
            return None;
        }
        Some(DrawingCtx {
            index: &self.time_index,
            time_scale: &self.time_scale,
            price_scale: &pane.price_scale,
            norm: self.drawing_norm(i)?,
            plot: pane.plot,
            dpr: self.dpr,
            labels: pane.labels,
            text,
        })
    }

    /// The drawings of pane `i` (main layer, above series and markers) and
    /// the handles of the selected one; records their screen bounds.
    fn emit_drawings(&mut self, i: usize, text: &dyn TextMeasurer) {
        let Some(pane) = self.panes.get(i) else {
            return;
        };
        let (id, plot) = (pane.id, pane.plot);
        if !self.drawings.entries.iter().any(|e| e.pane == id) {
            return;
        }
        let Some(norm) = self.drawing_norm(i) else {
            return;
        };
        if self.time_index.is_empty() || !pane.has_content {
            return;
        }
        let ctx = DrawingCtx {
            index: &self.time_index,
            time_scale: &self.time_scale,
            price_scale: &pane.price_scale,
            norm,
            plot,
            dpr: self.dpr,
            labels: pane.labels,
            text,
        };
        let clip = self.data_clip(plot);
        let frame = &mut self.frame;
        frame.push_cmd(LayerId::Main, DrawCmd::PushClip(clip));
        let selected = self.drawings.selected();
        let mut handles = self.handle_buf.borrow_mut();
        handles.clear();
        for e in &mut self.drawings.entries {
            if e.pane != id {
                continue;
            }
            let mark = PoolMark::of(frame);
            if e.visible {
                e.tool.build(
                    &ctx,
                    &mut FrameBuilder {
                        frame: &mut *frame,
                        layer: LayerId::Main,
                    },
                );
            }
            e.bbox = mark.bounds(frame, &ctx);
            if e.visible && selected == Some(e.id) {
                e.tool.handles(&ctx, &mut handles);
            }
        }
        // Handles: rings in the line color around the background color.
        let outer = (HANDLE_CSS * self.dpr).round();
        let inner = (outer - 2.0 * (1.5 * self.dpr).round()).max(2.0);
        let color = selected
            .and_then(|s| self.drawings.entry(s))
            .map_or(self.theme.line, |e| e.tool.style().line);
        for p in handles.iter() {
            let pos = PointPx {
                x: p.x.round(),
                y: p.y.round(),
            };
            for (size, c) in [(outer, color), (inner, self.theme.background)] {
                frame.push_icon(
                    LayerId::Main,
                    crate::frame::IconInst {
                        pos,
                        size: Px(size),
                        icon: crate::transform_helpers::ICON_CIRCLE,
                        color: c,
                        ..Default::default()
                    },
                );
            }
        }
        frame.push_cmd(LayerId::Main, DrawCmd::PopClip);
    }

    /// Adds an event, replacing a pending one of the same kind for
    /// position-like events.
    pub(crate) fn push_event(&mut self, event: ChartEvent) {
        let coalesce = matches!(
            event,
            ChartEvent::CrosshairMove { .. }
                | ChartEvent::MarkerHover(_)
                | ChartEvent::VisibleRangeChanged { .. }
                | ChartEvent::Sync(_)
        );
        let same_kind = |e: &ChartEvent| match (e, &event) {
            (ChartEvent::Sync(a), ChartEvent::Sync(b)) => {
                core::mem::discriminant(a) == core::mem::discriminant(b)
            }
            _ => core::mem::discriminant(e) == core::mem::discriminant(&event),
        };
        if coalesce && let Some(e) = self.events.iter_mut().find(|e| same_kind(e)) {
            *e = event;
            return;
        }
        self.events.push(event);
    }

    fn emit_view_events(&mut self) {
        if self.time_index.is_empty() {
            return;
        }
        let (from, to) = self.time_scale.visible_range();
        if (from, to) != self.last_range {
            self.last_range = (from, to);
            self.push_event(ChartEvent::VisibleRangeChanged { from, to });
        }
        let first = self.time_index.first_index();
        let before = from.floor() as i64 - first;
        if (before as f64) < to - from && self.history_requested != Some(first) {
            self.history_requested = Some(first);
            self.push_event(ChartEvent::NeedMoreHistory {
                bars_before_start: before,
            });
        }
    }

    /// Values at logical index `j` of the first OHLC series that has a bar
    /// there.
    fn bar_at(&self, j: i64) -> Option<BarValues> {
        let (e, [open, high, low, close]) = self
            .series
            .iter()
            .filter(|e| is_ohlc_kind(e.kind))
            .find_map(|e| Some((e, e.store.bar_at(j)?)))?;
        Some(BarValues {
            open,
            high,
            low,
            close,
            volume: e.store.volume_at(j),
        })
    }

    /// The bar a magnet snaps to at logical index `j` of `pane`: that of the
    /// pane's first price series, in its own prices.
    pub(crate) fn magnet_bar(&self, pane: PaneId, j: i64) -> Option<[f64; 4]> {
        self.series
            .iter()
            .find(|e| e.pane == pane && !volume_like(e) && !e.store.is_empty())
            .and_then(|e| e.store.bar_at(j - i64::from(e.opts.shift)))
    }

    /// Index of the pane whose plot contains `p`.
    pub(crate) fn pane_at(&self, p: PointPx) -> Option<usize> {
        self.panes.iter().position(|pane| pane.plot.contains(p))
    }

    /// The crosshair in the overlay layer, and its `CrosshairMove` event.
    /// The vertical line crosses every pane; the horizontal line and the price
    /// label belong to the pane under the pointer. Without a pointer over the
    /// plot, a crosshair received from a synced chart is drawn as the
    /// vertical line and time label only. Returns the logical index under the
    /// pointer.
    fn emit_crosshair(&mut self, time_h: f32, pad: f32, font: FontStyleRef) -> Option<i64> {
        let hovered = self
            .input
            .pointer
            .filter(|_| self.input.crosshair != CrosshairMode::Hidden && !self.revealing())
            .and_then(|p| Some((p, self.pane_at(p)?)));
        let local = hovered.and_then(|(p, i)| {
            let pane = self.panes.get(i)?;
            let plot = pane.plot;
            let idx = self.time_scale.x_to_index(Px(p.x - plot.x)).round() as i64;
            let x = plot.x + self.time_scale.index_to_x_f64(idx as f64) as f32;
            let bar = self.bar_at(idx);
            let (top, height) = (f64::from(plot.y), f64::from(plot.h));
            let mut y = p.y;
            let magnet = self
                .series
                .iter()
                .find(|e| e.pane == pane.id && !volume_like(e) && !e.store.is_empty())
                .and_then(|e| Some((e.store.bar_at(idx - i64::from(e.opts.shift))?, e.norm?)));
            if self.input.crosshair == CrosshairMode::Magnet
                && let Some((b, norm)) = magnet
            {
                let mut best = f32::INFINITY;
                for v in b {
                    let vy = pane.price_scale.price_to_y(norm.apply(v), top, height) as f32;
                    if (vy - p.y).abs() < (best - p.y).abs() {
                        best = vy;
                    }
                }
                if best.is_finite() {
                    y = best.clamp(plot.y, plot.bottom() - 1.0);
                }
            }
            let price = pane.price_scale.y_to_price(f64::from(y), top, height);
            Some((
                Some((i, y)),
                x,
                CrosshairPoint {
                    logical_index: idx,
                    time: self.time_index.time_at(idx),
                    pane: pane.id,
                    price,
                    bar,
                },
            ))
        });
        let local_index = local.map(|(_, _, c)| c.logical_index);
        let point = local.or_else(|| {
            if self.input.crosshair == CrosshairMode::Hidden {
                return None;
            }
            let idx = self.remote_crosshair_index()?;
            let plot = self.panes.first()?.plot;
            let x = plot.x + self.time_scale.index_to_x_f64(idx as f64) as f32;
            Some((
                None,
                x,
                CrosshairPoint {
                    logical_index: idx,
                    time: self.time_index.time_at(idx),
                    pane: self.main_pane(),
                    price: f64::NAN,
                    bar: self.bar_at(idx),
                },
            ))
        });
        let event_point = point.map(|(_, _, c)| c);
        if !same_crosshair(event_point.as_ref(), self.last_crosshair.as_ref()) {
            self.last_crosshair = event_point;
            self.push_event(ChartEvent::CrosshairMove { point: event_point });
        }
        let (row, x, info) = point?;
        let plot0 = self.panes.first().map_or(RectPx::default(), |p| p.plot);
        if row.is_none() && !(x >= plot0.x && x < plot0.right()) {
            return local_index;
        }
        let stroke = StrokeStyle {
            width: Px(1.0),
            color: self.theme.crosshair,
            dash: LineDash::Dashed,
        };
        let cx = x.floor() + 0.5;
        for k in 0..self.panes.len() {
            let plot = self.panes[k].plot;
            let (from, to) = (
                PointPx { x: cx, y: plot.y },
                PointPx {
                    x: cx,
                    y: plot.bottom(),
                },
            );
            self.frame
                .push_line(LayerId::Overlay, LineInst { from, to, stroke });
        }
        let bg = Some(LabelBg {
            color: self.theme.crosshair_label_bg,
            padding: Px((pad * 0.5).round()),
        });
        let color = self.theme.crosshair_label_text;
        if let Some((i, y)) = row
            && let Some(pane) = self.panes.get(i)
        {
            let (plot, labels) = (pane.plot, pane.labels);
            let cy = y.floor() + 0.5;
            let (from, to) = (
                PointPx { x: plot.x, y: cy },
                PointPx {
                    x: plot.right(),
                    y: cy,
                },
            );
            self.frame
                .push_line(LayerId::Overlay, LineInst { from, to, stroke });
            let start = self.frame.pools.text.len();
            write_label(&mut self.frame.pools.text, info.price, labels);
            let span = Span {
                start: u32::try_from(start).unwrap_or(0),
                len: u32::try_from(self.frame.pools.text.len() - start).unwrap_or(0),
            };
            self.frame.push_text(
                LayerId::Overlay,
                TextRun {
                    pos: PointPx {
                        x: plot.right() + pad,
                        y: y.round(),
                    },
                    text: span,
                    style: font,
                    color,
                    align: TextAlign::Left,
                    bg,
                },
            );
        }
        if let Some(t) = info.time {
            let mut s = StackStr::<32>::new();
            if format_crosshair_time(t + self.tz.offset_ms(t), &mut s).is_ok() {
                let span = self.frame.push_str(s.as_str());
                self.frame.push_text(
                    LayerId::Overlay,
                    TextRun {
                        pos: PointPx {
                            x: x.round(),
                            y: (self.layout.time_axis.y + time_h * 0.5).round(),
                        },
                        text: span,
                        style: font,
                        color,
                        align: TextAlign::Center,
                        bg,
                    },
                );
            }
        }
        local_index
    }

    /// Chooses time labels: candidates by calendar weight, highest weight first,
    /// keeping labels at least 1.5 label widths apart.
    fn place_time_ticks(&mut self, text: &dyn TextMeasurer, font: FontStyleRef) {
        self.placed.clear();
        if self.time_index.is_empty() {
            return;
        }
        let spacing = self.time_scale.bar_spacing();
        let mut shift = 0;
        while ((1_u64 << shift) as f64) * spacing < TICK_SAMPLE_PX && shift < 40 {
            shift += 1;
        }
        let (from, to) = self.time_scale.visible_range();
        collect_ticks(
            &self.time_index,
            self.tz.as_ref(),
            from.floor() as i64,
            to.ceil() as i64,
            shift,
            &mut self.time_ticks,
        );
        self.time_ticks
            .sort_unstable_by(|a, b| b.weight.cmp(&a.weight).then(a.index.cmp(&b.index)));
        let plot_w = self.layout.plot.w;
        for &tick in &self.time_ticks {
            let x = self.time_scale.index_to_x_f64(tick.index as f64) as f32;
            let Some(label) = self.tick_label(tick) else {
                continue;
            };
            let width = text_width(text, label.as_str(), font);
            if x - width * 0.5 < 0.0 || x + width * 0.5 > plot_w {
                continue;
            }
            let free = self
                .placed
                .iter()
                .all(|p| (p.x - x).abs() >= 1.5 * p.width.max(width));
            if free && self.placed.len() < self.placed.capacity() {
                self.placed.push(PlacedTick { x, width, tick });
            }
        }
    }

    fn tick_label(&self, tick: TimeTick) -> Option<StackStr<32>> {
        let t = self.time_index.time_at(tick.index)?;
        let mut s = StackStr::<32>::new();
        format_tick(t + self.tz.offset_ms(t), tick.weight, &mut s).ok()?;
        Some(s)
    }

    fn line(from: PointPx, to: PointPx, color: crate::geom::Rgba) -> LineInst {
        LineInst {
            from,
            to,
            stroke: StrokeStyle {
                width: Px(1.0),
                color,
                dash: LineDash::Solid,
            },
        }
    }

    fn emit_grid(&mut self, i: usize) {
        let grid = self.theme.grid;
        let Some(plot) = self.panes.get(i).map(|p| p.plot) else {
            return;
        };
        for k in 0..self.panes[i].ticks.len() {
            let pane = &self.panes[i];
            let y = pane
                .price_scale
                .price_to_y(pane.ticks[k], f64::from(plot.y), f64::from(plot.h))
                as f32;
            let y = y.floor() + 0.5;
            if y > plot.y && y < plot.bottom() {
                let l = Self::line(
                    PointPx { x: plot.x, y },
                    PointPx { x: plot.right(), y },
                    grid,
                );
                self.frame.push_line(LayerId::Background, l);
            }
        }
        for k in 0..self.placed.len() {
            let x = self.placed[k].x.floor() + 0.5 + plot.x;
            let l = Self::line(
                PointPx { x, y: plot.y },
                PointPx {
                    x,
                    y: plot.bottom(),
                },
                grid,
            );
            self.frame.push_line(LayerId::Background, l);
        }
    }

    /// Element range and x mapping of `view` for the visible range, for a
    /// series drawn `shift` bars later.
    fn draw_range(
        &self,
        view: &OhlcLevelView<'_>,
        plot: RectPx,
        shift: i32,
    ) -> (crate::data::IdxSpan, XTransform) {
        let (from, to) = self.time_scale.visible_range();
        let shift = f64::from(shift);
        let span = visible_span(view, from - shift, to - shift);
        let k = u32::from(view.level);
        let size = (1_i64 << k) as f64;
        let center = if k == 0 {
            span.start as f64
        } else {
            bucket_start(span.start, k) as f64 + (size - 1.0) * 0.5
        };
        let xt = XTransform {
            base: span.start,
            step: (self.time_scale.bar_spacing() * size) as f32,
            offset: (f64::from(plot.x) + self.time_scale.index_to_x_f64(center + shift)) as f32,
        };
        (span, xt)
    }

    /// The series of pane `i`: volume series first, then the others in the
    /// order they were added.
    fn emit_series(&mut self, i: usize, text: &dyn TextMeasurer) {
        let Some(pane) = self.panes.get(i) else {
            return;
        };
        let (id, plot) = (pane.id, pane.plot);
        let clip = self.data_clip(plot);
        self.frame.push_cmd(LayerId::Main, DrawCmd::PushClip(clip));
        let has_price = self
            .series
            .iter()
            .any(|e| e.pane == id && !volume_like(e) && !e.store.is_empty());
        for volumes in [true, false] {
            if !volumes {
                for b in 0..self.bands.len() {
                    self.emit_band(i, b);
                }
            }
            for k in 0..self.series.len() {
                let e = &self.series[k];
                if e.pane == id && (e.kind == SeriesKind::Volume) == volumes {
                    match e.kind {
                        SeriesKind::Custom => self.emit_custom(i, k, text),
                        SeriesKind::Kagi | SeriesKind::PointFigure => self.emit_figure(i, k),
                        _ => self.emit_one(i, k, has_price),
                    }
                }
            }
        }
        self.frame.push_cmd(LayerId::Main, DrawCmd::PopClip);
    }

    /// One `SeriesDraw` for series `k` in pane `i`.
    fn emit_one(&mut self, i: usize, k: usize, has_price: bool) {
        let (Some(pane), Some(e)) = (self.panes.get(i), self.series.get(k)) else {
            return;
        };
        let plot = pane.plot;
        let (top, height) = (f64::from(plot.y), f64::from(plot.h));
        let (data_id, store) = if e.kind == SeriesKind::Volume {
            let Some(src) = e
                .opts
                .source
                .and_then(|s| self.series.iter().find(|x| x.id == s))
            else {
                return;
            };
            if !src.store.has_volume() {
                return;
            }
            (src.id, &src.store)
        } else {
            (e.id, &e.store)
        };
        let Some(log) = store.log() else { return };
        if store.is_empty() {
            return;
        }
        let level = lod_level(store.lod(), self.time_scale.bar_spacing());
        let Some(view) = store.level_view(level) else {
            return;
        };
        let shift = if e.kind == SeriesKind::Volume {
            0
        } else {
            e.opts.shift
        };
        let (span, xt) = self.draw_range(&view, plot, shift);
        let yt = if volume_like(e) {
            if has_price {
                pane.volume_scale.transform(top, height)
            } else {
                pane.price_scale.transform(top, height)
            }
        } else {
            // A relative scale without a base for this series: nothing to draw.
            let Some(norm) = e.norm else { return };
            pane.price_scale.series_transform(top, height, norm)
        };
        let generation = log.generation();
        let (kind, style) = (e.kind, self.style_of(e));
        let flagged = is_ohlc_kind(kind) && !e.bar_flags.is_empty();
        let style_ref = self.frame.push_style(style);
        self.frame.push_cmd(
            LayerId::Main,
            DrawCmd::Series(SeriesDraw {
                series: data_id,
                generation,
                style: style_ref,
                kind,
                lod_level: level,
                index_range: span,
                x_transform: xt,
                y_transform: yt,
                clip: plot,
                pair: None,
            }),
        );
        if flagged {
            self.emit_bar_flags(k, level, span, &xt, &yt, &style);
        }
    }

    /// The bar quality flags of OHLC series `k` over its `SeriesDraw` (r22): a `FILLED`
    /// bar's pixels are cleared to the background and it is
    /// drawn again hollow in `filled_bar_color`; a `CLIPPED` bar gets a
    /// 3 CSS px tick at each end of its wick. Geometry from the same
    /// `transform_helpers` (and chunk references) as the backends, so the
    /// new pixels cover exactly the old ones. Flags and visible bars are
    /// walked in time order; nothing is allocated.
    fn emit_bar_flags(
        &mut self,
        k: usize,
        level: u8,
        span: IdxSpan,
        xt: &XTransform,
        yt: &YTransform,
        style: &SeriesStyle,
    ) {
        use crate::frame::RectInst;
        use crate::transform_helpers::{
            bar_metrics, candle, chunk_of, initial_chunk_ref, is_gap, is_up_rel, ohlc_bar,
            ohlc_rel, x_center, y, y_bias,
        };
        let Some(e) = self.series.get(k) else {
            return;
        };
        let Some(view) = e.store.level_view(level) else {
            return;
        };
        let lvl = u32::from(level);
        let (first, end) = (self.time_index.first_index(), self.time_index.end_index());
        // Logical indices of the bars in `span` (buckets above level 0).
        let (ia, ib) = if lvl == 0 {
            (span.start, span.end)
        } else {
            (bucket_start(span.start, lvl), bucket_start(span.end, lvl))
        };
        let (ia, ib) = (ia.max(first), ib.min(end));
        let (Some(t_a), Some(t_b)) = (self.time_index.time_at(ia), self.time_index.time_at(ib - 1))
        else {
            return;
        };
        let m = bar_metrics(xt.step);
        let bg = self.theme.background;
        let filled_color = e
            .opts
            .filled_bar_color
            .unwrap_or_else(|| style.up.with_alpha(0x66));
        let tick = {
            let w = (3.0 * self.dpr).round().max(1.0);
            if w % 2.0 == 0.0 { w + 1.0 } else { w }
        };
        let candles = e.kind != SeriesKind::OhlcBar;
        let frame = &mut self.frame;
        let index = &self.time_index;
        let mut chunk = (i64::MIN, None::<f64>);
        // Draws element `el` with `filled` of its bars filled, and whether
        // any is clipped.
        let mut draw = |el: i64, filled: i64, clipped: bool| {
            let bars = if lvl == 0 {
                1
            } else {
                bucket_start(el + 1, lvl).min(end) - bucket_start(el, lvl).max(first)
            };
            let filled = filled > 0 && filled >= bars;
            if !(filled || clipped) {
                return;
            }
            let bar = (|| {
                Some([
                    view.open.at(view.first, el)?,
                    view.high.at(view.first, el)?,
                    view.low.at(view.first, el)?,
                    view.close.at(view.first, el)?,
                ])
            })();
            let c = chunk_of(level, el);
            if chunk.0 != c {
                chunk = (c, initial_chunk_ref(&view, c));
            }
            let (Some(ohlc), Some(r)) = (bar, chunk.1) else {
                return;
            };
            let rel = ohlc_rel(yt, ohlc, r);
            if rel.iter().any(|&v| is_gap(v)) {
                return;
            }
            let bias = y_bias(yt, r);
            let xc = x_center(xt, el);
            let ys = rel.map(|v| y(yt, v, bias));
            let up = is_up_rel(rel[0], rel[3]);
            let mut put = |rect: RectPx, color: Rgba| {
                frame.push_rect(LayerId::Main, RectInst { rect, color });
            };
            let (wick, wick_color) = if candles {
                let g = candle(xc, m, ys);
                if filled {
                    put(g.wick, bg);
                    if let Some(b) = g.body {
                        put(b, bg);
                    }
                    put(g.wick, filled_color);
                    if let Some(b) = g.body {
                        if b.w >= 3.0 && b.h >= 3.0 {
                            // Hollow: the four edges.
                            let inner = b.h - 2.0;
                            put(RectPx { h: 1.0, ..b }, filled_color);
                            put(
                                RectPx {
                                    y: b.y + b.h - 1.0,
                                    h: 1.0,
                                    ..b
                                },
                                filled_color,
                            );
                            put(
                                RectPx {
                                    y: b.y + 1.0,
                                    w: 1.0,
                                    h: inner,
                                    ..b
                                },
                                filled_color,
                            );
                            let right = RectPx {
                                x: b.x + b.w - 1.0,
                                y: b.y + 1.0,
                                w: 1.0,
                                h: inner,
                            };
                            put(right, filled_color);
                        } else {
                            put(b, filled_color);
                        }
                    }
                }
                (g.wick, if up { style.up_wick } else { style.down_wick })
            } else {
                let parts = ohlc_bar(xc, m, ys);
                if filled {
                    for r in parts.into_iter().flatten() {
                        put(r, bg);
                    }
                    for r in parts.into_iter().flatten() {
                        put(r, filled_color);
                    }
                }
                let Some(w) = parts[0] else { return };
                (w, if up { style.up } else { style.down })
            };
            if clipped {
                let color = if filled { filled_color } else { wick_color };
                let x = wick.x - (tick - 1.0) * 0.5;
                for ty in [wick.y, wick.y + wick.h - 1.0] {
                    put(
                        RectPx {
                            x,
                            y: ty,
                            w: tick,
                            h: 1.0,
                        },
                        color,
                    );
                }
            }
        };
        let flags = &e.bar_flags;
        let start = flags.partition_point(|f| f.0 < t_a);
        // The element being collected: its filled bars, any clipped.
        let (mut cur, mut filled, mut clipped) = (i64::MIN, 0_i64, false);
        for &(t, f) in &flags[start..] {
            if t > t_b {
                break;
            }
            let j = index.lower_bound(t);
            if index.time_at(j) != Some(t) {
                continue; // not a bar
            }
            let el = if lvl == 0 { j } else { bucket_of(j, lvl) };
            if el != cur {
                if cur != i64::MIN {
                    draw(cur, filled, clipped);
                }
                (cur, filled, clipped) = (el, 0, false);
            }
            filled += i64::from(f.contains(BarFlags::FILLED));
            clipped |= f.contains(BarFlags::CLIPPED);
        }
        if cur != i64::MIN {
            draw(cur, filled, clipped);
        }
    }

    /// The `SeriesDraw` of band `b` if it is in pane `i`. Not drawn when the
    /// two series are mapped differently (different shifts, or relative
    /// price scales with different bases).
    fn emit_band(&mut self, i: usize, b: usize) {
        let (Some(pane), Some(band)) = (self.panes.get(i), self.bands.get(b)) else {
            return;
        };
        let find = |id: SeriesId| self.series.iter().find(|e| e.id == id);
        let (Some(u), Some(l)) = (find(band.upper), find(band.lower)) else {
            return;
        };
        if u.pane != pane.id || u.store.is_empty() || l.store.is_empty() {
            return;
        }
        let (Some(norm), Some(lower_norm)) = (u.norm, l.norm) else {
            return;
        };
        if norm != lower_norm || u.opts.shift != l.opts.shift {
            return;
        }
        let (Some(ul), Some(ll)) = (u.store.log(), l.store.log()) else {
            return;
        };
        let level = lod_level(u.store.lod(), self.time_scale.bar_spacing());
        let (Some(view), Some(_)) = (u.store.level_view(level), l.store.level_view(level)) else {
            return;
        };
        let plot = pane.plot;
        let (span, xt) = self.draw_range(&view, plot, u.opts.shift);
        let yt = pane
            .price_scale
            .series_transform(f64::from(plot.y), f64::from(plot.h), norm);
        let generation = ul
            .generation()
            .wrapping_add(ll.generation().rotate_left(32));
        let fill = band
            .opts
            .fill
            .unwrap_or_else(|| self.style_of(u).up.with_alpha(0x26));
        let style = SeriesStyle {
            fill_top: fill,
            fill_bottom: band.opts.fill_crossed.unwrap_or(fill),
            ..SeriesStyle::default()
        };
        let (upper, lower) = (band.upper, band.lower);
        let style = self.frame.push_style(style);
        self.frame.push_cmd(
            LayerId::Main,
            DrawCmd::Series(SeriesDraw {
                series: upper,
                generation,
                style,
                kind: SeriesKind::Band,
                lod_level: level,
                index_range: span,
                x_transform: xt,
                y_transform: yt,
                clip: plot,
                pair: Some(lower),
            }),
        );
    }

    /// The primitives of custom series `k` in pane `i`.
    fn emit_custom(&mut self, i: usize, k: usize, text: &dyn TextMeasurer) {
        let (Some(pane), Some(e)) = (self.panes.get(i), self.series.get(k)) else {
            return;
        };
        let (Some(c), Some(norm)) = (e.store.custom(), e.norm) else {
            return;
        };
        let shift = f64::from(e.opts.shift);
        let (from, to) = self.time_scale.visible_range();
        let visible = c.points_in(&self.time_index, from - shift, to - shift);
        if visible.0 >= visible.1 {
            return;
        }
        let ctx = SeriesCtx {
            coords: DrawingCtx {
                index: &self.time_index,
                time_scale: &self.time_scale,
                price_scale: &pane.price_scale,
                norm,
                plot: pane.plot,
                dpr: self.dpr,
                labels: pane.labels,
                text,
            },
            store: c,
            shift,
            visible,
            style: self.style_of(e),
        };
        if !self.native_kinds.contains(&c.kind) {
            c.plugin.build(
                &ctx,
                visible.0,
                visible.1,
                &mut FrameBuilder {
                    frame: &mut self.frame,
                    layer: LayerId::Main,
                },
            );
            return;
        }
        let mut params = [0.0; 8];
        c.plugin.native_params(&ctx, &mut params);
        let plot = pane.plot;
        let base = from.floor() as i64;
        let x_transform = XTransform {
            base,
            step: self.time_scale.bar_spacing() as f32,
            offset: (f64::from(plot.x) + self.time_scale.index_to_x_f64(base as f64 + shift))
                as f32,
        };
        let y_transform =
            pane.price_scale
                .series_transform(f64::from(plot.y), f64::from(plot.h), norm);
        let (series, generation, kind) = (e.id, c.log.generation(), c.kind);
        let style = self.frame.push_style(ctx.style);
        let points = IdxSpan {
            start: i64::try_from(visible.0).unwrap_or(i64::MAX),
            end: i64::try_from(visible.1).unwrap_or(i64::MAX),
        };
        self.frame.push_cmd(
            LayerId::Main,
            DrawCmd::Custom(CustomDraw {
                series,
                generation,
                kind,
                points,
                x_transform,
                y_transform,
                style,
                params,
                clip: plot,
            }),
        );
    }

    /// Kagi lines or Point & Figure marks of transform series `k` in pane
    /// `i`, as primitives. Below 1 px per element, one min–max line
    /// per pixel column (from the LOD levels), so the primitives never
    /// outnumber the plot's pixel columns.
    fn emit_figure(&mut self, i: usize, k: usize) {
        let (Some(pane), Some(e)) = (self.panes.get(i), self.series.get(k)) else {
            return;
        };
        let (Some(t), Some(norm)) = (e.store.transform(), e.norm) else {
            return;
        };
        let style = self.style_of(e);
        let out = &t.out;
        if out.len() == 0 {
            return;
        }
        let plot = pane.plot;
        let (top, height) = (f64::from(plot.y), f64::from(plot.h));
        let scale = &pane.price_scale;
        let ts = &self.time_scale;
        let y = |p: f64| -> Option<f32> {
            let y = scale.price_to_y(norm.apply(p), top, height);
            y.is_finite().then(|| y.clamp(-1e7, 1e7) as f32)
        };
        let shift = f64::from(e.opts.shift);
        let x = |j: f64| (f64::from(plot.x) + ts.index_to_x_f64(j + shift)) as f32;
        let spacing = ts.bar_spacing();
        let (from, to) = ts.visible_range();
        let mut b = FrameBuilder {
            frame: &mut self.frame,
            layer: LayerId::Main,
        };
        let thin = Px((style.line_width.0 / 2.0).max(1.0));
        let stroke = |flag: u8| StrokeStyle {
            width: if flag == 1 { style.line_width } else { thin },
            color: if flag == 1 { style.up } else { style.down },
            dash: LineDash::Solid,
        };
        if spacing < 1.0 {
            let level = lod_level(Some(out.lod()), spacing);
            let Some(view) = out.level_view(level) else {
                return;
            };
            let span = visible_span(&view, from - shift, to - shift);
            let lvl = u32::from(level);
            let size = (1_i64 << lvl) as f64;
            let one = StrokeStyle {
                width: Px(1.0),
                color: style.up,
                dash: LineDash::Solid,
            };
            // The current pixel column and its range of y.
            let mut col: Option<(f32, f32, f32)> = None;
            let flush = |b: &mut FrameBuilder<'_>, c: (f32, f32, f32)| {
                let x = c.0 + 0.5;
                b.line(
                    PointPx { x, y: c.1 },
                    PointPx {
                        x,
                        y: c.2.max(c.1 + 1.0),
                    },
                    one,
                );
            };
            for j in span.start..span.end {
                let center = if lvl == 0 {
                    j as f64
                } else {
                    bucket_start(j, lvl) as f64 + (size - 1.0) * 0.5
                };
                let (Some(hi), Some(lo)) =
                    (view.high.at(view.first, j), view.low.at(view.first, j))
                else {
                    continue;
                };
                let (Some(y0), Some(y1)) = (y(hi), y(lo)) else {
                    continue;
                };
                let px = x(center).floor();
                col = match col {
                    // Both are whole pixels.
                    Some(c) if (c.0 - px).abs() < 0.5 => Some((px, c.1.min(y0), c.2.max(y1))),
                    Some(c) => {
                        flush(&mut b, c);
                        Some((px, y0, y1))
                    }
                    None => Some((px, y0, y1)),
                };
            }
            if let Some(c) = col {
                flush(&mut b, c);
            }
            return;
        }
        let first = out.first();
        let a = ((from - shift).floor() as i64 - 1).max(first);
        let end = ((to - shift).ceil() as i64 + 2).min(out.end());
        let flag_at = |j: i64| {
            usize::try_from(j - first)
                .ok()
                .and_then(|p| t.flags.get(p).copied())
                .unwrap_or(0)
        };
        if e.kind == SeriesKind::Kagi {
            for j in a..end {
                let Some([open, _, _, close]) = out.bar_at(j) else {
                    continue;
                };
                let (Some(yo), Some(yc)) = (y(open), y(close)) else {
                    continue;
                };
                let xj = x(j as f64);
                let flag = flag_at(j);
                let prev = out.bar_at(j - 1);
                let start = if prev.is_some() { flag_at(j - 1) } else { flag };
                if prev.is_some() {
                    // The shoulder or waist joining the previous line.
                    let from = PointPx {
                        x: x((j - 1) as f64),
                        y: yo,
                    };
                    b.line(from, PointPx { x: xj, y: yo }, stroke(start));
                }
                match prev.and_then(|p| y(p[0])) {
                    // The thickness changes at the previous line's start.
                    Some(ys) if start != flag => {
                        b.line(
                            PointPx { x: xj, y: yo },
                            PointPx { x: xj, y: ys },
                            stroke(start),
                        );
                        b.line(
                            PointPx { x: xj, y: ys },
                            PointPx { x: xj, y: yc },
                            stroke(flag),
                        );
                    }
                    _ => b.line(
                        PointPx { x: xj, y: yo },
                        PointPx { x: xj, y: yc },
                        stroke(flag),
                    ),
                }
            }
            return;
        }
        // Point & Figure.
        let Some(size) = t.plugin.box_size().filter(|s| s.is_finite() && *s > 0.0) else {
            return;
        };
        let w = (spacing * 0.8) as f32;
        let mark = Px((style.line_width.0 * 0.75).max(1.0));
        for j in a..end {
            let Some([_, high, low, _]) = out.bar_at(j) else {
                continue;
            };
            let (Some(y_hi), Some(y_lo)) = (y(high), y(low)) else {
                continue;
            };
            let flag = flag_at(j);
            let color = if flag == 1 { style.up } else { style.down };
            let xc = x(j as f64);
            let boxes = ((high - low) / size).round();
            let box_px = (y_lo - y_hi).abs() / boxes.max(1.0) as f32;
            if spacing < 4.0 || box_px < 4.0 || boxes > 10_000.0 {
                let r = RectPx {
                    x: xc - w / 2.0,
                    y: y_hi.min(y_lo),
                    w: w.max(1.0),
                    h: (y_lo - y_hi).abs().max(1.0),
                };
                b.rect(r, color);
                continue;
            }
            let pen = StrokeStyle {
                width: mark,
                color,
                dash: LineDash::Solid,
            };
            let (l, r) = (xc - w / 2.0, xc + w / 2.0);
            for m in 0..boxes as i64 {
                let (Some(y0), Some(y1)) =
                    (y(low + m as f64 * size), y(low + (m + 1) as f64 * size))
                else {
                    continue;
                };
                // Inset so neighbouring boxes do not touch.
                let inset = (y0 - y1).abs() * 0.1;
                let (yb, yt) = (y0.max(y1) - inset, y0.min(y1) + inset);
                if flag == 1 {
                    b.line(PointPx { x: l, y: yt }, PointPx { x: r, y: yb }, pen);
                    b.line(PointPx { x: l, y: yb }, PointPx { x: r, y: yt }, pen);
                } else {
                    let (cy, rx, ry) = (f32::midpoint(yb, yt), w / 2.0, (yb - yt) / 2.0);
                    b.polyline(
                        (0..=CIRCLE_SEGMENTS).map(|s| {
                            let a =
                                f32::from(s) * core::f32::consts::TAU / f32::from(CIRCLE_SEGMENTS);
                            PointPx {
                                x: xc + rx * a.cos(),
                                y: cy + ry * a.sin(),
                            }
                        }),
                        pen,
                    );
                }
            }
        }
    }

    /// Declares the custom series kinds the renderer draws natively
    /// ([`Renderer::native_kinds`](crate::render::Renderer::native_kinds));
    /// for them the frame holds a [`DrawCmd::Custom`] instead of the plugin's
    /// primitives. Host helpers call it before every frame; it does
    /// nothing (and does not allocate) when the kinds are unchanged.
    pub fn set_native_kinds(&mut self, kinds: &[&'static str]) {
        if self.native_kinds.as_slice() == kinds {
            return;
        }
        self.native_kinds.clear();
        self.native_kinds.extend_from_slice(kinds);
        self.invalidation = Invalidation::Full;
    }

    fn push_label(&mut self, pos: PointPx, align: TextAlign, font: FontStyleRef, span: Span) {
        self.frame.push_text(
            LayerId::Main,
            TextRun {
                pos,
                text: span,
                style: font,
                color: self.theme.text,
                align,
                bg: None,
            },
        );
    }

    /// Price labels of pane `i` on the right axis.
    fn emit_price_labels(&mut self, i: usize, line_h: f32, pad: f32, font: FontStyleRef) {
        let Some(pane) = self.panes.get(i) else {
            return;
        };
        let (plot, labels) = (pane.plot, pane.labels.for_ticks(&pane.ticks));
        for k in 0..self.panes[i].ticks.len() {
            let pane = &self.panes[i];
            let p = pane.ticks[k];
            let y = pane
                .price_scale
                .price_to_y(p, f64::from(plot.y), f64::from(plot.h)) as f32;
            if y < plot.y + line_h * 0.5 || y > plot.bottom() - line_h * 0.5 {
                continue;
            }
            // Value labels have priority over ticks.
            let label_h = line_h + pad;
            if self.axis_labels.iter().any(|l| (l.at - y).abs() < label_h) {
                continue;
            }
            let start = self.frame.pools.text.len();
            write_label(&mut self.frame.pools.text, p, labels);
            let span = Span {
                start: u32::try_from(start).unwrap_or(0),
                len: u32::try_from(self.frame.pools.text.len() - start).unwrap_or(0),
            };
            self.push_label(
                PointPx {
                    x: plot.right() + pad,
                    y: y.round(),
                },
                TextAlign::Left,
                font,
                span,
            );
        }
        // Value labels: a background over the whole axis width.
        let label_h = (line_h + pad).round();
        let axis_right = self.frame.size_px.w;
        for n in 0..self.axis_labels.len() {
            let l = self.axis_labels[n];
            let y = l.at.round();
            self.frame.push_rect(
                LayerId::Main,
                crate::frame::RectInst {
                    rect: RectPx {
                        x: plot.right() + 1.0,
                        y: (y - label_h * 0.5).round(),
                        w: (axis_right - plot.right() - 1.0).max(0.0),
                        h: label_h,
                    },
                    color: l.bg,
                },
            );
            self.frame.push_text(
                LayerId::Main,
                TextRun {
                    pos: PointPx {
                        x: plot.right() + pad,
                        y,
                    },
                    text: l.text,
                    style: font,
                    color: l.fg,
                    align: TextAlign::Left,
                    bg: None,
                },
            );
        }
    }

    /// The last value of series `k` (the close of its last bar that is not a
    /// gap) and its color.
    fn last_value(&self, k: usize) -> Option<(f64, Rgba)> {
        let e = self.series.get(k)?;
        if let Some(c) = e.store.custom() {
            let v = c.plugin.last_value().filter(|v| v.is_finite())?;
            return Some((v, self.style_of(e).up));
        }
        let (first, end) = e.store.span()?;
        let [open, _, _, close] = (first..end).rev().find_map(|j| e.store.bar_at(j))?;
        let style = self.style_of(e);
        let up = match e.kind {
            SeriesKind::Baseline => close >= e.opts.base_value,
            k if is_ohlc_kind(k) || matches!(k, SeriesKind::Kagi | SeriesKind::PointFigure) => {
                close >= open
            }
            _ => true,
        };
        Some((close, if up { style.up } else { style.down }))
    }

    /// Markers of the series in pane `i` (main layer, clipped to the plot). Markers on
    /// the same bar and side stack away from the bar; with
    /// `marker_stack_limit`, a side with more markers ends in a "+k" badge;
    /// with `marker_cluster`, a side of a bucket of bars with markers on two
    /// or more of them shows its first marker and "+k" (r22).
    fn emit_markers(&mut self, i: usize, line_h: f32, font: FontStyleRef, text: &dyn TextMeasurer) {
        let Some(pane) = self.panes.get(i) else {
            return;
        };
        let (id, plot) = (pane.id, pane.plot);
        let (top, height) = (f64::from(plot.y), f64::from(plot.h));
        if self.time_index.is_empty()
            || !self
                .series
                .iter()
                .any(|e| e.pane == id && !e.markers.is_empty())
        {
            return;
        }
        let (first, end) = (self.time_index.first_index(), self.time_index.end_index());
        let (a, b) = self.time_scale.visible_range();
        let ja = (a.floor() as i64).clamp(first, end - 1);
        let jb = (b.ceil() as i64).clamp(first, end - 1);
        let dpr = self.dpr;
        let spacing = self.time_scale.bar_spacing();
        let gap = (MARKER_GAP * dpr).round();
        let clip = self.data_clip(plot);
        self.frame.push_cmd(LayerId::Main, DrawCmd::PushClip(clip));
        for k in 0..self.series.len() {
            let e = &self.series[k];
            let Some(norm) = e.norm.filter(|_| e.pane == id && !e.markers.is_empty()) else {
                continue;
            };
            let (series, main, limit, shift) = (
                e.id,
                self.style_of(e).up,
                e.opts.marker_stack_limit,
                e.opts.shift,
            );
            // Bars per cluster bucket; 1 groups by bar (no clustering).
            let kb = e.opts.marker_cluster.map_or(1, |d| {
                let bars = (f64::from(d.0 * dpr) / spacing.max(f64::MIN_POSITIVE)).ceil();
                bars.clamp(1.0, 1e12) as i64
            });
            // The visible bars widened to whole buckets, so a bucket at an
            // edge counts all of its markers while panning.
            let ja = (ja.div_euclid(kb) * kb).max(first);
            let jb = ((jb.div_euclid(kb) + 1) * kb - 1).min(end - 1);
            let Some(t_hi) = self.time_index.time_at(jb) else {
                continue;
            };
            // Markers snap to the next bar: those before `t_skip` land left
            // of the first visible bar.
            let t_skip = self.time_index.time_at(ja - 1).unwrap_or(i64::MIN);
            let start = e.markers.partition_point(|m| m.time <= t_skip);
            let bar_of = |t: Timestamp| self.time_index.lower_bound(t).min(end - 1);
            // Groups need counting to cluster or fold.
            let count = kb > 1 || limit.is_some();
            // The group (bucket of bars) being drawn on each side.
            let mut group = [MarkerGroup::EMPTY; 2];
            for n in start..self.series[k].markers.len() {
                let e = &self.series[k];
                let m = &e.markers[n];
                if m.time > t_hi && jb < end - 1 {
                    break;
                }
                let j = bar_of(m.time);
                let Some(bar_j) = e.store.bar_at(j) else {
                    continue; // on a gap
                };
                let side = stack_side(m.position);
                // The bucket (a bar without clustering: no division).
                let key = if kb == 1 { j } else { j.div_euclid(kb) };
                if let Some(s) = side
                    && group[s].key != key
                {
                    group[s] = if count {
                        let ix = MarkerBars {
                            index: &self.time_index,
                            store: &e.store,
                            end,
                        };
                        MarkerGroup::scan(&e.markers[n..], s, kb, j, &ix)
                    } else {
                        MarkerGroup::single(j)
                    };
                }
                let size_css = m.size.map_or(MARKER_SIZE, |s| s.0);
                let size = (size_css * dpr).round().max(4.0);
                // A cluster is drawn on its bar with the most markers.
                let at = side.map_or(j, |s| group[s].at);
                let shown = at + i64::from(shift);
                let x = (plot.x + self.time_scale.index_to_x(shown as f64).0).round();
                let has_text = !m.text.is_empty();
                let mut bars = IdxSpan {
                    start: j,
                    end: j + 1,
                };
                // Center y, and where the text goes (y, x, alignment).
                let mut overflow = 0;
                let (cy, ty, tx, align) = if let Some(side) = side {
                    let g = &mut group[side];
                    if g.clustered {
                        // The first marker, then "+k" for the rest.
                        match g.drawn {
                            0 => {}
                            1 => (overflow, bars) = (g.total - 1, g.bars),
                            _ => continue,
                        }
                    } else if let Some(lim) = limit.map(u32::from)
                        && g.total > lim
                    {
                        // Folding: the place of marker `lim` shows "+k" for the rest.
                        if g.drawn >= lim {
                            continue;
                        }
                        if g.drawn == lim - 1 {
                            overflow = g.total - g.drawn;
                        }
                    }
                    g.drawn += 1;
                    let bar = if at == j {
                        Some(bar_j)
                    } else {
                        e.store.bar_at(at)
                    };
                    let Some([_, high, low, _]) = bar else {
                        continue;
                    };
                    let price = if side == 0 { high } else { low };
                    let y_bar = self.panes[i]
                        .price_scale
                        .price_to_y(norm.apply(price), top, height)
                        as f32;
                    let text_h = if has_text && overflow == 0 {
                        line_h
                    } else {
                        0.0
                    };
                    let off = &mut group[side].off;
                    let (cy, ty) = if side == 0 {
                        let cy = y_bar - gap - *off - size * 0.5;
                        *off += gap + size + text_h;
                        (cy, cy - size * 0.5 - text_h * 0.5)
                    } else {
                        let cy = y_bar + gap + *off + size * 0.5;
                        *off += gap + size + text_h;
                        (cy, cy + size * 0.5 + text_h * 0.5)
                    };
                    (cy, ty, x, TextAlign::Center)
                } else {
                    // `AtPrice` (r22): on its price, text on the right.
                    let p = m.price.unwrap_or(f64::NAN);
                    let cy = self.panes[i]
                        .price_scale
                        .price_to_y(norm.apply(p), top, height) as f32;
                    if !cy.is_finite() {
                        continue; // e.g. a price <= 0 on a log scale
                    }
                    (cy, cy, x + (size * 0.5 + gap).round(), TextAlign::Left)
                };
                let pos = PointPx { x, y: cy.round() };
                let (target, inst, color) = if overflow > 0 {
                    let target = ClickTarget::MarkerOverflow {
                        series,
                        first: n,
                        count: overflow,
                        id: m.id,
                        bars,
                    };
                    let inst = crate::frame::IconInst {
                        pos,
                        size: Px(size),
                        icon: MarkerShape::Circle.icon(),
                        color: main,
                        ..Default::default()
                    };
                    (target, inst, main)
                } else {
                    let color = m.color.unwrap_or(main);
                    let border = m.border.unwrap_or_default();
                    let inst = crate::frame::IconInst {
                        pos,
                        size: Px(size),
                        icon: m.shape.icon(),
                        color,
                        image: m.icon,
                        border,
                        border_width: Px((m.border_width.0 * dpr).min(size * 0.5)),
                    };
                    let target = ClickTarget::Marker {
                        series,
                        index: n,
                        id: m.id,
                    };
                    (target, inst, color)
                };
                self.marker_hits.push(MarkerSpot {
                    target,
                    bars,
                    pane: id,
                    pos,
                    radius: size * 0.5,
                });
                self.frame.push_icon(LayerId::Main, inst);
                // The label inside the shape: the marker's, or "+k".
                let label = if overflow > 0 {
                    let start = self.frame.pools.text.len();
                    let _ = core::fmt::Write::write_fmt(
                        &mut self.frame.pools.text,
                        format_args!("+{overflow}"),
                    );
                    Some(Span {
                        start: start as u32,
                        len: (self.frame.pools.text.len() - start) as u32,
                    })
                } else if !self.series[k].markers[n].label.is_empty() {
                    Some(self.frame.push_str(&self.series[k].markers[n].label))
                } else {
                    None
                };
                if let Some(span) = label {
                    let s = self.frame.pools.text_of(span);
                    // Full-width characters (CJK) are unreadable at the
                    // Latin size: they start larger and still shrink to fit.
                    let ratio = if s.is_ascii() { 0.45 } else { 0.6 };
                    let mut fs = (size_css * ratio).round().max(MARKER_LABEL_MIN_FONT);
                    let style = |fs: f32| FontStyleRef {
                        font: font.font,
                        size: fs * dpr,
                    };
                    while fs > MARKER_LABEL_MIN_FONT
                        && crate::text::text_width(text, s, style(fs)) > size * 0.8
                    {
                        fs -= 1.0;
                    }
                    self.frame.push_text(
                        LayerId::Main,
                        TextRun {
                            pos,
                            text: span,
                            style: style(fs),
                            color: contrast(inst.color),
                            align: TextAlign::Center,
                            bg: None,
                        },
                    );
                }
                if has_text && overflow == 0 {
                    let span = self.frame.push_str(&self.series[k].markers[n].text);
                    self.frame.push_text(
                        LayerId::Main,
                        TextRun {
                            pos: PointPx {
                                x: tx,
                                y: ty.round(),
                            },
                            text: span,
                            style: font,
                            color,
                            align,
                            bg: None,
                        },
                    );
                }
            }
        }
        self.frame.push_cmd(LayerId::Main, DrawCmd::PopClip);
    }

    /// Last-value lines and price lines of pane `i` (main layer, above the
    /// series), and the placement of their axis labels in `axis_labels`:
    /// labels are placed in priority order (series in the order
    /// they were added, then price lines), each at the free position nearest
    /// to its price.
    fn emit_marks(&mut self, i: usize, line_h: f32, pad: f32, font: FontStyleRef) {
        self.axis_labels.clear();
        if self.revealing() {
            return; // after the reveal
        }
        let Some(pane) = self.panes.get(i) else {
            return;
        };
        let (id, plot, labels) = (pane.id, pane.plot, pane.labels);
        let (top, height) = (f64::from(plot.y), f64::from(plot.h));
        let inside = |y: f32| y >= plot.y && y < plot.bottom();
        let format = |e: &SeriesEntry| match labels {
            NumberFormat::Fixed(_) | NumberFormat::Auto(_) => match e.opts.price_precision {
                Some(p) => NumberFormat::Fixed(p),
                None => NumberFormat::Auto(e.auto_decimals),
            },
            other => other,
        };
        for k in 0..self.series.len() {
            let e = &self.series[k];
            if e.pane != id || volume_like(e) {
                continue;
            }
            let (Some(norm), Some((value, color))) = (e.norm, self.last_value(k)) else {
                continue;
            };
            let (show_line, show_label, fmt) = (
                e.opts.price_line_visible,
                e.opts.last_value_visible,
                format(e),
            );
            let d = norm.apply(value);
            let y = self.panes[i].price_scale.price_to_y(d, top, height) as f32;
            if !inside(y) {
                continue;
            }
            if show_line {
                let yl = y.floor() + 0.5;
                self.frame.push_line(
                    LayerId::Main,
                    LineInst {
                        from: PointPx { x: plot.x, y: yl },
                        to: PointPx {
                            x: plot.right(),
                            y: yl,
                        },
                        stroke: StrokeStyle {
                            width: Px(1.0),
                            color,
                            dash: LineDash::Dotted,
                        },
                    },
                );
            }
            if show_label {
                self.push_axis_label(y, d, fmt, color);
            }
        }
        for n in 0..self.price_lines.len() {
            let line = &self.price_lines[n];
            let Some(k) = self.series.iter().position(|e| e.id == line.series) else {
                continue;
            };
            let e = &self.series[k];
            let Some(norm) = e.norm.filter(|_| e.pane == id) else {
                continue;
            };
            let color = line.opts.color.unwrap_or_else(|| self.style_of(e).up);
            let fmt = format(e);
            let d = norm.apply(line.opts.price);
            let y = self.panes[i].price_scale.price_to_y(d, top, height) as f32;
            if !inside(y) {
                continue;
            }
            let width = line.opts.line_width.to_px(self.dpr).0.round().max(1.0);
            let yl = if width as i32 % 2 == 1 {
                y.floor() + 0.5
            } else {
                y.round()
            };
            self.frame.push_line(
                LayerId::Main,
                LineInst {
                    from: PointPx { x: plot.x, y: yl },
                    to: PointPx {
                        x: plot.right(),
                        y: yl,
                    },
                    stroke: StrokeStyle {
                        width: Px(width),
                        color,
                        dash: line.opts.line_style,
                    },
                },
            );
            let show_label = line.opts.axis_label_visible;
            if !line.opts.title.is_empty() {
                let span = self.frame.push_str(&self.price_lines[n].opts.title);
                self.frame.push_text(
                    LayerId::Main,
                    TextRun {
                        pos: PointPx {
                            x: plot.right() - pad,
                            y: (y - line_h * 0.5 - 2.0).round(),
                        },
                        text: span,
                        style: font,
                        color,
                        align: TextAlign::Right,
                        bg: None,
                    },
                );
            }
            if show_label {
                self.push_axis_label(y, d, fmt, color);
            }
        }
        // Place in priority order: the candidate positions are the wanted
        // one and the slots right above and below each placed label.
        let h = line_h + pad;
        let (lo, hi) = (plot.y + h * 0.5, plot.bottom() - h * 0.5);
        for n in 0..self.axis_labels.len() {
            let want = self.axis_labels[n].want.clamp(lo, hi.max(lo));
            let placed = &self.axis_labels[..n];
            let free = |c: f32| {
                c >= lo - 0.01
                    && c <= hi + 0.01
                    && placed.iter().all(|p| (p.at - c).abs() >= h - 0.01)
            };
            let mut best = want;
            if !free(want) {
                let mut dist = f32::INFINITY;
                for p in placed {
                    for c in [p.at - h, p.at + h] {
                        if free(c) && (c - want).abs() < dist {
                            dist = (c - want).abs();
                            best = c;
                        }
                    }
                }
            }
            self.axis_labels[n].at = best;
        }
    }

    /// Writes a value label's text and queues it for placement.
    fn push_axis_label(&mut self, want: f32, value: f64, format: NumberFormat, bg: Rgba) {
        let start = self.frame.pools.text.len();
        write_label(&mut self.frame.pools.text, value, format);
        let text = Span {
            start: u32::try_from(start).unwrap_or(0),
            len: u32::try_from(self.frame.pools.text.len() - start).unwrap_or(0),
        };
        if self.axis_labels.len() < self.axis_labels.capacity() {
            self.axis_labels.push(AxisLabel {
                want,
                at: want,
                text,
                bg,
                fg: contrast(bg),
            });
        }
    }

    /// Axis lines, pane separators and time labels.
    fn emit_axes(&mut self, time_h: f32, font: FontStyleRef) {
        let axis = self.theme.axis_line;
        let (w, h) = (self.frame.size_px.w, self.frame.size_px.h);
        let layout = self.layout;
        let x = layout.price_axis.x + 0.5;
        let l = Self::line(
            PointPx { x, y: 0.0 },
            PointPx {
                x,
                y: layout.price_axis.bottom(),
            },
            axis,
        );
        self.frame.push_line(LayerId::Main, l);
        // Separators: the 1-px rows between panes, across the price axis too.
        for k in 1..self.panes.len() {
            let y = self.panes[k].plot.y - SEPARATOR_PX + 0.5;
            let l = Self::line(PointPx { x: 0.0, y }, PointPx { x: w, y }, axis);
            self.frame.push_line(LayerId::Main, l);
        }
        let y = layout.time_axis.y + 0.5;
        if y < h {
            let l = Self::line(PointPx { x: 0.0, y }, PointPx { x: w, y }, axis);
            self.frame.push_line(LayerId::Main, l);
        }

        let y = (layout.time_axis.y + time_h * 0.5).round();
        for k in 0..self.placed.len() {
            let placed = self.placed[k];
            let Some(label) = self.tick_label(placed.tick) else {
                continue;
            };
            let span = self.frame.push_str(label.as_str());
            self.push_label(
                PointPx {
                    x: layout.plot.x + placed.x.round(),
                    y,
                },
                TextAlign::Center,
                font,
                span,
            );
        }
    }
}

/// Read-only view of the chart data for renderers.
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::chart::{SeriesData, SeriesOptions};
/// use ferrochart_core::data::DataDelta;
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::testing::random_walk;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
/// chart.set_data(id, SeriesData::Ohlc(random_walk(1, 100, 0, 60_000)?))?;
///
/// let view = chart.data_view();
/// assert_eq!(view.changes_since(id, 0), DataDelta::Full); // never synced
/// let synced = view.generation(id).unwrap();
/// assert_eq!(view.changes_since(id, synced), DataDelta::None);
/// assert_eq!(view.ohlc(id, 0).unwrap().len(), 100);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
#[derive(Clone, Copy)]
pub struct ChartDataView<'a> {
    series: &'a [SeriesEntry],
    time_index: &'a TimeIndex,
    icons: crate::icon::IconsView<'a>,
}

/// The time index of [`ChartDataView::empty`].
static EMPTY_TIME_INDEX: TimeIndex = TimeIndex::new();

impl<'a> ChartDataView<'a> {
    /// The registered images (r21): renderers upload the tiles when
    /// [`IconsView::generation`](crate::icon::IconsView::generation) changes.
    pub fn icons(self) -> crate::icon::IconsView<'a> {
        self.icons
    }

    /// A view without series, for frames that only hold immediate
    /// primitives (`ferrochart-plot`).
    ///
    /// ```
    /// use ferrochart_core::chart::ChartDataView;
    /// use ferrochart_core::data::SeriesId;
    ///
    /// let view = ChartDataView::empty();
    /// assert_eq!(view.generation(SeriesId(1)), None);
    /// assert!(view.time_index().is_empty());
    /// ```
    pub fn empty() -> ChartDataView<'static> {
        ChartDataView {
            series: &[],
            time_index: &EMPTY_TIME_INDEX,
            icons: crate::icon::IconsView::empty(),
        }
    }

    /// The store holding `id`'s data (a volume series reads its source).
    fn store(self, id: SeriesId) -> Option<&'a Store> {
        let series = self.series;
        let e = series.iter().find(|e| e.id == id)?;
        match e.kind {
            SeriesKind::Volume => {
                let src = e.opts.source?;
                series.iter().find(|x| x.id == src).map(|x| &x.store)
            }
            _ => Some(&e.store),
        }
    }

    /// Current data generation of a series, `None` for unknown ids.
    pub fn generation(&self, id: SeriesId) -> Option<u64> {
        self.store(id)?.log().map(ChangeLog::generation)
    }

    /// What changed in a series since generation `since_gen`. Unknown ids
    /// give `Full`.
    pub fn changes_since(&self, id: SeriesId, since_gen: u64) -> DataDelta {
        self.store(id)
            .and_then(Store::log)
            .map_or(DataDelta::Full, |log| log.changes_since(since_gen))
    }

    /// Data of one level (0 = original, `>= 2` = LOD). Value series appear as
    /// bars with `open == high == low == close == value` at level 0 and
    /// `(first, max, min, last)` per bucket at LOD levels; NaN values are gaps.
    pub fn ohlc(&self, id: SeriesId, level: u8) -> Option<OhlcLevelView<'a>> {
        self.store(id)?.level_view(level)
    }

    /// Per-point colors of a value series (RGBA8, `0` = the series color) and
    /// the logical index of the first one. Level 0 only.
    pub fn point_colors(&self, id: SeriesId) -> Option<(i64, &'a [u32])> {
        self.store(id)?.colors()
    }

    /// The plugin of a custom series, for native renderers to read its data
    /// (downcast with `as &dyn Any`); `None` for other series.
    pub fn custom(&self, id: SeriesId) -> Option<&'a dyn CustomSeries> {
        let e = self.series.iter().find(|e| e.id == id)?;
        e.store.custom().map(|c| c.plugin.as_ref())
    }

    /// Fractional logical index of point `point` of custom series `id` (its
    /// slot, or its time interpolated between slots for `AxisMode::Overlay`),
    /// without `SeriesOptions::shift` (which is in [`CustomDraw::x_transform`]).
    pub fn custom_index(&self, id: SeriesId, point: usize) -> Option<f64> {
        let e = self.series.iter().find(|e| e.id == id)?;
        e.store.custom()?.x_index(self.time_index, point)
    }

    /// The global time index.
    pub fn time_index(&self) -> &'a TimeIndex {
        self.time_index
    }
}

/// LOD level for the current bar spacing.
fn lod_level(lod: Option<&crate::data::lod::Lod>, bar_spacing: f64) -> u8 {
    let bars_per_px = 1.0 / bar_spacing;
    if bars_per_px < 4.0 {
        return 0;
    }
    let Some(max) = lod.and_then(crate::data::lod::Lod::max_level) else {
        return 0;
    };
    let want = bars_per_px.log2().ceil().max(2.0).min(f64::from(max));
    want as u8
}

/// Elements of `view` covering the visible logical range `from..to`, one
/// extra on each side.
fn visible_span(view: &OhlcLevelView<'_>, from: f64, to: f64) -> crate::data::IdxSpan {
    let (i0, i1) = (from.floor() as i64 - 1, to.ceil() as i64 + 1);
    let k = u32::from(view.level);
    let (start, end) = if k == 0 {
        (i0, i1 + 1)
    } else {
        (bucket_of(i0, k), bucket_of(i1, k) + 1)
    };
    crate::data::IdxSpan {
        start: start.max(view.first),
        end: end.min(view.end()),
    }
}

/// Largest volume of `view` in `span`.
fn max_volume_in(view: &OhlcLevelView<'_>, span: crate::data::IdxSpan) -> f64 {
    let mut max_volume: f64 = 0.0;
    if let Some(col) = view.volume {
        for j in span.start..span.end {
            let p = usize::try_from(j - view.first).unwrap_or(usize::MAX);
            max_volume = max_volume.max(col.get(p).unwrap_or(0.0));
        }
    }
    max_volume
}

/// Writes an axis label in the pane's format.
fn write_label(out: &mut impl core::fmt::Write, v: f64, format: NumberFormat) {
    format.write(out, v);
}

/// Black or white text, whichever reads better on `bg`.
fn contrast(bg: Rgba) -> Rgba {
    let l = 0.299 * f32::from(bg.r) + 0.587 * f32::from(bg.g) + 0.114 * f32::from(bg.b);
    if l > 160.0 {
        Rgba::hex(0x0013_1722)
    } else {
        Rgba::hex(0x00FF_FFFF)
    }
}

/// The first value (close) at logical indices `from..to` that is not a gap:
/// the base of relative price scales.
fn first_value(store: &Store, from: i64, to: i64) -> Option<f64> {
    let (first, end) = store.span()?;
    (from.max(first)..to.min(end)).find_map(|j| store.bar_at(j).map(|b| b[3]))
}

impl Column<'_> {
    /// Value at logical index / bucket `j` of a view whose first element is `first`.
    pub fn at(&self, first: i64, j: i64) -> Option<f64> {
        self.get(usize::try_from(j - first).ok()?)
    }
}

/// Crosshair equality with prices compared bit for bit (a synced crosshair
/// has a NaN price; gap bars have NaN values).
fn same_crosshair(a: Option<&CrosshairPoint>, b: Option<&CrosshairPoint>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let bits = |v: Option<BarValues>| {
                v.map(|v| {
                    [v.open, v.high, v.low, v.close, v.volume.unwrap_or(f64::NAN)].map(f64::to_bits)
                })
            };
            a.logical_index == b.logical_index
                && a.time == b.time
                && a.pane == b.pane
                && a.price.to_bits() == b.price.to_bits()
                && bits(a.bar) == bits(b.bar)
                && a.bar.map(|v| v.volume.is_some()) == b.bar.map(|v| v.volume.is_some())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{OhlcColumns, ValueColumns};

    fn ohlc(times: &[i64], close: f64) -> SeriesData {
        let n = times.len();
        SeriesData::Ohlc(OhlcColumns {
            time: times.to_vec(),
            open: vec![close; n],
            high: vec![close + 1.0; n],
            low: vec![close - 1.0; n],
            close: vec![close; n],
            volume: Some(vec![10.0; n]),
        })
    }

    fn values(times: &[i64], v: &[f64]) -> SeriesData {
        SeriesData::Value(ValueColumns {
            time: times.to_vec(),
            value: v.to_vec(),
            color: None,
        })
    }

    fn two_series(a: SeriesKind, b: SeriesKind) -> (Chart, SeriesId, SeriesId) {
        let mut chart = Chart::new(ChartOptions::default());
        let pane = chart.main_pane();
        let x = chart.add_series(pane, a, SeriesOptions::default()).unwrap();
        let y = chart.add_series(pane, b, SeriesOptions::default()).unwrap();
        (chart, x, y)
    }

    /// Value of series `id` at time `t`, through the time axis.
    fn at(chart: &Chart, id: SeriesId, t: i64) -> Option<f64> {
        let j = chart.time_index.index_of(t)?;
        let e = chart.series.iter().find(|e| e.id == id)?;
        e.store.bar_at(j).map(|b| b[3])
    }

    fn merged_warnings(chart: &mut Chart) -> usize {
        chart
            .drain_events()
            .filter(|e| matches!(e, ChartEvent::Warning(WarningKind::TimeAxisMerged { .. })))
            .count()
    }

    /// The axis is the union of all series' times; a time between
    /// existing bars rebuilds it with a warning, data stays at its times.
    #[test]
    fn time_axis_is_the_union_of_all_series() {
        let (mut chart, candles, line) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart
            .set_data(candles, ohlc(&[10, 20, 30, 40], 5.0))
            .unwrap();
        chart.drain_events().for_each(drop);
        // Extending past the end keeps every logical index; no warning.
        chart
            .set_data(line, values(&[30, 40, 50, 60], &[1.0, 2.0, 3.0, 4.0]))
            .unwrap();
        assert_eq!(chart.time_index.times(), &[10, 20, 30, 40, 50, 60]);
        assert_eq!(chart.time_index.index_of(10), Some(0));
        assert_eq!(merged_warnings(&mut chart), 0);
        // A time between bars: rebuilt, with a warning.
        chart
            .set_data(line, values(&[15, 30, 60], &[1.0, 2.0, 4.0]))
            .unwrap();
        assert_eq!(chart.time_index.times(), &[10, 15, 20, 30, 40, 60]);
        assert_eq!(merged_warnings(&mut chart), 1);
        assert_eq!(chart.time_index.index_of(10), Some(0)); // the first shared slot keeps its index
        for t in [10, 20, 30, 40] {
            assert_eq!(at(&chart, candles, t), Some(5.0), "t = {t}");
        }
        assert_eq!(at(&chart, candles, 15), None); // a gap bar
        assert_eq!(at(&chart, line, 15), Some(1.0));
        assert_eq!(at(&chart, line, 20), Some(1.5)); // interpolated
        // Appending a candle between line points rebuilds again.
        chart.append(candles, ohlc(&[50], 6.0)).unwrap();
        assert_eq!(chart.time_index.times(), &[10, 15, 20, 30, 40, 50, 60]);
        assert_eq!(merged_warnings(&mut chart), 1);
        assert_eq!(at(&chart, candles, 50), Some(6.0));
        assert_eq!(at(&chart, line, 60), Some(4.0));
        // update_last past the end extends the axis without a rebuild.
        let bar = BarUpdate {
            time: 70,
            open: 7.0,
            high: 8.0,
            low: 6.0,
            close: 7.0,
            volume: Some(1.0),
        };
        assert_eq!(
            chart.update_last(candles, bar),
            Ok(DataChange::Append { count: 2 })
        );
        assert_eq!(at(&chart, candles, 60), None);
        assert_eq!(at(&chart, candles, 70), Some(7.0));
        assert_eq!(merged_warnings(&mut chart), 0);
    }

    /// Replacing every series (e.g. another symbol) leaves no stale bars.
    #[test]
    fn replaced_data_leaves_no_stale_bars() {
        let (mut chart, candles, line) = two_series(SeriesKind::Candlestick, SeriesKind::Area);
        chart.set_data(candles, ohlc(&[1, 2, 3], 5.0)).unwrap();
        chart.set_data(line, values(&[1, 2, 3], &[1.0; 3])).unwrap();
        chart
            .set_data(candles, ohlc(&[101, 102, 103], 9.0))
            .unwrap();
        // The line still has the old times.
        assert_eq!(chart.time_index.len(), 6);
        chart
            .set_data(line, values(&[101, 102, 103], &[2.0; 3]))
            .unwrap();
        assert_eq!(chart.time_index.times(), &[101, 102, 103]);
        assert_eq!(at(&chart, candles, 102), Some(9.0));
        assert_eq!(at(&chart, line, 103), Some(2.0));
    }

    /// In relative modes every series starts at 0 % / 100 at the first
    /// visible bar, whatever its price level.
    #[test]
    fn relative_scales_normalize_each_series() {
        use crate::price_scale::PriceScaleMode;
        use crate::testing::MonoMeasurer;
        use crate::transform_helpers::y_of_price;

        let (mut chart, a, b) = two_series(SeriesKind::Line, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let times: Vec<i64> = (0..50).collect();
        let va: Vec<f64> = (0..50).map(|i| 10.0 + f64::from(i) * 0.1).collect();
        let vb: Vec<f64> = (0..50).map(|i| 1000.0 - f64::from(i)).collect();
        chart.set_data(a, values(&times, &va)).unwrap();
        chart.set_data(b, values(&times, &vb)).unwrap();
        chart.fit_content();
        let pane = chart.main_pane();
        for (mode, start) in [
            (PriceScaleMode::Percentage, 0.0),
            (PriceScaleMode::IndexedTo100, 100.0),
        ] {
            chart
                .price_scale(pane, ScaleSide::Right)
                .unwrap()
                .set_mode(mode)
                .unwrap();
            chart.build_frame(0.0, &MonoMeasurer);
            let draws: Vec<SeriesDraw> = chart.frame().layers[LayerId::Main as usize]
                .cmds
                .iter()
                .filter_map(|c| match c {
                    DrawCmd::Series(s) => Some(*s),
                    _ => None,
                })
                .collect();
            assert_eq!(draws.len(), 2);
            // The base is the first bar whose index is in the visible range.
            let j0 = chart.time_scale.visible_range().0.floor().max(0.0) as usize;
            let ya = y_of_price(&draws[0].y_transform, va[j0]);
            let yb = y_of_price(&draws[1].y_transform, vb[j0]);
            assert!((ya - yb).abs() < 0.01, "{mode:?}: {ya} vs {yb}");
            // The scale's start value sits at the same y.
            let p = &chart.panes[0];
            let y0 = p
                .price_scale
                .price_to_y(start, f64::from(p.plot.y), f64::from(p.plot.h));
            assert!((y0 as f32 - ya).abs() < 0.01, "{mode:?}");
            // Rising and falling series end on opposite sides of the start.
            let (ea, eb) = (
                y_of_price(&draws[0].y_transform, va[49]),
                y_of_price(&draws[1].y_transform, vb[49]),
            );
            assert!(ea < ya && eb > yb, "{mode:?}");
        }
        // Percent labels.
        let text = &chart.frame().pools.text;
        assert!(!text.contains('%'), "indexed labels have no %");
        chart
            .price_scale(pane, ScaleSide::Right)
            .unwrap()
            .set_mode(PriceScaleMode::Percentage)
            .unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        assert!(chart.frame().pools.text.contains('%'));
    }

    /// Errors leave the time axis and the data unchanged.
    #[test]
    fn failed_operations_leave_the_axis_unchanged() {
        let (mut chart, candles, line) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.set_data(candles, ohlc(&[10, 20, 30], 5.0)).unwrap();
        chart
            .set_data(line, values(&[10, 20, 30], &[1.0; 3]))
            .unwrap();
        let before = chart.time_index.clone();
        let mut bad = ohlc(&[40, 50], 5.0);
        if let SeriesData::Ohlc(c) = &mut bad {
            c.high[1] = f64::INFINITY;
        }
        assert!(chart.append(candles, bad.clone()).is_err()); // past the end
        assert!(matches!(
            chart.append(line, ohlc(&[40], 1.0)),
            Err(Error::InvalidArgument(_))
        )); // wrong kind
        assert_eq!(chart.time_index, before);
        if let SeriesData::Ohlc(c) = &mut bad {
            c.time = vec![15, 25];
        }
        assert!(chart.set_data(candles, bad).is_err()); // a rebuild
        assert_eq!(chart.time_index, before);
        assert_eq!(at(&chart, candles, 20), Some(5.0));
        assert_eq!(
            chart.append(line, values(&[20], &[3.0])),
            Err(Error::TimeNotIncreasing { index: 0 })
        );
    }

    #[test]
    fn chart_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Chart>();
    }

    #[test]
    fn invalid_ids_are_errors() {
        let mut chart = Chart::new(ChartOptions::default());
        let pane = chart.main_pane();
        assert_eq!(
            chart.add_series(PaneId(9), SeriesKind::Candlestick, SeriesOptions::default()),
            Err(Error::UnknownId)
        );
        assert_eq!(chart.remove_series(SeriesId(3)), Err(Error::UnknownId));
        let line = chart
            .add_series(pane, SeriesKind::Line, SeriesOptions::default())
            .unwrap();
        // A line cannot be the source of a volume series.
        assert!(matches!(
            chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(line)),
            Err(Error::InvalidArgument(_))
        ));
        assert!(matches!(
            chart.add_series(
                pane,
                SeriesKind::Volume,
                SeriesOptions::volume_of(SeriesId(0))
            ),
            Err(Error::InvalidArgument(_))
        ));
        assert!(chart.price_scale(pane, ScaleSide::Left).is_none());
        assert_eq!(
            chart.set_series_options(SeriesId(9), SeriesOptions::default()),
            Err(Error::UnknownId)
        );
        assert!(chart.series_options(SeriesId(9)).is_none());
        // The source cannot change.
        assert!(matches!(
            chart.set_series_options(line, SeriesOptions::volume_of(line)),
            Err(Error::InvalidArgument(_))
        ));
        assert_eq!(chart.series_kind(line), Some(SeriesKind::Line));
        assert_eq!(chart.series_pane(line), Some(pane));
        assert_eq!(chart.series_pane(SeriesId(9)), None);
    }

    fn loaded(n: usize, options: ChartOptions) -> (Chart, SeriesId) {
        let mut chart = Chart::new(options);
        chart.resize(SizeCss { w: 800.0, h: 400.0 }, 1.0);
        let pane = chart.main_pane();
        let id = chart
            .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
            .unwrap();
        chart
            .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))
            .unwrap();
        let data = crate::testing::random_walk(9, n, 0, 60_000).unwrap();
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        (chart, id)
    }

    #[test]
    fn redraw_only_when_something_changed() {
        let (mut chart, _) = loaded(1000, ChartOptions::default());
        assert_eq!(chart.needs_redraw(), Invalidation::Full);
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        assert_eq!(chart.needs_redraw(), Invalidation::None);
        chart.time_scale().scroll_by(Px(10.0));
        assert_eq!(chart.needs_redraw(), Invalidation::Light);
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        assert_eq!(chart.needs_redraw(), Invalidation::None);
    }

    #[test]
    fn prepend_keeps_the_view() {
        let (mut chart, id) = loaded(1000, ChartOptions::default());
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        let before = chart.time_scale().visible_range();
        let older = crate::testing::random_walk(10, 500, -500 * 60_000, 60_000).unwrap();
        chart.prepend(id, SeriesData::Ohlc(older)).unwrap();
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        assert_eq!(chart.time_scale().visible_range(), before);
        assert_eq!(chart.data_view().time_index().first_index(), -500);
    }

    #[test]
    fn repaired_bars_produce_a_warning() {
        let (mut chart, id) = loaded(
            10,
            ChartOptions {
                ohlc_validation: ValidationMode::Repair,
                ..ChartOptions::default()
            },
        );
        let mut bad = crate::testing::random_walk(1, 2, 10 * 60_000, 60_000).unwrap();
        bad.high[1] = bad.low[1] - 1.0;
        chart.append(id, SeriesData::Ohlc(bad)).unwrap();
        let events: Vec<_> = chart.drain_events().collect();
        assert_eq!(
            events,
            [ChartEvent::Warning(WarningKind::OhlcRepaired {
                series: id,
                bars: 1
            })]
        );
    }

    #[test]
    fn removing_candles_removes_their_volume() {
        let (mut chart, id) = loaded(100, ChartOptions::default());
        let volume = SeriesId(1);
        assert!(chart.data_view().ohlc(volume, 0).is_some());
        chart.remove_series(id).unwrap();
        assert_eq!(chart.remove_series(volume), Err(Error::UnknownId));
        assert!(chart.data_view().ohlc(id, 0).is_none());
        // An empty chart still builds a (background-only) frame.
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
    }

    #[test]
    fn volume_series_has_no_data_of_its_own() {
        let (mut chart, _) = loaded(10, ChartOptions::default());
        let data = crate::testing::random_walk(1, 10, 0, 60_000).unwrap();
        assert!(matches!(
            chart.set_data(SeriesId(1), SeriesData::Ohlc(data)),
            Err(Error::InvalidArgument(_))
        ));
    }

    /// Text runs of the main layer as `(text, y)`.
    fn main_texts(chart: &Chart) -> Vec<(String, f32)> {
        let f = chart.frame();
        f.layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text(s) => Some(*s),
                _ => None,
            })
            .flat_map(|s| f.pools.text_runs[s.start as usize..(s.start + s.len) as usize].to_vec())
            .map(|r| (f.pools.text_of(r.text).to_owned(), r.pos.y))
            .collect()
    }

    /// Price lines are tied to a series, validated, and removed with it.
    #[test]
    fn price_lines_follow_their_series() {
        let mut chart = Chart::new(ChartOptions::default());
        let pane = chart.main_pane();
        let line = chart
            .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
            .unwrap();
        let vol = chart
            .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(line))
            .unwrap();
        let opts = |price| PriceLineOptions {
            price,
            ..PriceLineOptions::default()
        };
        assert_eq!(
            chart.create_price_line(vol, opts(1.0)),
            Err(Error::InvalidArgument("volume series have no price lines"))
        );
        assert!(chart.create_price_line(line, opts(f64::NAN)).is_err());
        assert_eq!(
            chart.create_price_line(SeriesId(99), opts(1.0)),
            Err(Error::UnknownId)
        );
        let a = chart.create_price_line(line, opts(1.0)).unwrap();
        let b = chart.create_price_line(line, opts(2.0)).unwrap();
        assert_ne!(a, b);
        chart.update_price_line(a, opts(3.0)).unwrap();
        assert_eq!(chart.price_line(a).map(|o| o.price), Some(3.0));
        chart.remove_price_line(b).unwrap();
        assert_eq!(chart.remove_price_line(b), Err(Error::UnknownId));
        chart.remove_series(line).unwrap();
        assert!(chart.price_line(a).is_none());
        assert_eq!(chart.update_price_line(a, opts(1.0)), Err(Error::UnknownId));
    }

    /// Automatic precision shows meme-coin prices with subscript
    /// zeros; a set precision is kept as it is.
    #[test]
    fn automatic_precision_reaches_tiny_prices() {
        use crate::testing::MonoMeasurer;

        let (mut chart, a, _) = two_series(SeriesKind::Line, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let times: Vec<i64> = (0..20).collect();
        let v: Vec<f64> = (0..20)
            .map(|i| 0.000_012_34 + f64::from(i) * 1e-9)
            .collect();
        chart.set_data(a, values(&times, &v)).unwrap();
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let texts: Vec<String> = main_texts(&chart).into_iter().map(|(t, _)| t).collect();
        // The last value 0.000012359, and ticks widened to their step
        // (0.0₄12345).
        assert!(texts.iter().any(|t| t == "0.0₄1236"), "{texts:?}");
        let ticks = texts.iter().filter(|t| t.starts_with("0.0₄123")).count();
        assert!(ticks >= 3, "{texts:?}");

        let k = chart.series.iter().position(|e| e.id == a).unwrap();
        chart.series[k].opts.price_precision = Some(2);
        chart.invalidation = Invalidation::Full;
        chart.build_frame(0.0, &MonoMeasurer);
        let texts: Vec<String> = main_texts(&chart).into_iter().map(|(t, _)| t).collect();
        assert!(texts.iter().any(|t| t == "0.00"), "{texts:?}");
        assert!(!texts.iter().any(|t| t.contains('₄')), "{texts:?}");
    }

    /// Price ticks finer than the series precision get the decimals
    /// they need, so no two ticks read the same.
    #[test]
    fn fine_ticks_are_not_rounded_together() {
        use crate::testing::MonoMeasurer;

        let (mut chart, a, _) = two_series(SeriesKind::Line, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 600.0 }, 1.0);
        let times: Vec<i64> = (0..20).collect();
        let v: Vec<f64> = (0..20).map(|i| 1.0 + f64::from(i) * 0.0002).collect();
        chart.set_data(a, values(&times, &v)).unwrap();
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let ticks: Vec<String> = main_texts(&chart)
            .into_iter()
            .map(|(t, _)| t)
            .filter(|t| t.parse::<f64>().is_ok() && t.len() > "1.00".len())
            .collect();
        assert!(ticks.len() >= 3, "{ticks:?}");
        let mut unique = ticks.clone();
        unique.dedup();
        assert_eq!(unique, ticks);
    }

    /// Value labels never overlap, stay inside the plot, and hide the
    /// ticks they cover.
    #[test]
    fn axis_labels_do_not_overlap() {
        use crate::testing::MonoMeasurer;

        let (mut chart, a, b) = two_series(SeriesKind::Line, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let times: Vec<i64> = (0..20).collect();
        let va: Vec<f64> = (0..20).map(f64::from).collect();
        let mut vb = va.clone();
        vb[19] = 19.01;
        chart.set_data(a, values(&times, &va)).unwrap();
        chart.set_data(b, values(&times, &vb)).unwrap();
        for price in [19.02, 18.99] {
            chart
                .create_price_line(
                    a,
                    PriceLineOptions {
                        price,
                        title: "t".into(),
                        ..PriceLineOptions::default()
                    },
                )
                .unwrap();
        }
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        assert_eq!(chart.axis_labels.len(), 4);
        let plot = chart.panes[0].plot;
        let mut ys: Vec<f32> = chart.axis_labels.iter().map(|l| l.at).collect();
        // The first label (series a) keeps its wanted position.
        assert!((ys[0] - chart.axis_labels[0].want).abs() < 0.01);
        ys.sort_by(f32::total_cmp);
        let texts = main_texts(&chart);
        let gap = ys
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(f32::INFINITY, f32::min);
        assert!(gap >= 10.0, "labels overlap: {ys:?}");
        assert!(ys.iter().all(|&y| y > plot.y && y < plot.bottom()));
        // Titles are drawn; labels show the values.
        assert_eq!(texts.iter().filter(|(t, _)| t == "t").count(), 2);
        for v in ["19.00", "19.01", "19.02", "18.99"] {
            assert!(texts.iter().any(|(t, _)| t == v), "missing {v}: {texts:?}");
        }
        // Turning the marks off removes the labels.
        let k = chart.series.iter().position(|e| e.id == b).unwrap();
        chart.series[k].opts.last_value_visible = false;
        chart.invalidation = Invalidation::Full;
        chart.build_frame(0.0, &MonoMeasurer);
        assert_eq!(chart.axis_labels.len(), 3);
    }

    /// Markers are sorted, snap to the next bar, stack away from the
    /// bar, and skip gap bars.
    #[test]
    fn markers_stack_and_snap() {
        use crate::testing::MonoMeasurer;
        use MarkerPosition::{AboveBar, BelowBar};

        let (mut chart, candles, line) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        chart
            .set_data(candles, ohlc(&[10, 20, 30, 40], 5.0))
            .unwrap();
        // Time 25 puts a gap bar into the candles.
        chart
            .set_data(line, values(&[10, 25, 40], &[5.0, 5.0, 5.0]))
            .unwrap();
        let m = |time, position, text: &str| Marker {
            time,
            position,
            text: text.into(),
            ..Marker::default()
        };
        chart
            .set_markers(
                candles,
                vec![
                    m(40, AboveBar, ""),
                    m(19, AboveBar, "a"), // snaps to 20
                    m(20, AboveBar, ""),
                    m(20, BelowBar, ""),
                    m(25, AboveBar, ""), // gap bar: not drawn
                ],
            )
            .unwrap();
        assert_eq!(
            chart
                .markers(candles)
                .unwrap()
                .iter()
                .map(|m| m.time)
                .collect::<Vec<_>>(),
            [19, 20, 20, 25, 40]
        );
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let icons = chart.frame().pools.icons.clone();
        assert_eq!(icons.len(), 4);
        let x20 = chart.panes[0].plot.x + chart.time_scale.index_to_x(1.0).0;
        let at20: Vec<_> = icons
            .iter()
            .filter(|i| (i.pos.x - x20.round()).abs() < 0.5)
            .collect();
        assert_eq!(at20.len(), 3);
        // Above the bar: the second marker sits above the first and its text.
        let (y_text, y1, y2, y_below) = (
            main_texts(&chart).iter().find(|(t, _)| t == "a").unwrap().1,
            at20[0].pos.y,
            at20[1].pos.y,
            at20[2].pos.y,
        );
        assert!(y2 < y_text && y_text < y1, "{y2} < {y_text} < {y1}");
        assert!(y_below > y1);
        // Errors.
        assert_eq!(
            chart.set_markers(SeriesId(99), vec![]),
            Err(Error::UnknownId)
        );
        let vol = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Volume,
                SeriesOptions::volume_of(candles),
            )
            .unwrap();
        assert!(chart.set_markers(vol, vec![]).is_err());
        assert!(chart.markers(SeriesId(99)).is_none());
    }

    #[test]
    fn marker_fields_are_checked() {
        let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        let ok = Marker::default();
        let bad = [
            Marker {
                size: Some(Css(3.0)),
                ..Marker::default()
            },
            Marker {
                size: Some(Css(f32::NAN)),
                ..Marker::default()
            },
            Marker {
                shape: MarkerShape::ArrowUp,
                label: "DB".into(),
                ..Marker::default()
            },
            Marker {
                shape: MarkerShape::ArrowDown,
                icon: Some(crate::icon::IconId(0)),
                ..Marker::default()
            },
            Marker {
                icon: Some(crate::icon::IconId(0)),
                label: "M".into(),
                ..Marker::default()
            },
            Marker {
                label: "123456789".into(),
                ..Marker::default()
            },
            Marker {
                border_width: Css(-1.0),
                ..Marker::default()
            },
        ];
        chart.set_markers(candles, vec![ok.clone()]).unwrap();
        for m in bad {
            assert!(matches!(
                chart.set_markers(candles, vec![ok.clone(), m]),
                Err(Error::InvalidArgument(_))
            ));
        }
        // Rejected lists leave the markers as they were.
        assert_eq!(chart.markers(candles).map(<[Marker]>::len), Some(1));
        // Eight characters (not bytes) are fine.
        chart
            .set_markers(
                candles,
                vec![Marker {
                    label: "买入卖出买入卖出".into(),
                    ..Marker::default()
                }],
            )
            .unwrap();
        let mut o = SeriesOptions {
            marker_stack_limit: Some(0),
            ..SeriesOptions::default()
        };
        assert!(chart.set_series_options(candles, o).is_err());
        o.marker_stack_limit = Some(3);
        chart.set_series_options(candles, o).unwrap();
    }

    #[test]
    fn markers_with_images_labels_and_folding() {
        use crate::testing::MonoMeasurer;

        let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 600.0 }, 2.0);
        chart
            .set_data(candles, ohlc(&[10, 20, 30, 40], 5.0))
            .unwrap();
        let avatar = chart
            .add_icon(crate::icon::IconImage {
                width: 2,
                height: 2,
                rgba: &[255; 16],
            })
            .unwrap();
        let mut markers: Vec<Marker> = (0..6)
            .map(|_| Marker {
                size: Some(Css(20.0)),
                icon: Some(avatar),
                border: Some(Rgba::hex(0xFF_0000)),
                ..Marker::default()
            })
            .collect();
        markers.push(Marker {
            time: 20,
            label: "DB".into(),
            color: Some(Rgba::hex(0x2F_BF71)),
            ..Marker::default()
        });
        for m in &mut markers[..6] {
            m.time = 30;
        }
        chart.set_markers(candles, markers).unwrap();
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let icons = chart.frame().pools.icons.clone();
        assert_eq!(icons.len(), 7);
        let img = icons
            .iter()
            .filter(|i| i.image == Some(avatar))
            .collect::<Vec<_>>();
        assert_eq!(img.len(), 6);
        // 20 CSS px at dpr 2, border 2 CSS px, stacked 4 CSS px apart.
        assert_eq!((img[0].size, img[0].border_width), (Px(40.0), Px(4.0)));
        assert!((img[0].pos.y - img[1].pos.y - 48.0).abs() < 1e-3);
        // The label is centered in its circle, black or white on the fill.
        let db = icons
            .iter()
            .find(|i| i.color == Rgba::hex(0x2F_BF71))
            .unwrap();
        let labels = main_texts(&chart);
        let (_, y) = labels.iter().find(|(t, _)| t == "DB").unwrap();
        assert!((*y - db.pos.y).abs() < 1e-3);

        // Fold above 4: three markers and a "+3" badge in the series color.
        let mut o = *chart.series_options(candles).unwrap();
        o.marker_stack_limit = Some(4);
        chart.set_series_options(candles, o).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        let icons = chart.frame().pools.icons.clone();
        assert_eq!(icons.iter().filter(|i| i.image == Some(avatar)).count(), 3);
        assert!(main_texts(&chart).iter().any(|(t, _)| t == "+3"));
        let badge = icons
            .iter()
            .find(|i| i.image.is_none() && i.size == Px(40.0))
            .unwrap();
        // Clicking the badge reports the markers it stands for.
        let p = badge.pos;
        let pane = chart.main_pane();
        assert_eq!(
            chart.hit_marker(pane, p),
            Some(ClickTarget::MarkerOverflow {
                series: candles,
                first: 4,
                count: 3,
                id: 0,
                bars: IdxSpan { start: 2, end: 3 },
            })
        );
    }

    #[test]
    fn at_price_markers() {
        use crate::testing::MonoMeasurer;

        let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        chart
            .set_data(candles, ohlc(&[10, 20, 30, 40], 5.0))
            .unwrap();
        // A price only with `AtPrice`, and `AtPrice` needs a finite one.
        for (position, price) in [
            (MarkerPosition::AtPrice, None),
            (MarkerPosition::AtPrice, Some(f64::NAN)),
            (MarkerPosition::AboveBar, Some(5.0)),
        ] {
            let m = Marker {
                time: 20,
                position,
                price,
                ..Marker::default()
            };
            assert!(chart.set_markers(candles, vec![m]).is_err());
        }
        let at = |price: f64, id: u64| Marker {
            time: 20,
            position: MarkerPosition::AtPrice,
            price: Some(price),
            id,
            text: "fill".into(),
            ..Marker::default()
        };
        // Two on their prices and one above the bar: the above one is
        // placed as if it were alone (AtPrice markers do not stack).
        let above = Marker {
            time: 20,
            ..Marker::default()
        };
        chart
            .set_markers(candles, vec![at(4.5, 11), above.clone(), at(5.5, 12)])
            .unwrap();
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let icons = chart.frame().pools.icons.clone();
        assert_eq!(icons.len(), 3);
        let plot = chart.panes[0].plot;
        let y_of = |p: f64| {
            chart.panes[0]
                .price_scale
                .price_to_y(p, f64::from(plot.y), f64::from(plot.h)) as f32
        };
        for (icon, p) in [(icons[0], 4.5), (icons[2], 5.5)] {
            assert!((icon.pos.y - y_of(p).round()).abs() < 1e-3);
        }
        assert!((icons[0].pos.x - icons[1].pos.x).abs() < 1e-3);
        let y_high = y_of(6.0);
        assert!((icons[1].pos.y - (y_high - MARKER_GAP - MARKER_SIZE * 0.5).round()).abs() < 1e-3);
        // Text on the right, left aligned, at the marker's y.
        let f = chart.frame();
        let texts: Vec<TextRun> = f.layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text(s) => Some(*s),
                _ => None,
            })
            .flat_map(|s| f.pools.text_runs[s.start as usize..(s.start + s.len) as usize].to_vec())
            .filter(|t| f.pools.text_of(t.text) == "fill")
            .collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].align, TextAlign::Left);
        assert!((texts[0].pos.x - (icons[0].pos.x + 9.0)).abs() < 1e-3);
        assert!((texts[0].pos.y - icons[0].pos.y).abs() < 1e-3);
        // Clicking one reports its id.
        assert_eq!(
            chart.hit_marker(chart.main_pane(), icons[2].pos),
            Some(ClickTarget::Marker {
                series: candles,
                index: 2,
                id: 12
            })
        );
        // On a log scale, a price <= 0 is not drawn.
        chart
            .set_markers(candles, vec![at(-1.0, 1), at(5.0, 2)])
            .unwrap();
        chart
            .price_scale(chart.main_pane(), ScaleSide::Right)
            .unwrap()
            .set_mode(crate::price_scale::PriceScaleMode::Logarithmic)
            .unwrap();
        chart.build_frame(16.0, &MonoMeasurer);
        assert_eq!(chart.frame().pools.icons.len(), 1);
    }

    /// The "+k" badges and plain markers of the last frame: (target, x).
    fn marker_targets(chart: &Chart) -> Vec<(ClickTarget, f32)> {
        chart
            .marker_hits
            .iter()
            .map(|m| (m.target, m.pos.x))
            .collect()
    }

    #[test]
    fn markers_cluster_across_bars() {
        use crate::testing::MonoMeasurer;
        use MarkerPosition::{AboveBar, AtPrice, BelowBar};

        let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let times: Vec<i64> = (1..=20).map(|i| i * 10).collect();
        chart.set_data(candles, ohlc(&times, 5.0)).unwrap();
        for bad in [0.0, -1.0, f32::NAN] {
            let o = SeriesOptions {
                marker_cluster: Some(Css(bad)),
                ..SeriesOptions::default()
            };
            assert!(chart.set_series_options(candles, o).is_err());
        }
        let m = |bar: usize, id: u64, position: MarkerPosition| Marker {
            time: times[bar],
            position,
            id,
            ..Marker::default()
        };
        let at = Marker {
            price: Some(5.0),
            ..m(3, 8, AtPrice)
        };
        chart
            .set_markers(
                candles,
                vec![
                    // Bucket 0 (bars 0..4): above on bars 1, 2, 2.
                    m(1, 1, AboveBar),
                    m(2, 2, AboveBar),
                    m(2, 3, AboveBar),
                    at,
                    // Bucket 1: two on bar 5 only, not clustered.
                    m(5, 4, AboveBar),
                    m(5, 5, AboveBar),
                    // Bucket 2: below on bars 9 and 10.
                    m(9, 6, BelowBar),
                    m(10, 7, BelowBar),
                ],
            )
            .unwrap();
        chart.time_scale().set_bar_spacing(10.0);
        chart.scroll_to_realtime();
        // Without clustering: every marker is drawn.
        chart.build_frame(0.0, &MonoMeasurer);
        assert_eq!(chart.frame().pools.icons.len(), 8);
        // 35 CSS px over 10 px bars: buckets of 4.
        let o = SeriesOptions {
            marker_cluster: Some(Css(35.0)),
            ..SeriesOptions::default()
        };
        chart.set_series_options(candles, o).unwrap();
        chart.build_frame(16.0, &MonoMeasurer);
        let index_of = |id: u64| {
            chart
                .markers(candles)
                .unwrap()
                .iter()
                .position(|m| m.id == id)
                .unwrap()
        };
        let x_of = |bar: usize| {
            (chart.panes[0].plot.x + chart.time_scale.index_to_x(bar as f64).0).round()
        };
        let mut got = marker_targets(&chart);
        got.sort_by(|a, b| a.1.total_cmp(&b.1));
        let want = vec![
            // Bucket 0 on bar 2 (most markers): the first marker and "+2".
            (
                ClickTarget::Marker {
                    series: candles,
                    index: index_of(1),
                    id: 1,
                },
                x_of(2),
            ),
            (
                ClickTarget::MarkerOverflow {
                    series: candles,
                    first: index_of(2),
                    count: 2,
                    id: 2,
                    bars: IdxSpan { start: 1, end: 3 },
                },
                x_of(2),
            ),
            // AtPrice markers are not clustered.
            (
                ClickTarget::Marker {
                    series: candles,
                    index: index_of(8),
                    id: 8,
                },
                x_of(3),
            ),
            (
                ClickTarget::Marker {
                    series: candles,
                    index: index_of(4),
                    id: 4,
                },
                x_of(5),
            ),
            (
                ClickTarget::Marker {
                    series: candles,
                    index: index_of(5),
                    id: 5,
                },
                x_of(5),
            ),
            // Bucket 2, a tie: on the earlier bar, "+1".
            (
                ClickTarget::Marker {
                    series: candles,
                    index: index_of(6),
                    id: 6,
                },
                x_of(9),
            ),
            (
                ClickTarget::MarkerOverflow {
                    series: candles,
                    first: index_of(7),
                    count: 1,
                    id: 7,
                    bars: IdxSpan { start: 9, end: 11 },
                },
                x_of(9),
            ),
        ];
        // Same x: order by the target's index.
        let key = |t: &ClickTarget| match t {
            ClickTarget::Marker { index, .. } => *index,
            ClickTarget::MarkerOverflow { first, .. } => *first,
            _ => usize::MAX,
        };
        let norm = |mut v: Vec<(ClickTarget, f32)>| {
            v.sort_by(|a, b| a.1.total_cmp(&b.1).then(key(&a.0).cmp(&key(&b.0))));
            v.into_iter().map(|(t, _)| t).collect::<Vec<_>>()
        };
        assert_eq!(norm(got.clone()), norm(want.clone()));
        let sorted = |mut v: Vec<(ClickTarget, f32)>| {
            v.sort_by(|a, b| a.1.total_cmp(&b.1).then(key(&a.0).cmp(&key(&b.0))));
            v
        };
        for ((_, gx), (_, wx)) in sorted(got).into_iter().zip(sorted(want.clone())) {
            assert!((gx - wx).abs() < 1e-3);
        }
        // Panning keeps the groups (buckets are aligned to the index).
        chart.time_scale().scroll_by(Px(7.0));
        chart.build_frame(32.0, &MonoMeasurer);
        assert_eq!(norm(marker_targets(&chart)), norm(want), "after a pan");
        // Zoomed in, buckets of one bar: nothing is clustered.
        chart.time_scale().set_bar_spacing(40.0);
        chart.scroll_to_realtime();
        chart.build_frame(48.0, &MonoMeasurer);
        assert!(
            marker_targets(&chart)
                .iter()
                .all(|(t, _)| matches!(t, ClickTarget::Marker { .. }))
        );
    }

    mod cluster_props {
        use super::*;
        use crate::testing::MonoMeasurer;
        use proptest::prelude::*;

        /// A chart of 60 bars with `markers` (bar, below, id) and clustering
        /// over `d` CSS px, spacing 12 px.
        fn setup(markers: &[(usize, bool)], d: f32) -> (Chart, SeriesId, Vec<i64>) {
            let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
            chart.resize(SizeCss { w: 900.0, h: 300.0 }, 1.0);
            let times: Vec<i64> = (1..=60).map(|i| i * 10).collect();
            chart.set_data(candles, ohlc(&times, 5.0)).unwrap();
            let list = markers
                .iter()
                .enumerate()
                .map(|(n, &(bar, below))| Marker {
                    time: times[bar],
                    position: if below {
                        MarkerPosition::BelowBar
                    } else {
                        MarkerPosition::AboveBar
                    },
                    id: n as u64 + 1,
                    ..Marker::default()
                })
                .collect();
            chart.set_markers(candles, list).unwrap();
            let o = SeriesOptions {
                marker_cluster: Some(Css(d)),
                ..SeriesOptions::default()
            };
            chart.set_series_options(candles, o).unwrap();
            chart.time_scale().set_bar_spacing(12.0);
            chart.scroll_to_realtime();
            (chart, candles, times)
        }

        /// Per side: markers drawn plus the "+k" counts.
        fn shown(chart: &Chart, candles: SeriesId) -> [u32; 2] {
            let markers = chart.markers(candles).unwrap();
            let side = |i: usize| usize::from(markers[i].position == MarkerPosition::BelowBar);
            let mut out = [0; 2];
            for m in &chart.marker_hits {
                match m.target {
                    ClickTarget::Marker { index, .. } => out[side(index)] += 1,
                    ClickTarget::MarkerOverflow { first, count, .. } => out[side(first)] += count,
                    _ => {}
                }
            }
            out
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// With every bar in view, each marker is drawn or counted in
            /// exactly one "+k".
            #[test]
            fn every_marker_is_drawn_or_counted(
                markers in prop::collection::vec((0..60_usize, any::<bool>()), 0..80),
                d in 1.0_f32..80.0,
            ) {
                let (mut chart, candles, _) = setup(&markers, d);
                chart.build_frame(0.0, &MonoMeasurer);
                let below = markers.iter().filter(|m| m.1).count() as u32;
                prop_assert_eq!(shown(&chart, candles), [markers.len() as u32 - below, below]);
            }

            /// Panning does not regroup: the groups of buckets in view before
            /// and after a pan are the same.
            #[test]
            fn panning_keeps_groups(
                markers in prop::collection::vec((0..60_usize, any::<bool>()), 1..80),
                d in 1.0_f32..80.0,
                dx in -300.0_f32..300.0,
            ) {
                let (mut chart, _, _) = setup(&markers, d);
                let kb = (f64::from(d) / 12.0).ceil().max(1.0) as i64;
                let groups = |chart: &mut Chart, t: f64| {
                    chart.build_frame(t, &MonoMeasurer);
                    let (a, b) = chart.time_scale.visible_range();
                    // Buckets wholly in view.
                    let lo = (a.ceil() as i64).div_euclid(kb) + 1;
                    let hi = (b.floor() as i64).div_euclid(kb) - 1;
                    let mut v: Vec<(ClickTarget, i64)> = chart
                        .marker_hits
                        .iter()
                        .filter_map(|m| {
                            let bucket = m.bars.start.div_euclid(kb);
                            (lo..=hi).contains(&bucket).then_some((m.target, bucket))
                        })
                        .collect();
                    v.sort_by_key(|(t, b)| (*b, format!("{t:?}")));
                    (v, lo, hi)
                };
                let (before, lo0, hi0) = groups(&mut chart, 0.0);
                chart.time_scale().scroll_by(Px(dx));
                let (after, lo1, hi1) = groups(&mut chart, 16.0);
                let both = lo0.max(lo1)..=hi0.min(hi1);
                let keep = |v: Vec<(ClickTarget, i64)>| -> Vec<ClickTarget> {
                    v.into_iter().filter(|(_, b)| both.contains(b)).map(|(t, _)| t).collect()
                };
                prop_assert_eq!(keep(before), keep(after));
            }
        }
    }

    #[test]
    fn bar_flags_are_checked_sorted_and_drawn() {
        use crate::testing::MonoMeasurer;

        let (mut chart, candles, line) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let times: Vec<i64> = (1..=20).map(|i| i * 10).collect();
        chart.set_data(candles, ohlc(&times, 5.0)).unwrap();
        assert!(
            chart
                .set_bar_flags(line, vec![(10, BarFlags::FILLED)])
                .is_err()
        );
        assert!(
            chart
                .set_bar_flags(candles, vec![(10, BarFlags(4))])
                .is_err()
        );
        // Sorted; the last of equal times wins; NONE dropped.
        chart
            .set_bar_flags(
                candles,
                vec![
                    (50, BarFlags::CLIPPED),
                    (30, BarFlags::FILLED),
                    (50, BarFlags::FILLED),
                    (70, BarFlags::NONE),
                    (35, BarFlags::FILLED), // not a bar
                ],
            )
            .unwrap();
        assert_eq!(
            chart.bar_flags(candles).unwrap(),
            &[
                (30, BarFlags::FILLED),
                (35, BarFlags::FILLED),
                (50, BarFlags::FILLED)
            ]
        );
        chart.time_scale().set_bar_spacing(16.0);
        chart.scroll_to_realtime();
        chart.build_frame(0.0, &MonoMeasurer);
        let rects = |chart: &Chart| -> Vec<crate::frame::RectInst> {
            let f = chart.frame();
            f.layers[LayerId::Main as usize]
                .cmds
                .iter()
                .filter_map(|c| match c {
                    DrawCmd::Rects(s) => Some(f.pools.rects[s.range()].to_vec()),
                    _ => None,
                })
                .flatten()
                .collect()
        };
        let bg = chart.theme.background;
        let filled = chart.style_of(&chart.series[0]).up.with_alpha(0x66);
        let r = rects(&chart);
        // Two filled bars (bars 2 and 4): each cleared (wick, body) and
        // drawn again; their bodies are 1 px tall (open = close), too thin
        // to be hollow.
        assert_eq!(r.iter().filter(|r| r.color == bg).count(), 4);
        assert_eq!(r.iter().filter(|r| r.color == filled).count(), 4);
        // Clipped: two ticks of 3 px in the wick color.
        chart
            .set_bar_flags(candles, vec![(30, BarFlags::CLIPPED)])
            .unwrap();
        chart.build_frame(16.0, &MonoMeasurer);
        let wick = chart.style_of(&chart.series[0]).up_wick;
        let ticks: Vec<_> = rects(&chart)
            .into_iter()
            .filter(|r| {
                r.color == wick
                    && r.rect.w.to_bits() == 3.0_f32.to_bits()
                    && r.rect.h.to_bits() == 1.0_f32.to_bits()
            })
            .collect();
        assert_eq!(ticks.len(), 2);
        assert!(ticks[1].rect.y > ticks[0].rect.y);
    }

    #[test]
    fn marker_hover_events() {
        use crate::interaction::{InputEvent, Modifiers};
        use crate::testing::MonoMeasurer;

        let (mut chart, candles, _) = two_series(SeriesKind::Candlestick, SeriesKind::Line);
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        chart
            .set_data(candles, ohlc(&[10, 20, 30, 40], 5.0))
            .unwrap();
        chart
            .set_markers(
                candles,
                vec![Marker {
                    time: 20,
                    size: Some(Css(16.0)),
                    id: 7,
                    ..Marker::default()
                }],
            )
            .unwrap();
        chart.fit_content();
        chart.build_frame(0.0, &MonoMeasurer);
        let icon = chart.frame().pools.icons[0];
        let hovers = |chart: &mut Chart| -> Vec<Option<MarkerHit>> {
            chart
                .drain_events()
                .filter_map(|e| match e {
                    ChartEvent::MarkerHover(h) => Some(h),
                    _ => None,
                })
                .collect()
        };
        let mv = |chart: &mut Chart, x: f32, y: f32, t: f64| {
            chart.handle_input(InputEvent::PointerMove {
                id: 0,
                pos: PointCss { x, y },
                mods: Modifiers::default(),
                time_ms: t,
            });
            chart.build_frame(t, &MonoMeasurer);
        };
        let _ = hovers(&mut chart);
        mv(&mut chart, icon.pos.x, icon.pos.y, 16.0);
        let h = hovers(&mut chart);
        assert_eq!(h.len(), 1);
        let hit = h[0].unwrap();
        assert_eq!(
            (hit.series, hit.index, hit.id, hit.overflow, hit.size),
            (candles, 0, 7, 0, Css(16.0))
        );
        assert_eq!(hit.bars, IdxSpan { start: 1, end: 2 });
        // Moving within the marker sends nothing; leaving sends `None`.
        mv(&mut chart, icon.pos.x + 2.0, icon.pos.y, 32.0);
        assert!(hovers(&mut chart).is_empty());
        mv(&mut chart, icon.pos.x, icon.pos.y + 60.0, 48.0);
        assert_eq!(hovers(&mut chart), vec![None]);

        // Without a crosshair, moving the pointer still asks for a frame, in
        // which the hover is decided.
        chart.set_crosshair_mode(CrosshairMode::Hidden);
        chart.build_frame(64.0, &MonoMeasurer);
        assert_eq!(chart.needs_redraw(), Invalidation::None);
        chart.handle_input(InputEvent::PointerMove {
            id: 0,
            pos: PointCss {
                x: icon.pos.x,
                y: icon.pos.y,
            },
            mods: Modifiers::default(),
            time_ms: 80.0,
        });
        assert_ne!(chart.needs_redraw(), Invalidation::None);
        chart.build_frame(80.0, &MonoMeasurer);
        assert_eq!(hovers(&mut chart).len(), 1);
    }

    /// Draws one rectangle filling its area, and reports whether it had
    /// price coordinates.
    struct Fill(crate::plugin::LayerPosition, Rgba);

    impl LayerPlugin for Fill {
        fn position(&self) -> LayerPosition {
            self.0
        }
        fn build(&self, ctx: &LayerCtx<'_>, out: &mut FrameBuilder<'_>) {
            let mut r = ctx.rect();
            if ctx.coords().is_some() {
                r.h /= 2.0;
            }
            out.rect(r, self.1);
        }
    }

    fn rects_of(chart: &Chart, layer: LayerId) -> Vec<(Rgba, RectPx)> {
        let f = chart.frame();
        f.layers[layer as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Rects(span) => Some(*span),
                _ => None,
            })
            .flat_map(|span| {
                let a = span.start as usize;
                f.pools.rects[a..a + span.len as usize]
                    .iter()
                    .map(|r| (r.color, r.rect))
            })
            .collect()
    }

    #[test]
    fn layer_plugins_draw_in_their_layer() {
        let (mut chart, _) = loaded(200, ChartOptions::default());
        let red = Rgba::hex(0xFF_0000);
        let blue = Rgba::hex(0x00_00FF);
        let green = Rgba::hex(0x00_FF00);
        let pane = chart.add_pane(PaneOptions::default());
        let wide = chart
            .add_layer(None, Box::new(Fill(LayerPosition::Background, red)))
            .unwrap();
        chart
            .add_layer(
                Some(chart.main_pane()),
                Box::new(Fill(LayerPosition::AboveSeries, blue)),
            )
            .unwrap();
        chart
            .add_layer(Some(pane), Box::new(Fill(LayerPosition::Overlay, green)))
            .unwrap();
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        let bg = rects_of(&chart, LayerId::Background);
        let main = rects_of(&chart, LayerId::Main);
        let overlay = rects_of(&chart, LayerId::Overlay);
        let plot = chart.layout().plot;
        // Chart-wide: all plots, no price coordinates.
        assert!(
            bg.iter()
                .any(|&(c, r)| c == red && r.y == 0.0 && r.h > plot.h)
        );
        // Main pane, with coordinates: the top half of its plot.
        assert!(main.iter().any(|&(c, r)| c == blue
            && r == RectPx {
                h: plot.h / 2.0,
                ..plot
            }));
        // The empty pane has no coordinates: its whole plot.
        let lower = chart.pane_layout(pane).unwrap().plot;
        assert!(overlay.iter().any(|&(c, r)| c == green && r == lower));

        chart.remove_pane(pane).unwrap();
        chart.remove_layer(wide).unwrap();
        assert_eq!(chart.remove_layer(wide), Err(Error::UnknownId));
        assert_eq!(chart.invalidate_layer(wide), Err(Error::UnknownId));
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        assert!(
            !rects_of(&chart, LayerId::Background)
                .iter()
                .any(|&(c, _)| c == red)
        );
        assert!(
            !rects_of(&chart, LayerId::Overlay)
                .iter()
                .any(|&(c, _)| c == green)
        );
        assert!(
            rects_of(&chart, LayerId::Main)
                .iter()
                .any(|&(c, _)| c == blue)
        );
        assert!(
            chart
                .add_layer(Some(pane), Box::new(Fill(LayerPosition::Overlay, green)))
                .is_err()
        );
    }

    /// A custom series: a rect from `lo` to `hi` per point.
    #[derive(Default)]
    struct Ranges {
        mode: Option<AxisMode>,
        time: Vec<i64>,
        range: Vec<(f64, f64)>,
    }

    impl Ranges {
        fn overlay() -> Self {
            Self {
                mode: Some(AxisMode::Overlay),
                ..Self::default()
            }
        }

        fn push(&mut self, t: i64, lo: f64, hi: f64) -> DataChange {
            self.time.push(t);
            self.range.push((lo, hi));
            DataChange::Append { count: 1 }
        }
    }

    impl CustomSeries for Ranges {
        fn kind(&self) -> &'static str {
            "test.ranges"
        }
        fn axis_mode(&self) -> AxisMode {
            self.mode.unwrap_or(AxisMode::Slots)
        }
        fn times(&self) -> &[Timestamp] {
            &self.time
        }
        fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
            // Linear: fine for a test.
            self.range[from..to]
                .iter()
                .fold(None, |acc, &(l, h)| match acc {
                    None => Some((l, h)),
                    Some((a, b)) => Some((l.min(a), h.max(b))),
                })
        }
        fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
            for i in from..to {
                let (lo, hi) = self.range[i];
                let (Some(x), Some(y0), Some(y1)) = (
                    ctx.x_of_point(i),
                    ctx.coords().y_of(hi),
                    ctx.coords().y_of(lo),
                ) else {
                    continue;
                };
                let rect = RectPx {
                    x: x - 1.0,
                    y: y0,
                    w: 2.0,
                    h: y1 - y0,
                };
                out.rect(rect, ctx.style().up);
            }
        }
        fn last_value(&self) -> Option<f64> {
            self.range.last().map(|r| r.1)
        }
        fn crosshair_value(&self, point: usize) -> Option<f64> {
            self.range.get(point).map(|r| r.1)
        }
        fn native_params(&self, ctx: &SeriesCtx<'_>, params: &mut [f32; 8]) {
            params[0] = ctx.visible().1 as f32;
        }
    }

    const RED: u32 = 0x00FF_0000;

    /// Another custom series type.
    struct Probe;

    impl CustomSeries for Probe {
        fn kind(&self) -> &'static str {
            "test.probe"
        }
        fn axis_mode(&self) -> AxisMode {
            AxisMode::Slots
        }
        fn times(&self) -> &[Timestamp] {
            &[]
        }
        fn extremes(&self, _: usize, _: usize) -> Option<(f64, f64)> {
            None
        }
        fn build(&self, _: &SeriesCtx<'_>, _: usize, _: usize, _: &mut FrameBuilder<'_>) {}
    }

    /// An indicator that computes nothing.
    struct Nothing;

    impl Indicator for Nothing {
        fn name(&self) -> &'static str {
            "nothing"
        }
        fn outputs(&self) -> &[crate::plugin::OutputSpec] {
            const OUT: [crate::plugin::OutputSpec; 1] = [crate::plugin::OutputSpec::line("x")];
            &OUT
        }
        fn default_placement(&self) -> Placement {
            Placement::Overlay
        }
        fn warmup(&self) -> usize {
            0
        }
        fn update(&mut self, _: &IndicatorInput<'_>, _: DataChange, _: &mut IndicatorOutputs) {}
    }

    fn custom_chart(r: Ranges) -> (Chart, SeriesId) {
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let opts = SeriesOptions {
            color: Some(Rgba::hex(RED)),
            ..SeriesOptions::default()
        };
        let id = chart
            .add_custom_series(chart.main_pane(), Box::new(r), opts)
            .unwrap();
        (chart, id)
    }

    fn red_rects(chart: &mut Chart) -> Vec<RectPx> {
        chart.build_frame(0.0, &crate::testing::MonoMeasurer);
        // In the plot: the last-value label on the axis has the color too.
        let right = chart.layout().plot.right();
        rects_of(chart, LayerId::Main)
            .into_iter()
            .filter(|(c, r)| *c == Rgba::hex(RED) && r.x < right)
            .map(|(_, r)| r)
            .collect()
    }

    #[test]
    fn custom_series_take_slots_and_draw() {
        let mut r = Ranges::default();
        for (n, t) in [60_000, 120_000, 180_000].into_iter().enumerate() {
            r.push(t, n as f64, n as f64 + 1.0);
        }
        let (mut chart, id) = custom_chart(r);
        assert_eq!(chart.series_kind(id), Some(SeriesKind::Custom));
        assert_eq!(chart.time_index.times(), &[60_000, 120_000, 180_000]);
        chart.fit_content();
        let rects = red_rects(&mut chart);
        assert_eq!(rects.len(), 3);
        assert!(rects[0].x < rects[1].x && rects[1].x < rects[2].x);
        // Auto scaling covers 0..3: the first rect ends low, the last starts high.
        let plot = chart.layout().plot;
        assert!(rects[0].bottom() > plot.y + plot.h * 0.7);
        assert!(rects[2].y < plot.y + plot.h * 0.3);

        let view = chart.data_view();
        let g = view.generation(id).unwrap();
        let any: &dyn Any = view.custom(id).unwrap();
        assert_eq!(any.downcast_ref::<Ranges>().unwrap().time.len(), 3);

        // Real time: the last point changes, then a new one.
        chart
            .update_custom(id, |r: &mut Ranges| {
                r.range[2].1 = 9.0;
                DataChange::UpdateLast
            })
            .unwrap();
        assert_eq!(
            chart.data_view().changes_since(id, g),
            DataDelta::Range(IdxSpan { start: 2, end: 3 })
        );
        chart
            .update_custom(id, |r: &mut Ranges| r.push(240_000, 1.0, 2.0))
            .unwrap();
        assert_eq!(chart.time_index.len(), 4);
        assert_eq!(chart.custom_series::<Ranges>(id).unwrap().time.len(), 4);
        assert_eq!(chart.entry(id).unwrap().store.bar_at(2), Some([9.0; 4]));
        assert_eq!(chart.last_value(0).map(|v| v.0), Some(2.0));
        chart.fit_content();
        assert_eq!(red_rects(&mut chart).len(), 4);
    }

    #[test]
    fn custom_series_share_the_axis() {
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(SizeCss { w: 400.0, h: 300.0 }, 1.0);
        let pane = chart.main_pane();
        let candles = chart
            .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
            .unwrap();
        chart.set_data(candles, ohlc(&[0, 20, 40], 10.0)).unwrap();
        let mut r = Ranges::default();
        r.push(20, 1.0, 2.0);
        r.push(40, 1.0, 2.0);
        let id = chart
            .add_custom_series(pane, Box::new(r), SeriesOptions::default())
            .unwrap();
        // On existing slots: nothing added.
        assert_eq!(chart.time_index.times(), &[0, 20, 40]);
        let c = |chart: &Chart| chart.entry(id).unwrap().store.span();
        assert_eq!(c(&chart), Some((1, 3)));

        // A time between slots rebuilds the axis; the candles keep their bars.
        chart
            .update_custom(id, |r: &mut Ranges| {
                r.time.insert(1, 30);
                r.range.insert(1, (0.0, 1.0));
                DataChange::Replace
            })
            .unwrap();
        assert_eq!(chart.time_index.times(), &[0, 20, 30, 40]);
        assert_eq!(at(&chart, candles, 40), Some(10.0));
        assert_eq!(merged_warnings(&mut chart), 1);
        assert_eq!(
            chart.entry(id).unwrap().store.custom().unwrap().point_at(3),
            Some(2)
        );

        // Candles rebuilding the axis re-map the custom series.
        chart
            .set_data(candles, ohlc(&[0, 10, 20, 40], 10.0))
            .unwrap();
        assert_eq!(chart.time_index.times(), &[0, 10, 20, 30, 40]);
        let cs = chart.entry(id).unwrap().store.custom().unwrap();
        assert_eq!(
            (cs.point_at(2), cs.point_at(3), cs.point_at(4)),
            (Some(0), Some(1), Some(2))
        );

        // Removing the candles keeps the custom series' slots.
        chart.remove_series(candles).unwrap();
        assert!(chart.time_index.len() >= 3);
        assert_eq!(c(&chart).map(|(a, b)| b - a), Some(3));
        assert!(
            chart
                .add_indicator(id, Box::new(Nothing), Placement::Overlay)
                .is_err()
        );
        assert!(chart.set_data(id, values(&[0], &[1.0])).is_err());
        assert!(
            chart
                .add_series(pane, SeriesKind::Custom, SeriesOptions::default())
                .is_err()
        );
    }

    #[test]
    fn overlay_custom_series_add_no_slots() {
        let (mut chart, id) = custom_chart(Ranges::overlay());
        let line = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Line,
                SeriesOptions::default(),
            )
            .unwrap();
        chart
            .set_data(line, values(&[0, 60, 120], &[0.0, 0.0, 0.0]))
            .unwrap();
        chart
            .update_custom(id, |r: &mut Ranges| {
                for t in [10, 30, 90] {
                    r.push(t, 1.0, 2.0);
                }
                DataChange::Replace
            })
            .unwrap();
        assert_eq!(chart.time_index.len(), 3);
        chart.fit_content();
        let rects = red_rects(&mut chart);
        assert_eq!(rects.len(), 3);
        // x by interpolation: 1/6, 1/2 and 3/2 of the first two slots' gap.
        let x = |i: f64| chart.layout().plot.x + chart.time_scale.index_to_x_f64(i) as f32 - 1.0;
        for (r, i) in rects.iter().zip([1.0 / 6.0, 0.5, 1.5]) {
            assert!((r.x - x(i)).abs() < 0.01, "{} {}", r.x, x(i));
        }
    }

    #[test]
    fn invalid_custom_data_is_an_error() {
        let (mut chart, id) = custom_chart(Ranges::default());
        chart
            .update_custom(id, |r: &mut Ranges| r.push(60, 1.0, 2.0))
            .unwrap();
        chart
            .update_custom(id, |r: &mut Ranges| r.push(120, 1.0, 2.0))
            .unwrap();
        // Not increasing: an error, and nothing is drawn.
        assert_eq!(
            chart.update_custom(id, |r: &mut Ranges| r.push(90, 1.0, 2.0)),
            Err(Error::TimeNotIncreasing { index: 2 })
        );
        assert!(red_rects(&mut chart).is_empty());
        // Another type: `f` is not called.
        assert!(
            chart
                .update_custom(id, |_: &mut Probe| unreachable!())
                .is_err()
        );
        assert!(chart.custom_series::<Probe>(id).is_none());
        // A valid change brings it back.
        chart
            .update_custom(id, |r: &mut Ranges| {
                r.time[2] = 180;
                DataChange::Replace
            })
            .unwrap();
        chart.fit_content();
        assert_eq!(red_rects(&mut chart).len(), 3);
        // Duplicate times are rejected when added.
        let mut dup = Ranges::default();
        dup.push(1, 0.0, 1.0);
        dup.push(1, 0.0, 1.0);
        let pane = chart.main_pane();
        assert!(
            chart
                .add_custom_series(pane, Box::new(dup), SeriesOptions::default())
                .is_err()
        );
        assert_eq!(chart.series.len(), 1);
        chart.remove_series(id).unwrap();
        assert!(chart.custom_series::<Ranges>(id).is_none());
    }

    #[test]
    fn native_kinds_get_a_custom_draw() {
        let mut r = Ranges::default();
        for (n, t) in [60_000, 120_000, 180_000].into_iter().enumerate() {
            r.push(t, n as f64, n as f64 + 1.0);
        }
        let (mut chart, id) = custom_chart(r);
        chart.fit_content();
        let portable = red_rects(&mut chart);
        assert_eq!(portable.len(), 3);

        chart.set_native_kinds(&["test.ranges"]);
        assert_eq!(chart.needs_redraw(), Invalidation::Full);
        assert!(red_rects(&mut chart).is_empty());
        let draws: Vec<CustomDraw> = chart.frame().layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Custom(d) => Some(*d),
                _ => None,
            })
            .collect();
        assert_eq!(draws.len(), 1);
        let d = draws[0];
        assert_eq!((d.series, d.kind), (id, "test.ranges"));
        assert_eq!(d.points, IdxSpan { start: 0, end: 3 });
        assert_eq!(d.clip, chart.layout().plot);
        assert!((d.params[0] - 3.0).abs() < f32::EPSILON);
        assert_eq!(Some(d.generation), chart.data_view().generation(id));
        assert_eq!(chart.frame().styles[d.style.0 as usize].up, Rgba::hex(RED));
        // The same x as the portable path.
        let view = chart.data_view();
        for (i, r) in portable.iter().enumerate() {
            let idx = view.custom_index(id, i).unwrap();
            let x = ((idx - d.x_transform.base as f64) as f32) * d.x_transform.step
                + d.x_transform.offset;
            assert!((x - (r.x + 1.0)).abs() < 1e-3, "{x} {}", r.x + 1.0);
        }
        // Unchanged kinds: nothing to redraw.
        chart.set_native_kinds(&["test.ranges"]);
        assert_eq!(chart.needs_redraw(), Invalidation::None);
        chart.set_native_kinds(&[]);
        assert_eq!(red_rects(&mut chart).len(), 3);
    }

    use crate::testing::{MonoMeasurer as Mono, random_walk};

    /// The lines of the main layer.
    fn main_lines(chart: &Chart) -> Vec<LineInst> {
        let f = chart.frame();
        f.layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Lines(s) => f.pools.lines.get(s.range()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }

    fn transform_chart(t: Box<dyn SeriesTransform>, bars: usize) -> (Chart, SeriesId) {
        let mut chart = Chart::new(ChartOptions {
            min_bar_spacing: Css(0.01),
            ..ChartOptions::default()
        });
        chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
        let id = chart
            .add_transform_series(chart.main_pane(), t, SeriesOptions::default())
            .unwrap();
        let data = random_walk(5, bars, 0, 60_000).unwrap();
        chart.set_data(id, SeriesData::Ohlc(data)).unwrap();
        chart.fit_content();
        (chart, id)
    }

    #[test]
    fn a_transform_series_owns_the_axis() {
        use crate::transform::{Kagi, Renko, Reversal};
        let (mut chart, renko) = transform_chart(Box::new(Renko::fixed(1.0).unwrap()), 2_000);
        let pane = chart.main_pane();
        let n = chart.time_index.len();
        assert!(n > 10 && n < 2_000, "{n} bricks");
        // Synthetic times: strictly increasing, within the input's.
        let times = chart.time_index.times();
        assert!(times.windows(2).all(|w| w[0] < w[1]));
        assert!(times[0] >= 0 && times[n - 1] <= 2_000 * 60_000);
        let opts = SeriesOptions::default;
        assert!(chart.add_series(pane, SeriesKind::Line, opts()).is_err());
        assert!(chart.add_series(pane, SeriesKind::Kagi, opts()).is_err());
        assert!(
            chart
                .add_transform_series(
                    pane,
                    Box::new(Kagi::new(Reversal::Fixed(1.0)).unwrap()),
                    opts()
                )
                .is_err()
        );
        assert!(
            chart
                .add_custom_series(pane, Box::new(Ranges::default()), opts())
                .is_err()
        );
        // Volume reads the bricks' volume.
        chart
            .add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(renko))
            .unwrap();
        chart.build_frame(0.0, &Mono);
        let kinds: Vec<SeriesKind> = chart.frame().layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Series(d) => Some(d.kind),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, [SeriesKind::Volume, SeriesKind::Candlestick]);
        // Without it, time-based series are welcome again, and the other
        // way round a transform series is refused.
        chart.remove_series(renko).unwrap();
        chart.add_series(pane, SeriesKind::Line, opts()).unwrap();
        assert!(
            chart
                .add_transform_series(pane, Box::new(Renko::fixed(1.0).unwrap()), opts())
                .is_err()
        );
    }

    #[test]
    fn indicators_follow_a_transform_series() {
        use crate::transform::{Kagi, Reversal};
        /// The close, doubled.
        struct Double;
        impl Indicator for Double {
            fn name(&self) -> &'static str {
                "double"
            }
            fn outputs(&self) -> &[crate::plugin::OutputSpec] {
                const OUT: [crate::plugin::OutputSpec; 1] = [crate::plugin::OutputSpec::line("d")];
                &OUT
            }
            fn default_placement(&self) -> Placement {
                Placement::Overlay
            }
            fn warmup(&self) -> usize {
                0
            }
            fn update(
                &mut self,
                input: &IndicatorInput<'_>,
                _: DataChange,
                out: &mut IndicatorOutputs,
            ) {
                let start = out.start();
                for (i, v) in out.column(0).iter_mut().enumerate() {
                    if let Some(b) = input.bar(start + i) {
                        *v = 2.0 * b.close;
                    }
                }
            }
        }
        let kagi = || Box::new(Kagi::new(Reversal::Fixed(1.5)).unwrap());
        let (mut chart, id) = transform_chart(kagi(), 300);
        let ind = chart
            .add_indicator(id, Box::new(Double), Placement::Overlay)
            .unwrap();
        let mut all = random_walk(5, 300, 0, 60_000).unwrap();
        let mut bar = BarUpdate {
            time: *all.time.last().unwrap(),
            open: *all.open.last().unwrap(),
            high: *all.high.last().unwrap(),
            low: *all.low.last().unwrap(),
            close: *all.close.last().unwrap(),
            volume: all.volume.as_ref().and_then(|v| v.last().copied()),
        };
        let mut x = 7_u64;
        for i in 0..400 {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let mv = ((x >> 33) as f64 / f64::from(1_u32 << 31) - 0.5) * 2.0;
            if i % 9 == 0 {
                bar = BarUpdate {
                    time: bar.time + 60_000,
                    open: bar.close,
                    high: bar.close,
                    low: bar.close,
                    close: bar.close,
                    volume: Some(0.0),
                };
                all.time.push(bar.time);
                all.open.push(bar.open);
                all.high.push(bar.high);
                all.low.push(bar.low);
                all.close.push(bar.close);
                all.volume.as_mut().unwrap().push(0.0);
            }
            bar.close += mv;
            bar.high = bar.high.max(bar.close);
            bar.low = bar.low.min(bar.close);
            bar.volume = bar.volume.map(|v| v + 1.0);
            let p = all.time.len() - 1;
            all.close[p] = bar.close;
            all.high[p] = bar.high;
            all.low[p] = bar.low;
            all.volume.as_mut().unwrap()[p] = bar.volume.unwrap();
            chart.update_last(id, bar).unwrap();
        }
        let (mut fresh, fid) = transform_chart(kagi(), 1);
        fresh.set_data(fid, SeriesData::Ohlc(all)).unwrap();
        let find = fresh
            .add_indicator(fid, Box::new(Double), Placement::Overlay)
            .unwrap();
        assert_eq!(chart.time_index.times(), fresh.time_index.times());
        let values = |c: &Chart, ind: IndicatorId| -> Vec<u64> {
            let s = c.indicator_series(ind).unwrap()[0];
            let view = c.data_view().ohlc(s, 0).unwrap();
            (0..view.close.len())
                .map(|i| view.close.get(i).unwrap().to_bits())
                .collect()
        };
        assert_eq!(values(&chart, ind), values(&fresh, find));
        assert_eq!(values(&chart, ind).len(), chart.time_index.len());
    }

    #[test]
    fn kagi_and_point_figure_are_primitives() {
        use crate::transform::{Kagi, PointFigure, Reversal};
        let theme = Theme::light();
        // Kagi: thick up-colored and thin down-colored lines.
        let (mut chart, _) =
            transform_chart(Box::new(Kagi::new(Reversal::Fixed(2.0)).unwrap()), 3_000);
        chart.time_scale().set_bar_spacing(8.0);
        chart.build_frame(0.0, &Mono);
        // The Kagi lines (the last-price line is dotted).
        let lines: Vec<LineInst> = main_lines(&chart)
            .into_iter()
            .filter(|l| l.stroke.dash == LineDash::Solid)
            .collect();
        let thick = lines.iter().filter(|l| l.stroke.color == theme.up).count();
        let thin = lines
            .iter()
            .filter(|l| l.stroke.color == theme.down)
            .count();
        assert!(thick > 10 && thin > 10, "{thick} thick, {thin} thin");
        assert!(
            lines
                .iter()
                .filter(|l| l.stroke.color == theme.up)
                .all(|l| l.stroke.width == Px(2.0))
        );
        assert!(
            lines
                .iter()
                .filter(|l| l.stroke.color == theme.down)
                .all(|l| l.stroke.width == Px(1.0))
        );
        // Zoomed far out: at most one line per pixel column.
        chart.time_scale().set_bar_spacing(0.05);
        chart.build_frame(0.0, &Mono);
        let n = main_lines(&chart).len();
        assert!(n > 10 && n <= 800, "{n} lines");
        // Point & Figure: X as crossed lines, O as closed polylines.
        let (mut chart, _) = transform_chart(Box::new(PointFigure::new(0.5, 3).unwrap()), 3_000);
        chart.time_scale().set_bar_spacing(12.0);
        chart.build_frame(0.0, &Mono);
        let xs = main_lines(&chart)
            .iter()
            .filter(|l| l.stroke.color == theme.up && l.stroke.dash == LineDash::Solid)
            .count();
        let os = chart.frame().layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter(|c| matches!(c, DrawCmd::Polyline { style, .. } if style.color == theme.down))
            .count();
        assert!(xs > 10 && xs % 2 == 0 && os > 5, "{xs} X lines, {os} Os");
        // Small boxes: one rect per column.
        chart.time_scale().set_bar_spacing(2.0);
        chart.build_frame(0.0, &Mono);
        let rects = rects_of(&chart, LayerId::Main)
            .iter()
            .filter(|(c, _)| *c == theme.up || *c == theme.down)
            .count();
        assert!(rects > 10, "{rects} columns");
    }

    fn series_draws(chart: &Chart) -> Vec<SeriesDraw> {
        chart.frame().layers[LayerId::Main as usize]
            .cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Series(s) => Some(*s),
                _ => None,
            })
            .collect()
    }

    /// A band is drawn before the series of its pane, with the lower
    /// series as `pair`, and goes away with either series.
    #[test]
    fn bands_follow_their_series() {
        use crate::testing::MonoMeasurer;
        let (mut chart, a, b) = two_series(SeriesKind::Line, SeriesKind::Line);
        chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
        let t = [0, 60_000, 120_000];
        chart.set_data(a, values(&t, &[2.0, 3.0, 1.0])).unwrap();
        chart.set_data(b, values(&t, &[1.0, 1.0, 2.0])).unwrap();
        let candles = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Candlestick,
                SeriesOptions::default(),
            )
            .unwrap();
        assert!(chart.add_band(a, candles, BandOptions::default()).is_err());
        assert!(chart.add_band(a, a, BandOptions::default()).is_err());
        assert_eq!(
            chart.add_band(a, SeriesId(99), BandOptions::default()),
            Err(Error::UnknownId)
        );
        assert!(
            chart
                .add_series(
                    chart.main_pane(),
                    SeriesKind::Band,
                    SeriesOptions::default()
                )
                .is_err()
        );
        let other = chart.add_pane(PaneOptions::default());
        let c = chart
            .add_series(other, SeriesKind::Line, SeriesOptions::default())
            .unwrap();
        assert!(chart.add_band(a, c, BandOptions::default()).is_err());

        let fill = Rgba::hex(0x0011_2233);
        let band = chart
            .add_band(
                a,
                b,
                BandOptions {
                    fill: Some(fill),
                    fill_crossed: None,
                },
            )
            .unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        let draws = series_draws(&chart);
        let kinds: Vec<_> = draws.iter().map(|d| d.kind).collect();
        assert_eq!(kinds[0], SeriesKind::Band, "{kinds:?}");
        assert_eq!((draws[0].series, draws[0].pair), (a, Some(b)));
        let style = chart.frame().styles[draws[0].style.0 as usize];
        // `fill_crossed: None` is the same color.
        assert_eq!((style.fill_top, style.fill_bottom), (fill, fill));
        assert!(draws[1..].iter().all(|d| d.pair.is_none()));

        // Different shifts: not drawn.
        let opts = SeriesOptions {
            shift: 2,
            ..*chart.series_options(b).unwrap()
        };
        chart.set_series_options(b, opts).unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        assert!(
            series_draws(&chart)
                .iter()
                .all(|d| d.kind != SeriesKind::Band)
        );
        let opts = SeriesOptions { shift: 0, ..opts };
        chart.set_series_options(b, opts).unwrap();

        chart
            .set_band_options(band, BandOptions::default())
            .unwrap();
        assert_eq!(chart.band_options(band), Some(&BandOptions::default()));
        chart.remove_series(b).unwrap();
        assert_eq!(chart.band_options(band), None);
        assert_eq!(chart.remove_band(band), Err(Error::UnknownId));
        chart.build_frame(0.0, &MonoMeasurer);
        assert!(
            series_draws(&chart)
                .iter()
                .all(|d| d.kind != SeriesKind::Band)
        );
    }

    /// `OutputSpec::fill_to` must name another output.
    #[test]
    fn fill_to_is_checked() {
        use crate::plugin::{IndicatorInput, IndicatorOutputs, OutputSpec};
        struct Bad([OutputSpec; 2]);
        impl Indicator for Bad {
            fn name(&self) -> &'static str {
                "bad"
            }
            fn outputs(&self) -> &[OutputSpec] {
                &self.0
            }
            fn default_placement(&self) -> Placement {
                Placement::Overlay
            }
            fn warmup(&self) -> usize {
                0
            }
            fn update(&mut self, _: &IndicatorInput<'_>, _: DataChange, _: &mut IndicatorOutputs) {}
        }
        let mut chart = Chart::new(ChartOptions::default());
        let src = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Line,
                SeriesOptions::default(),
            )
            .unwrap();
        for to in [0, 2] {
            let a = OutputSpec {
                fill_to: Some((to, BandOptions::default())),
                ..OutputSpec::line("a")
            };
            let bad = Bad([a, OutputSpec::line("b")]);
            assert!(
                chart
                    .add_indicator(src, Box::new(bad), Placement::Overlay)
                    .is_err()
            );
        }
        assert_eq!(chart.series.len(), 1, "nothing was added");
    }
}
