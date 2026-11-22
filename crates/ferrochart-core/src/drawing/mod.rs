//! Drawing tools.
//!
//! A drawing is a [`DrawingTool`]: anchors in data space ([`Anchor`]: time and
//! price), a style, and code that turns them into frame primitives through a
//! [`FrameBuilder`]. The [`DrawingManager`] (`Chart::drawings`) owns the
//! drawings of a chart: it creates them from [`DrawingData`] through
//! registered factories, keeps their z order, selection, visibility and lock
//! state, and records changes as commands for undo/redo.
//!
//! Anchors are timestamps, not logical indices, so they survive `prepend` and
//! axis rebuilds. Times outside the data are extrapolated with the spacing of
//! the last (or first) two bars; times between two slots are interpolated.

mod advanced;
mod manager;
mod tools;

use core::fmt;

use crate::data::{TimeIndex, Timestamp};
use crate::format::NumberFormat;
use crate::frame::{
    DrawCmd, FontId, FontStyleRef, Frame, IconInst, LabelBg, LayerId, LineDash, LineInst, RectInst,
    StrokeStyle, TextAlign, TextRun,
};
use crate::geom::{Css, PointPx, Px, RectPx, Rgba, Span};
use crate::price_scale::{Normalize, PriceScale};
use crate::text::{TextMeasurer, text_width};
use crate::time_scale::TimeScale;

pub(crate) use manager::LocalChange;
pub use manager::{DrawingFactory, DrawingManager};
/// Kinds of the built-in tools: the first batch, then the second. Long and short positions, the three pitchforks and the two Elliott
/// waves are one tool each with several kinds.
pub const BUILTIN_KINDS: [&str; 29] = [
    "trend_line",
    "ray",
    "extended_line",
    "horizontal_line",
    "horizontal_ray",
    "vertical_line",
    "parallel_channel",
    "rectangle",
    "ellipse",
    "text",
    "price_range",
    "date_range",
    "fib_retracement",
    "fib_extension",
    "long_position",
    "short_position",
    "pitchfork",
    "schiff_pitchfork",
    "modified_schiff_pitchfork",
    "gann_fan",
    "fib_time_zones",
    "xabcd_pattern",
    "head_and_shoulders",
    "elliott_impulse",
    "elliott_correction",
    "path",
    "brush",
    "arrow",
    "callout",
];

/// Factory of the built-in kinds.
pub(crate) fn builtin(data: &DrawingData) -> crate::error::Result<Box<dyn DrawingTool>> {
    if advanced::SECOND_KINDS.contains(&data.kind.as_str()) {
        advanced::build(data)
    } else {
        tools::builtin(data)
    }
}

/// Spacing used to extrapolate times when the axis has a single slot (ms).
const DEFAULT_STEP_MS: f64 = 60_000.0;
/// Hit tolerance around lines (CSS px).
pub(crate) const HIT_CSS: f32 = 4.0;

/// Most anchors of an `Open` or `Freehand` drawing.
pub const MAX_ANCHORS: usize = 4096;

/// Distance (CSS px) the pointer moves before a freehand drawing gets its
/// next anchor.
pub(crate) const FREEHAND_STEP_CSS: f32 = 3.0;

/// How a drawing is created.
///
/// # Examples
///
/// ```
/// use ferrochart_core::drawing::AnchorCount;
///
/// assert_eq!(AnchorCount::Fixed(3).min(), 3);
/// assert_eq!(AnchorCount::Open { min: 2 }.min(), 2);
/// assert_eq!(AnchorCount::Freehand.min(), 2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnchorCount {
    /// One click per anchor, `n` clicks (tools with more anchors add them in
    /// [`DrawingTool::finish`]).
    Fixed(usize),
    /// One click per anchor until a double click or Enter, at least `min`
    /// anchors (e.g. a path).
    Open {
        /// Fewest anchors.
        min: usize,
    },
    /// Press, drag and release: an anchor every 3 CSS px of movement (e.g. a
    /// brush), at least 2.
    Freehand,
}

impl AnchorCount {
    /// Fewest anchors a drawing of this kind has.
    pub fn min(self) -> usize {
        match self {
            Self::Fixed(n) | Self::Open { min: n } => n,
            Self::Freehand => 2,
        }
    }

    /// Most anchors (`Fixed`: the clicks; tools may add more in `finish`).
    pub(crate) fn max(self) -> usize {
        match self {
            Self::Fixed(n) => n,
            Self::Open { .. } | Self::Freehand => MAX_ANCHORS,
        }
    }
}

/// A point of a drawing in data space.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Anchor {
    /// Time (UTC milliseconds); need not be a bar time.
    pub time: Timestamp,
    /// Price, in the units of the pane's first price series.
    pub price: f64,
}

/// Colors and line settings of a drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DrawingStyle {
    /// Line color.
    pub line: Rgba,
    /// Line width.
    pub line_width: Css,
    /// Line dash pattern.
    pub line_dash: LineDash,
    /// Fill color of areas; alpha 0 draws no fill.
    pub fill: Rgba,
    /// Text color.
    pub text: Rgba,
    /// Font size of texts and labels.
    pub font_size: Css,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        let blue = Rgba::hex(0x0029_62FF);
        Self {
            line: blue,
            line_width: Css(1.0),
            line_dash: LineDash::Solid,
            fill: blue.with_alpha(40),
            text: blue,
            font_size: Css(12.0),
        }
    }
}

/// A tool-specific parameter of a drawing.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Param {
    /// A number.
    Number(f64),
    /// A flag.
    Bool(bool),
    /// A text.
    Text(String),
    /// A list of numbers (e.g. Fibonacci levels).
    Numbers(Vec<f64>),
}

/// Format-independent saved form of a drawing: what
/// [`DrawingTool::save`] returns and what factories build drawings from.
///
/// # Examples
///
/// ```
/// use ferrochart_core::drawing::{Anchor, DrawingData, Param};
///
/// let mut data = DrawingData::new("text", vec![Anchor { time: 0, price: 10.0 }]);
/// data.set("text", Param::Text("breakout".into()));
/// assert_eq!(data.text("text"), Some("breakout"));
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DrawingData {
    /// Tool kind, e.g. `"trend_line"` (see [`BUILTIN_KINDS`]).
    pub kind: String,
    /// Anchors.
    pub anchors: Vec<Anchor>,
    /// Style.
    pub style: DrawingStyle,
    /// Tool-specific parameters, by key.
    pub params: Vec<(String, Param)>,
}

impl DrawingData {
    /// A drawing of `kind` with the default style and no parameters.
    pub fn new(kind: &str, anchors: Vec<Anchor>) -> Self {
        Self {
            kind: kind.into(),
            anchors,
            style: DrawingStyle::default(),
            params: Vec::new(),
        }
    }

    /// The parameter `key`.
    pub fn param(&self, key: &str) -> Option<&Param> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Sets the parameter `key`.
    pub fn set(&mut self, key: &str, value: Param) {
        match self.params.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.params.push((key.into(), value)),
        }
    }

    /// The number parameter `key`.
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.param(key)? {
            Param::Number(x) => Some(*x),
            _ => None,
        }
    }

    /// The text parameter `key`.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.param(key)? {
            Param::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The number-list parameter `key`.
    pub fn numbers(&self, key: &str) -> Option<&[f64]> {
        match self.param(key)? {
            Param::Numbers(v) => Some(v),
            _ => None,
        }
    }
}

/// What part of a drawing a point hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitPart {
    /// Handle `n` of [`DrawingTool::handles`].
    Handle(usize),
    /// The drawing itself.
    Body,
}

/// A drawing tool. Built-in tools are listed in
/// [`BUILTIN_KINDS`]; custom tools are registered with
/// [`DrawingManager::register`].
///
/// `build` runs for every frame and must not allocate (it only writes into
/// the frame's pools through `out`).
pub trait DrawingTool: Send {
    /// Kind, e.g. `"trend_line"`.
    fn kind(&self) -> &'static str;
    /// Anchors.
    fn anchors(&self) -> &[Anchor];
    /// Anchors, for moving the drawing.
    fn anchors_mut(&mut self) -> &mut Vec<Anchor>;
    /// How the drawing is created.
    fn anchor_count(&self) -> AnchorCount;
    /// Draws the drawing.
    fn build(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>);
    /// What `p` hits, if anything (within [`DrawingCtx::hit_tolerance`] of
    /// lines).
    fn hit_test(&self, ctx: &DrawingCtx<'_>, p: PointPx) -> Option<HitPart>;
    /// Handles of the selected drawing, written into `out` (empty on entry;
    /// core reuses it, so any number of handles does not allocate). Handle
    /// `k` is dragged with [`move_handle`](Self::move_handle)`(k, …)`.
    fn handles(&self, ctx: &DrawingCtx<'_>, out: &mut Vec<PointPx>);
    /// Style.
    fn style(&self) -> &DrawingStyle;
    /// Saved form; a factory given it builds an equal drawing.
    fn save(&self) -> DrawingData;
    /// Drags handle `handle` to `to`. The default moves anchor `handle`.
    fn move_handle(&mut self, handle: usize, to: Anchor) {
        if let Some(a) = self.anchors_mut().get_mut(handle) {
            *a = to;
        }
    }
    /// Called once the drawing is created (after the last click, or by
    /// `DrawingManager::add`): tools with more anchors than clicks add the
    /// others here.
    fn finish(&mut self) {}
}

/// Coordinate conversions and measurements for [`DrawingTool`]s of one pane.
pub struct DrawingCtx<'a> {
    pub(crate) index: &'a TimeIndex,
    pub(crate) time_scale: &'a TimeScale,
    pub(crate) price_scale: &'a PriceScale,
    pub(crate) norm: Normalize,
    pub(crate) plot: RectPx,
    pub(crate) dpr: f32,
    pub(crate) labels: NumberFormat,
    pub(crate) text: &'a dyn TextMeasurer,
}

impl DrawingCtx<'_> {
    /// The pane's plot rectangle.
    pub fn plot(&self) -> RectPx {
        self.plot
    }

    /// Device pixel ratio.
    pub fn dpr(&self) -> f32 {
        self.dpr
    }

    /// `v` in physical pixels.
    pub fn px(&self, v: Css) -> Px {
        v.to_px(self.dpr)
    }

    /// Distance within which a point hits a line.
    pub fn hit_tolerance(&self) -> f32 {
        HIT_CSS * self.dpr
    }

    /// Width of one bar.
    pub fn bar_spacing(&self) -> Px {
        Px(self.time_scale.bar_spacing() as f32)
    }

    /// Logical index of `time`, fractional between slots and extrapolated
    /// outside the data; `None` without data.
    pub fn index_of_time(&self, time: Timestamp) -> Option<f64> {
        index_of_time(self.index, time)
    }

    /// x of the fractional logical index `i`.
    pub(crate) fn x_of_index(&self, i: f64) -> Option<f32> {
        let x = f64::from(self.plot.x) + self.time_scale.index_to_x_f64(i);
        x.is_finite().then_some(x.clamp(-1e7, 1e7) as f32)
    }

    /// x of `time`.
    pub fn x_of(&self, time: Timestamp) -> Option<f32> {
        let i = self.index_of_time(time)?;
        let x = f64::from(self.plot.x) + self.time_scale.index_to_x_f64(i);
        x.is_finite().then_some(x as f32)
    }

    /// y of `price`; `None` if it has none (e.g. ≤ 0 on a log scale).
    pub fn y_of(&self, price: f64) -> Option<f32> {
        let (top, h) = (f64::from(self.plot.y), f64::from(self.plot.h));
        let y = self.price_scale.price_to_y(self.norm.apply(price), top, h);
        // Keep far-away points finite for f32 geometry.
        y.is_finite().then_some(y.clamp(-1e7, 1e7) as f32)
    }

    /// Price at `y`.
    pub fn price_at(&self, y: f32) -> f64 {
        let (top, h) = (f64::from(self.plot.y), f64::from(self.plot.h));
        self.norm
            .invert(self.price_scale.y_to_price(f64::from(y), top, h))
    }

    /// Time at `x`: interpolated between slots, extrapolated beyond the
    /// data (the inverse of [`x_of`](Self::x_of), rounded to the
    /// millisecond); `None` without data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::atomic::{AtomicI64, Ordering};
    ///
    /// use ferrochart_core::chart::{SeriesData, SeriesOptions};
    /// use ferrochart_core::data::ValueColumns;
    /// use ferrochart_core::drawing::FrameBuilder;
    /// use ferrochart_core::frame::SeriesKind;
    /// use ferrochart_core::geom::SizeCss;
    /// use ferrochart_core::plugin::{LayerCtx, LayerPlugin, LayerPosition};
    /// use ferrochart_core::testing::MonoMeasurer;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// static TIME: AtomicI64 = AtomicI64::new(0);
    ///
    /// struct Probe;
    ///
    /// impl LayerPlugin for Probe {
    ///     fn position(&self) -> LayerPosition {
    ///         LayerPosition::Overlay
    ///     }
    ///     fn build(&self, ctx: &LayerCtx<'_>, _: &mut FrameBuilder<'_>) {
    ///         let Some(c) = ctx.coords() else { return };
    ///         // Half way between the bars at 60 s and 120 s.
    ///         if let Some(t) = c.x_of(90_000).and_then(|x| c.time_at(x)) {
    ///             TIME.store(t, Ordering::Relaxed);
    ///         }
    ///     }
    /// }
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.resize(SizeCss { w: 800.0, h: 400.0 }, 1.0);
    /// let pane = chart.main_pane();
    /// let line = chart.add_series(pane, SeriesKind::Line, SeriesOptions::default())?;
    /// chart.set_data(line, SeriesData::Value(ValueColumns {
    ///     time: (0..10).map(|i| i * 60_000).collect(),
    ///     value: vec![1.0; 10],
    ///     color: None,
    /// }))?;
    /// chart.add_layer(Some(pane), Box::new(Probe))?;
    /// chart.build_frame(0.0, &MonoMeasurer);
    /// assert_eq!(TIME.load(Ordering::Relaxed), 90_000);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn time_at(&self, x: f32) -> Option<Timestamp> {
        let i = self.time_scale.x_to_index(Px(x - self.plot.x));
        time_of_index(self.index, i)
    }

    /// Screen position of `a`.
    pub fn point(&self, a: Anchor) -> Option<PointPx> {
        Some(PointPx {
            x: self.x_of(a.time)?.clamp(-1e7, 1e7),
            y: self.y_of(a.price)?,
        })
    }

    /// Stroke of `style`'s lines (whole pixels, at least 1).
    pub fn stroke(&self, style: &DrawingStyle) -> StrokeStyle {
        StrokeStyle {
            width: Px(style.line_width.to_px(self.dpr).0.round().max(1.0)),
            color: style.line,
            dash: style.line_dash,
        }
    }

    /// Font of `size`.
    pub fn font(&self, size: Css) -> FontStyleRef {
        FontStyleRef {
            font: FontId(0),
            size: size.to_px(self.dpr).0,
        }
    }

    /// Width of `text`.
    pub fn text_width(&self, text: &str, font: FontStyleRef) -> f32 {
        text_width(self.text, text, font)
    }

    /// Line height of `font`.
    pub fn line_height(&self, font: FontStyleRef) -> f32 {
        self.text.line_height(font)
    }

    /// Writes `price` with the pane's precision.
    pub fn write_price(&self, out: &mut impl fmt::Write, price: f64) {
        match self.labels {
            f @ (NumberFormat::Fixed(_) | NumberFormat::Auto(_)) => f,
            NumberFormat::Percent(_) | NumberFormat::Compact => NumberFormat::Fixed(2),
        }
        .write(out, price);
    }
}

/// Writes the primitives of a drawing into the frame (main layer, clipped to
/// the pane by the caller).
pub struct FrameBuilder<'a> {
    pub(crate) frame: &'a mut Frame,
    pub(crate) layer: LayerId,
}

impl<'a> FrameBuilder<'a> {
    /// A builder writing into `layer` of `frame`, for code that builds
    /// frames outside a [`Chart`](crate::Chart) (`ferrochart-plot`).
    ///
    /// ```
    /// use ferrochart_core::drawing::FrameBuilder;
    /// use ferrochart_core::frame::{Frame, LayerId, StrokeStyle};
    /// use ferrochart_core::geom::{PointPx, Px, Rgba};
    ///
    /// let mut frame = Frame::default();
    /// let mut b = FrameBuilder::new(&mut frame, LayerId::Main);
    /// let stroke = StrokeStyle { color: Rgba::hex(0xFFFF_FFFF), width: Px(1.0), dash: Default::default() };
    /// b.line(PointPx { x: 0.0, y: 0.0 }, PointPx { x: 10.0, y: 0.0 }, stroke);
    /// assert_eq!(frame.pools.lines.len(), 1);
    /// ```
    pub fn new(frame: &'a mut Frame, layer: LayerId) -> Self {
        Self { frame, layer }
    }

    /// A line segment.
    pub fn line(&mut self, from: PointPx, to: PointPx, stroke: StrokeStyle) {
        self.frame
            .push_line(self.layer, LineInst { from, to, stroke });
    }

    fn points(&mut self, pts: impl IntoIterator<Item = PointPx>) -> Span {
        let start = self.frame.pools.points.len();
        self.frame.pools.points.extend(pts);
        let len = self.frame.pools.points.len() - start;
        Span {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            len: u32::try_from(len).unwrap_or(0),
        }
    }

    /// A polyline through `pts`.
    pub fn polyline(&mut self, pts: impl IntoIterator<Item = PointPx>, style: StrokeStyle) {
        let points = self.points(pts);
        if points.len >= 2 {
            self.frame
                .push_cmd(self.layer, DrawCmd::Polyline { points, style });
        }
    }

    /// A filled polygon with vertices `pts`.
    pub fn polygon(&mut self, pts: impl IntoIterator<Item = PointPx>, fill: Rgba) {
        let points = self.points(pts);
        if points.len >= 3 && fill.a > 0 {
            self.frame
                .push_cmd(self.layer, DrawCmd::Polygon { points, fill });
        }
    }

    /// A filled axis-aligned rectangle.
    pub fn rect(&mut self, rect: RectPx, color: Rgba) {
        if color.a > 0 && rect.w > 0.0 && rect.h > 0.0 {
            self.frame.push_rect(self.layer, RectInst { rect, color });
        }
    }

    /// A line of text; `pos.y` is its vertical center.
    pub fn text(&mut self, pos: PointPx, text: &str, label: Label) {
        let span = self.frame.push_str(text);
        self.push_run(pos, span, label);
    }

    /// A line of formatted text, written straight into the frame's text pool.
    pub fn text_fmt(&mut self, pos: PointPx, args: fmt::Arguments<'_>, label: Label) {
        let start = self.frame.pools.text.len();
        let _ = fmt::Write::write_fmt(&mut self.frame.pools.text, args);
        let len = self.frame.pools.text.len() - start;
        let span = Span {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            len: u32::try_from(len).unwrap_or(0),
        };
        self.push_run(pos, span, label);
    }

    fn push_run(&mut self, pos: PointPx, text: Span, label: Label) {
        self.frame.push_text(
            self.layer,
            TextRun {
                pos,
                text,
                style: label.font,
                color: label.color,
                align: label.align,
                bg: label.bg,
            },
        );
    }

    /// An icon from the built-in set (`transform_helpers::ICON_*`).
    pub fn icon(&mut self, icon: IconInst) {
        self.frame.push_icon(self.layer, icon);
    }
}

/// How [`FrameBuilder::text`] draws a text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Label {
    /// Font.
    pub font: FontStyleRef,
    /// Text color.
    pub color: Rgba,
    /// Alignment relative to `pos.x`.
    pub align: TextAlign,
    /// Optional background box.
    pub bg: Option<LabelBg>,
}

/// Distance from `p` to the segment `a`–`b`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::drawing::distance_to_segment;
/// use ferrochart_core::geom::PointPx;
///
/// let (a, b) = (PointPx { x: 0.0, y: 0.0 }, PointPx { x: 10.0, y: 0.0 });
/// assert_eq!(distance_to_segment(PointPx { x: 5.0, y: 3.0 }, a, b), 3.0);
/// assert_eq!(distance_to_segment(PointPx { x: 13.0, y: 4.0 }, a, b), 5.0);
/// ```
pub fn distance_to_segment(p: PointPx, a: PointPx, b: PointPx) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// The part of the line through `a` and `b` inside `rect`: from `a` (or from
/// the edge if `back`) to `b` (or to the edge if `forward`). `None` if it
/// misses the rectangle.
///
/// # Examples
///
/// ```
/// use ferrochart_core::drawing::clip_line;
/// use ferrochart_core::geom::{PointPx, RectPx};
///
/// let r = RectPx { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
/// let (a, b) = (PointPx { x: 10.0, y: 50.0 }, PointPx { x: 20.0, y: 50.0 });
/// let (p, q) = clip_line(a, b, r, false, true).unwrap();
/// assert_eq!((p.x, q.x), (10.0, 100.0));
/// ```
pub fn clip_line(
    a: PointPx,
    b: PointPx,
    rect: RectPx,
    back: bool,
    forward: bool,
) -> Option<(PointPx, PointPx)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx == 0.0 && dy == 0.0 {
        return Some((a, b));
    }
    let mut t0 = if back { f32::NEG_INFINITY } else { 0.0 };
    let mut t1 = if forward { f32::INFINITY } else { 1.0 };
    // Liang–Barsky.
    for (p, q) in [
        (-dx, a.x - rect.x),
        (dx, rect.right() - a.x),
        (-dy, a.y - rect.y),
        (dy, rect.bottom() - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            t0 = t0.max(r);
        } else {
            t1 = t1.min(r);
        }
    }
    if t0 > t1 || !t0.is_finite() || !t1.is_finite() {
        return None;
    }
    let at = |t: f32| PointPx {
        x: a.x + t * dx,
        y: a.y + t * dy,
    };
    Some((at(t0), at(t1)))
}

/// Whether `p` is inside the polygon `pts` (even-odd rule).
pub fn point_in_polygon(p: PointPx, pts: &[PointPx]) -> bool {
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for (i, a) in pts.iter().enumerate() {
        let Some(b) = pts.get(j) else {
            return false;
        };
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Spacing of the slots after the last one (`at_end`) or before the first.
fn step(index: &TimeIndex, at_end: bool) -> f64 {
    let (first, end) = (index.first_index(), index.end_index());
    let pair = if at_end {
        (index.time_at(end - 2), index.time_at(end - 1))
    } else {
        (index.time_at(first), index.time_at(first + 1))
    };
    match pair {
        (Some(a), Some(b)) if b > a => (b - a) as f64,
        _ => DEFAULT_STEP_MS,
    }
}

/// Logical index of `time`: exact on slots, interpolated between them,
/// extrapolated outside.
pub(crate) fn index_of_time(index: &TimeIndex, time: Timestamp) -> Option<f64> {
    if index.is_empty() {
        return None;
    }
    let (first, last) = (index.first_index(), index.end_index() - 1);
    let (t0, t1) = (index.time_at(first)?, index.time_at(last)?);
    if time > t1 {
        return Some(last as f64 + (time - t1) as f64 / step(index, true));
    }
    if time < t0 {
        return Some(first as f64 - (t0 - time) as f64 / step(index, false));
    }
    let j = index.lower_bound(time);
    let tj = index.time_at(j)?;
    if tj == time {
        return Some(j as f64);
    }
    let tp = index.time_at(j - 1)?;
    Some((j - 1) as f64 + (time - tp) as f64 / (tj - tp) as f64)
}

/// Time of the fractional logical index `i` (inverse of [`index_of_time`]).
pub(crate) fn time_of_index(index: &TimeIndex, i: f64) -> Option<Timestamp> {
    if index.is_empty() || !i.is_finite() {
        return None;
    }
    let (first, last) = (index.first_index(), index.end_index() - 1);
    let (t0, t1) = (index.time_at(first)?, index.time_at(last)?);
    let t = if i >= last as f64 {
        t1 as f64 + (i - last as f64) * step(index, true)
    } else if i <= first as f64 {
        t0 as f64 - (first as f64 - i) * step(index, false)
    } else {
        let j = i.floor() as i64;
        let (a, b) = (index.time_at(j)?, index.time_at(j + 1)?);
        a as f64 + (i - j as f64) * (b - a) as f64
    };
    Some(t.round().clamp(-9.0e18, 9.0e18) as i64)
}

/// Pixel center of a 1-px (odd width) line at `v`.
pub(crate) fn crisp(v: f32, width: Px) -> f32 {
    if (width.0 as i32) % 2 == 1 {
        v.floor() + 0.5
    } else {
        v.round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_map_to_indices_and_back() {
        let index = TimeIndex::from_times(vec![100, 200, 400]).unwrap_or_default();
        assert_eq!(index_of_time(&index, 200), Some(1.0));
        assert_eq!(index_of_time(&index, 300), Some(1.5));
        // Extrapolated with the last spacing (200) and the first one (100).
        assert_eq!(index_of_time(&index, 800), Some(4.0));
        assert_eq!(index_of_time(&index, 0), Some(-1.0));
        for i in [-3.0, -0.5, 0.0, 1.25, 2.0, 7.0] {
            let t = time_of_index(&index, i).unwrap_or_default();
            let back = index_of_time(&index, t).unwrap_or_default();
            assert!((back - i).abs() < 1e-9, "{i} -> {t} -> {back}");
        }
        assert_eq!(index_of_time(&TimeIndex::default(), 5), None);
    }

    #[test]
    fn lines_are_clipped_to_the_plot() {
        let r = RectPx {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let (a, b) = (PointPx { x: 10.0, y: 10.0 }, PointPx { x: 20.0, y: 20.0 });
        let (p, q) = clip_line(a, b, r, true, true).unwrap_or((a, a));
        assert_eq!(
            (p, q),
            (PointPx { x: 0.0, y: 0.0 }, PointPx { x: 50.0, y: 50.0 })
        );
        assert_eq!(clip_line(a, b, r, false, false), Some((a, b)));
        let outside = PointPx { x: 200.0, y: 10.0 };
        assert_eq!(
            clip_line(outside, PointPx { x: 300.0, y: 10.0 }, r, false, true),
            None
        );
    }

    #[test]
    fn polygons_contain_points() {
        let sq = [
            PointPx { x: 0.0, y: 0.0 },
            PointPx { x: 10.0, y: 0.0 },
            PointPx { x: 10.0, y: 10.0 },
            PointPx { x: 0.0, y: 10.0 },
        ];
        assert!(point_in_polygon(PointPx { x: 5.0, y: 5.0 }, &sq));
        assert!(!point_in_polygon(PointPx { x: 15.0, y: 5.0 }, &sq));
        assert!(!point_in_polygon(PointPx { x: 5.0, y: 5.0 }, &[]));
    }
}
