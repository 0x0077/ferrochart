//! `Frame`: the platform-independent draw description that the core produces and
//! the backends consume.
//!
//! The frame uses **flat pools**: every primitive instance lives in one `Vec` per
//! type at frame level, and a [`DrawCmd`] only refers to a [`Span`] of a pool.
//! Pools are cleared (keeping their capacity) at the start of each frame, so after
//! a few frames building a frame no longer allocates. `DrawCmd` and everything it
//! contains is `Copy`.

use crate::data::IdxSpan;
use crate::geom::{PointPx, Px, RectPx, Rgba, SizePx, Span};

/// How much of the chart needs to be redrawn. Ordered from least to most work.
///
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::Invalidation;
///
/// assert!(Invalidation::Full > Invalidation::Light);
/// assert_eq!(Invalidation::Cursor.max(Invalidation::Data), Invalidation::Data);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Invalidation {
    /// Nothing to redraw.
    #[default]
    None,
    /// Only the crosshair / hover state changed: redraw the overlay layer.
    Cursor,
    /// The viewport changed (pan/zoom): uniforms, ticks and text.
    Light,
    /// Data changed: incremental upload plus `Light`.
    Data,
    /// Layout, theme or the set of series changed: rebuild everything.
    Full,
}

/// Identifier of a font known to the [`TextMeasurer`](crate::text::TextMeasurer).
/// `FontId(0)` is the built-in default font.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontId(pub u16);

/// Font and size of a text run.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{FontId, FontStyleRef};
///
/// let style = FontStyleRef { font: FontId(0), size: 12.0 };
/// assert_eq!(style.size, 12.0);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FontStyleRef {
    /// Font.
    pub font: FontId,
    /// Font size (em size) in physical pixels.
    pub size: f32,
}

/// Horizontal alignment of a text run relative to its position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    /// `pos.x` is the left edge.
    #[default]
    Left,
    /// `pos.x` is the center.
    Center,
    /// `pos.x` is the right edge.
    Right,
}

/// Background box behind a label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelBg {
    /// Fill color.
    pub color: Rgba,
    /// Padding around the text box.
    pub padding: Px,
}

/// One line of text. The characters are `FramePools::text[text.range()]`.
///
/// `pos.y` is the vertical center of the line box (ascent to descent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRun {
    /// Anchor point; see [`TextAlign`].
    pub pos: PointPx,
    /// Byte span in `FramePools::text`.
    pub text: Span,
    /// Font and size.
    pub style: FontStyleRef,
    /// Text color.
    pub color: Rgba,
    /// Horizontal alignment.
    pub align: TextAlign,
    /// Optional background box.
    pub bg: Option<LabelBg>,
}

/// A filled rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectInst {
    /// Rectangle.
    pub rect: RectPx,
    /// Fill color.
    pub color: Rgba,
}

/// Dash pattern of a line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LineDash {
    /// Solid.
    #[default]
    Solid,
    /// Dashes.
    Dashed,
    /// Dots.
    Dotted,
}

/// Stroke parameters of a line or polyline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    /// Line width.
    pub width: Px,
    /// Color.
    pub color: Rgba,
    /// Dash pattern.
    pub dash: LineDash,
}

/// A straight line segment. Axis-aligned 1-px lines are placed on pixel centers
/// (`floor(x) + 0.5`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineInst {
    /// Start point.
    pub from: PointPx,
    /// End point.
    pub to: PointPx,
    /// Stroke.
    pub stroke: StrokeStyle,
}

/// A marker icon from the built-in set: the `ICON_*` constants
/// of [`crate::transform_helpers`], drawn with
/// [`crate::transform_helpers::icon_coverage`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconInst {
    /// Center.
    pub pos: PointPx,
    /// Width and height.
    pub size: Px,
    /// Icon number (`transform_helpers::ICON_*`).
    pub icon: u16,
    /// Tint color.
    pub color: Rgba,
}

/// Built-in series kinds.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeriesKind {
    /// Candles: body + wick, up/down colors.
    Candlestick,
    /// OHLC bars.
    OhlcBar,
    /// Heikin-Ashi candles.
    HeikinAshi,
    /// Line.
    Line,
    /// Line with gradient fill to the baseline.
    Area,
    /// Two-colored fill above/below a baseline.
    Baseline,
    /// Histogram.
    Histogram,
    /// One dot per point, `line_width` across (e.g. Parabolic SAR).
    Points,
    /// Fill between two single-value series: `SeriesDraw::series` is
    /// the upper one, `SeriesDraw::pair` the lower one; `fill_top` where
    /// upper ≥ lower, `fill_bottom` where it is below. Created with
    /// [`Chart::add_band`](crate::Chart::add_band), never with `add_series`.
    Band,
    /// Volume histogram of an OHLC series, colored by up/down.
    Volume,
    /// A plugin series; never in a [`SeriesDraw`].
    Custom,
    /// Kagi lines of a transform series, drawn by core as
    /// primitives; never in a [`SeriesDraw`].
    Kagi,
    /// Point & Figure columns of a transform series, drawn by core
    /// as primitives; never in a [`SeriesDraw`].
    PointFigure,
}

/// Index into [`Frame::styles`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SeriesStyleRef(pub u32);

/// Fully resolved colors of a series draw. The backend never decides colors; it
/// only picks `up` or `down` per bar using the rule in `transform_helpers`.
///
/// Value series use the fields as follows: `Line`/`Area`: `up` is the line
/// color; `Area` fills from `fill_top` at the top of the clip to `fill_bottom`
/// at its bottom. `Baseline`: `up`/`fill_top` above `base_value`,
/// `down`/`fill_bottom` below it. `Histogram`: `up` is the bar color (unless
/// the point has its own color), bars stand on `base_value`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeriesStyle {
    /// Body / bar color of up bars (`close >= open`).
    pub up: Rgba,
    /// Body / bar color of down bars.
    pub down: Rgba,
    /// Wick color of up bars.
    pub up_wick: Rgba,
    /// Wick color of down bars.
    pub down_wick: Rgba,
    /// Border color of up bodies.
    pub up_border: Rgba,
    /// Border color of down bodies.
    pub down_border: Rgba,
    /// Line width of line-like series.
    pub line_width: Px,
    /// Dash pattern of line-like series.
    pub line_dash: LineDash,
    /// Step line (horizontal, then vertical) instead of straight segments.
    pub step: bool,
    /// Upper fill color (see above).
    pub fill_top: Rgba,
    /// Lower fill color (see above).
    pub fill_bottom: Rgba,
    /// Base value of `Baseline` and `Histogram`, in price space.
    pub base_value: f64,
}

/// Index-to-x mapping of a [`SeriesDraw`]: `x = (j - base) * step + offset`, where
/// `j` is a logical index (level 0) or a bucket number (LOD level), and `x` is the
/// center of element `j`. `j - base` is a small integer, so the f32 math is exact
/// enough.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct XTransform {
    /// Index of the element at `offset`.
    pub base: i64,
    /// Distance between element centers in physical pixels.
    pub step: f32,
    /// x of the center of element `base`.
    pub offset: f32,
}

/// Value mapping of a price scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PriceMapping {
    /// `f(p) = p`.
    #[default]
    Linear,
    /// `f(p) = ln p`.
    Log,
}

/// Value-to-y mapping of a [`SeriesDraw`]: `y = a * (f(value) - f(view_ref)) + b`.
/// Renderers evaluate it per chunk in f32 as `(v_rel + bias) * a` with
/// `v_rel = f(value) - f(chunk_ref)`; use
/// `transform_helpers` instead of evaluating it by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct YTransform {
    /// Scale.
    pub a: f32,
    /// y of `view_ref`.
    pub b: f32,
    /// Reference value (in price space) the transform is relative to.
    pub view_ref: f64,
    /// Linear or logarithmic.
    pub mapping: PriceMapping,
}

/// A data-bound series draw: the backend reads the series data (of the given LOD
/// level) and transforms it itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesDraw {
    /// The series whose data is drawn (for `Volume`, the OHLC source series).
    pub series: crate::data::SeriesId,
    /// Data generation this draw was built for.
    pub generation: u64,
    /// Resolved colors.
    pub style: SeriesStyleRef,
    /// What to draw.
    pub kind: SeriesKind,
    /// LOD level (0 = original data).
    pub lod_level: u8,
    /// Elements to draw (logical indices or bucket numbers), one extra on each side.
    pub index_range: IdxSpan,
    /// Horizontal mapping.
    pub x_transform: XTransform,
    /// Vertical mapping.
    pub y_transform: YTransform,
    /// Clip rectangle.
    pub clip: RectPx,
    /// `Band`: the lower series (`series` is the upper one); otherwise `None`.
    pub pair: Option<crate::data::SeriesId>,
}

/// Native drawing of a custom series: emitted instead of the plugin's
/// primitives when the renderer declared its kind
/// ([`crate::render::Renderer::native_kinds`]).
///
/// Point `i` of the series is at x `(idx(i) - base) * step + offset`, where
/// `idx(i)` is its fractional logical index
/// ([`crate::chart::ChartDataView::custom_index`]) and the terms are those of
/// `x_transform`; prices map through `y_transform` like a [`SeriesDraw`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomDraw {
    /// The series.
    pub series: crate::data::SeriesId,
    /// Data generation of the series' change log (in point numbers).
    pub generation: u64,
    /// `CustomSeries::kind`.
    pub kind: &'static str,
    /// The points to draw (the series' own point numbers): the visible ones
    /// and one more on each side.
    pub points: IdxSpan,
    /// Logical index to x (`SeriesOptions::shift` included).
    pub x_transform: XTransform,
    /// Price to y.
    pub y_transform: YTransform,
    /// Resolved colors and line settings.
    pub style: SeriesStyleRef,
    /// Written by `CustomSeries::native_params` (e.g. a color-scale range).
    pub params: [f32; 8],
    /// Clip rectangle (the pane's plot).
    pub clip: RectPx,
}

/// One draw command. All variants are `Copy` and hold no heap data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawCmd {
    /// Intersects the clip with a rectangle until the matching `PopClip`.
    PushClip(RectPx),
    /// Restores the previous clip.
    PopClip,
    /// Data-bound series.
    Series(SeriesDraw),
    /// Filled rectangles: `FramePools::rects[span]`.
    Rects(Span),
    /// Line segments: `FramePools::lines[span]`.
    Lines(Span),
    /// A polyline through `FramePools::points[points]`.
    Polyline {
        /// Vertices.
        points: Span,
        /// Stroke.
        style: StrokeStyle,
    },
    /// A filled polygon with vertices `FramePools::points[points]`.
    Polygon {
        /// Vertices.
        points: Span,
        /// Fill color.
        fill: Rgba,
    },
    /// Text runs: `FramePools::text_runs[span]`.
    Text(Span),
    /// Icons: `FramePools::icons[span]`.
    Icon(Span),
    /// Native drawing of a custom series; only for kinds the renderer
    /// declared.
    Custom(CustomDraw),
}

/// Primitive pools of a frame.
#[derive(Clone, Debug, Default)]
pub struct FramePools {
    /// Rectangles.
    pub rects: Vec<RectInst>,
    /// Lines.
    pub lines: Vec<LineInst>,
    /// Polyline / polygon vertices.
    pub points: Vec<PointPx>,
    /// Text runs.
    pub text_runs: Vec<TextRun>,
    /// Characters of all text runs, concatenated.
    pub text: String,
    /// Icons.
    pub icons: Vec<IconInst>,
}

impl FramePools {
    fn clear(&mut self) {
        self.rects.clear();
        self.lines.clear();
        self.points.clear();
        self.text_runs.clear();
        self.text.clear();
        self.icons.clear();
    }

    /// The characters of `run`, or `""` if its span is out of range.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::frame::{FramePools, TextRun};
    /// use ferrochart_core::geom::Span;
    ///
    /// let mut pools = FramePools::default();
    /// pools.text.push_str("12.50");
    /// assert_eq!(pools.text_of(Span { start: 0, len: 2 }), "12");
    /// ```
    pub fn text_of(&self, span: Span) -> &str {
        self.text.get(span.range()).unwrap_or("")
    }
}

/// A layer of the frame.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    /// Content version; increases whenever the layer's content changes. Backends
    /// that cache a layer compare it with the revision they cached.
    pub revision: u64,
    /// Commands, in painting order.
    pub cmds: Vec<DrawCmd>,
}

/// Layer indices into [`Frame::layers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerId {
    /// Grid, watermark.
    Background = 0,
    /// Series, drawings, axes.
    Main = 1,
    /// Crosshair, hover, drawings being dragged.
    Overlay = 2,
}

/// The draw description of one frame.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{DrawCmd, Frame, LayerId, RectInst};
/// use ferrochart_core::geom::{RectPx, Rgba};
///
/// let mut frame = Frame::default();
/// let r = RectInst { rect: RectPx { x: 0.0, y: 0.0, w: 2.0, h: 2.0 }, color: Rgba::hex(0) };
/// frame.push_rect(LayerId::Main, r);
/// frame.push_rect(LayerId::Main, r); // merged into the same command
/// assert_eq!(frame.layers[1].cmds.len(), 1);
/// assert!(matches!(frame.layers[1].cmds[0], DrawCmd::Rects(s) if s.len == 2));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Size of the target in physical pixels.
    pub size_px: SizePx,
    /// Background color the target is cleared to.
    pub clear_color: Rgba,
    /// Background, main and overlay layers.
    pub layers: [Layer; 3],
    /// What changed since the previous frame.
    pub invalidation: Invalidation,
    /// Primitive pools.
    pub pools: FramePools,
    /// Resolved series styles, referenced by [`SeriesStyleRef`].
    pub styles: Vec<SeriesStyle>,
}

fn span_of(start: usize, len: usize) -> Span {
    Span {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        len: u32::try_from(len).unwrap_or(0),
    }
}

impl Frame {
    /// Starts a new frame: clears all pools and commands, keeping their
    /// capacity, and sets the target size, clear color and invalidation.
    ///
    /// ```
    /// use ferrochart_core::frame::{Frame, LayerId, RectInst};
    /// use ferrochart_core::geom::{RectPx, Rgba, SizePx};
    /// use ferrochart_core::frame::Invalidation;
    ///
    /// let mut frame = Frame::default();
    /// frame.reserve();
    /// let r = RectInst { rect: RectPx { x: 0.0, y: 0.0, w: 2.0, h: 2.0 }, color: Rgba::hex(0) };
    /// frame.push_rect(LayerId::Main, r);
    /// frame.begin(SizePx { w: 100.0, h: 50.0 }, Rgba::hex(0xFF), Invalidation::Full);
    /// assert!(frame.pools.rects.is_empty() && frame.layers[1].cmds.is_empty());
    /// ```
    pub fn begin(&mut self, size: SizePx, clear: Rgba, invalidation: Invalidation) {
        self.size_px = size;
        self.clear_color = clear;
        self.invalidation = invalidation;
        self.pools.clear();
        self.styles.clear();
        for layer in &mut self.layers {
            layer.cmds.clear();
        }
    }

    /// Pre-allocates pools so that typical frames do not allocate.
    pub fn reserve(&mut self) {
        let _ = self.pools.rects.try_reserve(256);
        let _ = self.pools.lines.try_reserve(512);
        let _ = self.pools.text_runs.try_reserve(512);
        let _ = self.pools.text.try_reserve(8192);
        let _ = self.pools.icons.try_reserve(256);
        let _ = self.styles.try_reserve(16);
        for layer in &mut self.layers {
            let _ = layer.cmds.try_reserve(64);
        }
    }

    /// Appends a command to a layer.
    pub fn push_cmd(&mut self, layer: LayerId, cmd: DrawCmd) {
        self.layers[layer as usize].cmds.push(cmd);
    }

    /// Adds a resolved style and returns its reference.
    pub fn push_style(&mut self, style: SeriesStyle) -> SeriesStyleRef {
        self.styles.push(style);
        SeriesStyleRef(u32::try_from(self.styles.len() - 1).unwrap_or(u32::MAX))
    }

    /// Adds a rectangle, extending the layer's last `Rects` command if it ends at
    /// the end of the pool.
    pub fn push_rect(&mut self, layer: LayerId, rect: RectInst) {
        let start = self.pools.rects.len();
        self.pools.rects.push(rect);
        let cmds = &mut self.layers[layer as usize].cmds;
        if let Some(DrawCmd::Rects(s)) = cmds.last_mut()
            && s.range().end == start
        {
            s.len += 1;
            return;
        }
        cmds.push(DrawCmd::Rects(span_of(start, 1)));
    }

    /// Adds a line, merging with the layer's last `Lines` command when possible.
    pub fn push_line(&mut self, layer: LayerId, line: LineInst) {
        let start = self.pools.lines.len();
        self.pools.lines.push(line);
        let cmds = &mut self.layers[layer as usize].cmds;
        if let Some(DrawCmd::Lines(s)) = cmds.last_mut()
            && s.range().end == start
        {
            s.len += 1;
            return;
        }
        cmds.push(DrawCmd::Lines(span_of(start, 1)));
    }

    /// Adds a text run whose characters are `pools.text[text]` (already written),
    /// merging with the layer's last `Text` command when possible.
    pub fn push_text(&mut self, layer: LayerId, run: TextRun) {
        let start = self.pools.text_runs.len();
        self.pools.text_runs.push(run);
        let cmds = &mut self.layers[layer as usize].cmds;
        if let Some(DrawCmd::Text(s)) = cmds.last_mut()
            && s.range().end == start
        {
            s.len += 1;
            return;
        }
        cmds.push(DrawCmd::Text(span_of(start, 1)));
    }

    /// Adds an icon, merging with the layer's last `Icon` command when possible.
    pub fn push_icon(&mut self, layer: LayerId, icon: IconInst) {
        let start = self.pools.icons.len();
        self.pools.icons.push(icon);
        let cmds = &mut self.layers[layer as usize].cmds;
        if let Some(DrawCmd::Icon(s)) = cmds.last_mut()
            && s.range().end == start
        {
            s.len += 1;
            return;
        }
        cmds.push(DrawCmd::Icon(span_of(start, 1)));
    }

    /// Writes `s` into the text pool and returns its span.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::frame::Frame;
    ///
    /// let mut frame = Frame::default();
    /// let span = frame.push_str("2025");
    /// assert_eq!(frame.pools.text_of(span), "2025");
    /// ```
    pub fn push_str(&mut self, s: &str) -> Span {
        let start = self.pools.text.len();
        self.pools.text.push_str(s);
        span_of(start, s.len())
    }
}

/// Gradual display of the data: the data layer is clipped to the
/// left `progress` of the plot area while grid and axes stay whole. Used by
/// [`Chart::set_reveal`](crate::chart::Chart::set_reveal) and by the plot
/// crate; the host moves `progress` over time.
///
/// ```
/// use ferrochart_core::frame::Reveal;
/// use ferrochart_core::geom::RectPx;
///
/// let r = Reveal { progress: 0.25, pen: None };
/// let plot = RectPx { x: 10.0, y: 0.0, w: 200.0, h: 100.0 };
/// assert_eq!(r.clip(plot), RectPx { x: 10.0, y: 0.0, w: 50.0, h: 100.0 });
/// assert!(!r.is_done());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reveal {
    /// Share of the plot width shown, from the left: 0..=1 (NaN counts as 0).
    pub progress: f32,
    /// Color of the line at the cut-off, if any.
    pub pen: Option<Rgba>,
}

impl Reveal {
    /// `progress` clamped to 0..=1, NaN as 0.
    pub fn amount(self) -> f32 {
        if self.progress.is_nan() {
            0.0
        } else {
            self.progress.clamp(0.0, 1.0)
        }
    }

    /// Whether everything is shown (progress 1): drawn as without a reveal.
    pub fn is_done(self) -> bool {
        self.amount() >= 1.0
    }

    /// The part of `plot` the data is drawn in.
    pub fn clip(self, plot: RectPx) -> RectPx {
        RectPx {
            w: (plot.w * self.amount()).round(),
            ..plot
        }
    }

    /// The pen line at the cut-off of `plot` (with two wider, fainter lines
    /// as a glow), clipped to `plot`, on `layer`; nothing when there is no
    /// pen or progress is 0 or 1.
    pub fn push_pen(self, frame: &mut Frame, layer: LayerId, plot: RectPx, dpr: f32) {
        let Some(color) = self.pen else {
            return;
        };
        let a = self.amount();
        if a <= 0.0 || a >= 1.0 || plot.w <= 0.0 || plot.h <= 0.0 {
            return;
        }
        let x = plot.x + self.clip(plot).w;
        frame.push_cmd(layer, DrawCmd::PushClip(plot));
        for (width, alpha) in [(14.0, 0.10), (5.0, 0.28), (1.5, 1.0)] {
            let alpha = (f32::from(color.a) * alpha).round() as u8;
            frame.push_line(
                layer,
                LineInst {
                    from: PointPx { x, y: plot.y },
                    to: PointPx {
                        x,
                        y: plot.bottom(),
                    },
                    stroke: StrokeStyle {
                        width: Px((width * dpr).max(1.0)),
                        color: color.with_alpha(alpha),
                        dash: LineDash::Solid,
                    },
                },
            );
        }
        frame.push_cmd(layer, DrawCmd::PopClip);
    }
}
