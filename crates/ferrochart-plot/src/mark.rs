//! Marks: the graphical elements of a plot (lines, areas, scatter, bars,
//! waterfalls, cells), their options, data and validation, plus rules and
//! labels.
//!

use ferrochart_core::error::{Error, Result};
use ferrochart_core::frame::{LineDash, TextAlign};
use ferrochart_core::geom::{Css, Rgba};
use ferrochart_core::transform_helpers::ICON_CIRCLE;

use crate::axis::{AxisFormat, AxisId};
use crate::color::{ColorScale, ScaleDomain, ScaleKind, resolve_domain};
use crate::index::GridIndex;

/// Identifier of a mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkId(pub u32);

/// Identifier of a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleId(pub u32);

/// Identifier of a label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

/// Kinds of marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// Points joined in data order.
    Line,
    /// The area between the line and `y0` (or the base value).
    Area,
    /// A symbol per point.
    Scatter,
    /// Bars from `y0` (or the base value) to `y`.
    Bars,
    /// Running-total bars on a category axis.
    Waterfall,
    /// Colored cells of a grid (heatmaps).
    Cells,
}

/// Direction of bars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Bars grow along y; categories (or values) are on the x axis.
    #[default]
    Vertical,
    /// Bars grow along x; categories are on the y axis.
    Horizontal,
}

/// The role of a waterfall item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WaterfallStep {
    /// Adds the value to the running total.
    #[default]
    Delta,
    /// A bar from 0 to the running total; the total is unchanged.
    Subtotal,
    /// Like `Subtotal`, conventionally last.
    Total,
}

/// Data of a mark: columns of the same length.
///
/// ```
/// use ferrochart_plot::MarkData;
///
/// let d = MarkData::xy(vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 15.0]);
/// assert_eq!(d.len(), 3);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkData {
    /// Numbers, UTC milliseconds or category numbers; values for horizontal bars.
    pub x: Vec<f64>,
    /// Values (NaN: a gap); category numbers for horizontal bars.
    pub y: Vec<f64>,
    /// Lower edge of areas, start of bars (`None`: the base value).
    pub y0: Option<Vec<f64>>,
    /// Values for cells, and for coloring bars or scatter by a color scale.
    pub value: Option<Vec<f64>>,
    /// Scatter diameters in CSS px.
    pub size: Option<Vec<f32>>,
    /// Per-point colors `0xRRGGBBAA`; 0 uses the mark color.
    pub color: Option<Vec<u32>>,
    /// Waterfall roles (`None`: all `Delta`).
    pub step: Option<Vec<WaterfallStep>>,
    /// Per-point texts: scatter labels; replaces the value text of bars and cells.
    pub label: Option<Vec<String>>,
}

impl MarkData {
    /// Data with `x` and `y` only.
    pub fn xy(x: Vec<f64>, y: Vec<f64>) -> Self {
        Self {
            x,
            y,
            ..Self::default()
        }
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// No points.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
}

/// One point for [`Plot::update_last`](crate::Plot::update_last) and
/// streaming appends. Unused fields are ignored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkPoint {
    /// x.
    pub x: f64,
    /// y.
    pub y: f64,
    /// Lower edge (NaN: the base value).
    pub y0: f64,
    /// Color-scale value.
    pub value: f64,
    /// Scatter diameter in CSS px (NaN: the mark's point size).
    pub size: f32,
    /// Color `0xRRGGBBAA` (0: the mark color).
    pub color: u32,
}

impl MarkPoint {
    /// A point with `x` and `y` only.
    pub fn xy(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            y0: f64::NAN,
            value: f64::NAN,
            size: f32::NAN,
            color: 0,
        }
    }
}

/// Options of a mark.
#[allow(clippy::struct_excessive_bools)] // independent switches
#[derive(Clone, Debug, PartialEq)]
pub struct MarkOptions {
    /// The vertical axis the mark uses: `Left` or `Right`.
    pub y_axis: AxisId,
    /// Bars and waterfalls: vertical or horizontal.
    pub orientation: Orientation,
    /// Main color (`None`: the next color of the plot's palette).
    pub color: Option<Rgba>,
    /// Areas below the base value; decreasing waterfall items.
    pub down_color: Option<Rgba>,
    /// Waterfall subtotals and totals.
    pub total_color: Option<Rgba>,
    /// Opacity of area fills, 0 to 1.
    pub fill_alpha: f32,
    /// Line width.
    pub line_width: Css,
    /// Line dash pattern.
    pub line_style: LineDash,
    /// Lines and areas as steps (horizontal to the next x, then vertical).
    pub step_line: bool,
    /// Line: a symbol at each point (0: none). Scatter: the diameter when
    /// there is no size column.
    pub point_size: Css,
    /// Symbol shape (`transform_helpers::ICON_*`).
    pub icon: u16,
    /// Areas and bars start at this value when there is no `y0`.
    pub base_value: f64,
    /// Bars with the same group share a category slot side by side.
    pub group: Option<u32>,
    /// Bars or areas with the same stack add up (positive and negative apart).
    pub stack: Option<u32>,
    /// Fraction of the slot taken by bars and cells (`NaN`: 0.8 for bars,
    /// 1.0 for cells).
    pub band: f32,
    /// Colors by value (cells always use one; the default scale if `None`).
    pub color_scale: Option<ColorScale>,
    /// Bars, waterfalls and cells: write the value on the element when it fits.
    pub show_values: bool,
    /// Format of value texts.
    pub value_format: AxisFormat,
    /// Cells on numeric axes: width and height in data units (`None`: the
    /// smallest gap between distinct x and between distinct y).
    pub cell_size: Option<(f64, f64)>,
    /// Hit by hover and click.
    pub hoverable: bool,
    /// Drawn.
    pub visible: bool,
}

impl Default for MarkOptions {
    fn default() -> Self {
        Self {
            y_axis: AxisId::Right,
            orientation: Orientation::Vertical,
            color: None,
            down_color: None,
            total_color: None,
            fill_alpha: 0.25,
            line_width: Css(2.0),
            line_style: LineDash::Solid,
            step_line: false,
            point_size: Css(0.0),
            icon: ICON_CIRCLE,
            base_value: 0.0,
            group: None,
            stack: None,
            band: f32::NAN,
            color_scale: None,
            show_values: false,
            value_format: AxisFormat::Auto,
            cell_size: None,
            hoverable: true,
            visible: true,
        }
    }
}

/// A line across the plot at a value, or a shaded band between two values.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleOptions {
    /// The axis the value is on: `X` gives a vertical rule.
    pub axis: AxisId,
    /// Value.
    pub value: f64,
    /// Other edge of a band (`None`: a line).
    pub to: Option<f64>,
    /// Color (`None`: the theme's crosshair color; bands at 12% opacity).
    pub color: Option<Rgba>,
    /// Line width.
    pub line_width: Css,
    /// Dash pattern.
    pub line_style: LineDash,
    /// Text on the axis at the value.
    pub label: Option<String>,
    /// The label sits on a box of the rule color (otherwise it is written in
    /// the rule color).
    pub label_bg: bool,
}

impl Default for RuleOptions {
    fn default() -> Self {
        Self {
            axis: AxisId::Right,
            value: 0.0,
            to: None,
            color: None,
            line_width: Css(1.0),
            line_style: LineDash::Dashed,
            label: None,
            label_bg: true,
        }
    }
}

/// A text or symbol at data coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelOptions {
    /// x.
    pub x: f64,
    /// y.
    pub y: f64,
    /// The vertical axis of `y`.
    pub y_axis: AxisId,
    /// Text (may be empty with an icon).
    pub text: String,
    /// Symbol drawn at the point.
    pub icon: Option<u16>,
    /// Color (`None`: the theme's text color).
    pub color: Option<Rgba>,
    /// Alignment of the text relative to the point.
    pub align: TextAlign,
    /// Offset of the text from the point, right and down.
    pub offset: (Css, Css),
}

impl Default for LabelOptions {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            y_axis: AxisId::Right,
            text: String::new(),
            icon: None,
            color: None,
            align: TextAlign::Left,
            offset: (Css(6.0), Css(0.0)),
        }
    }
}

/// Order, extents and running total of a run of consecutive points; the
/// statistics of two runs combine with [`Stats::then`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stats {
    n: usize,
    /// x never decreases within the run (a NaN x breaks it).
    sorted: bool,
    first_x: Option<f64>,
    last_x: f64,
    /// Largest finite x and y, plus one (0 without any).
    cat_x: f64,
    cat_y: f64,
    x: Option<(f64, f64)>,
    y: Option<(f64, f64)>,
    /// Where areas and bars start.
    lower: Option<(f64, f64)>,
    /// Waterfall: the sum of the deltas, and the extent of the running total
    /// (from 0 at the start of the run).
    total: f64,
    run: (f64, f64),
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            n: 0,
            sorted: true,
            first_x: None,
            last_x: f64::NAN,
            cat_x: 0.0,
            cat_y: 0.0,
            x: None,
            y: None,
            lower: None,
            total: 0.0,
            run: (0.0, 0.0),
        }
    }
}

impl Stats {
    /// The statistics of this run followed by `b`.
    fn then(&self, b: &Self) -> Self {
        let joined = match b.first_x {
            Some(f) if self.n > 0 => self.last_x <= f,
            _ => true,
        };
        Self {
            n: self.n + b.n,
            sorted: self.sorted && b.sorted && joined,
            first_x: self.first_x.or(b.first_x),
            last_x: if b.n > 0 { b.last_x } else { self.last_x },
            cat_x: self.cat_x.max(b.cat_x),
            cat_y: self.cat_y.max(b.cat_y),
            x: merge(self.x, b.x),
            y: merge(self.y, b.y),
            lower: merge(self.lower, b.lower),
            total: self.total + b.total,
            run: (
                self.run.0.min(self.total + b.run.0),
                self.run.1.max(self.total + b.run.1),
            ),
        }
    }
}

fn finite(v: f64) -> Option<(f64, f64)> {
    v.is_finite().then_some((v, v))
}

/// Entries of a mark's color lookup table (positions `k / 1024`, so the
/// quarter stops of the default scales are exact).
const LUT_LEN: usize = 1025;

/// A mark and its data.

#[derive(Clone, Debug)]
pub(crate) struct MarkEntry {
    pub id: MarkId,
    pub kind: MarkKind,
    pub opts: MarkOptions,
    pub data: MarkData,
    /// Palette color when `opts.color` is `None`.
    pub auto_color: Rgba,
    /// `x` never decreases.
    pub x_sorted: bool,
    /// Extent of the mark along x and y (bars: including their start).
    pub ext_x: Option<(f64, f64)>,
    pub ext_y: Option<(f64, f64)>,
    /// Extent of the running sums along the value axis when the mark is
    /// stacked (set by the plot).
    pub stack_ext: Option<(f64, f64)>,
    /// Statistics of all points but the last (`update_last` combines them
    /// with the new last point).
    head: Stats,
    /// Resolved domain of the color scale (`None`: no values).
    pub domain: Option<(f64, f64)>,
    /// The color scale sampled at `LUT_LEN` even positions (empty without a
    /// domain): coloring a point is a lookup, not an interpolation.
    pub lut: Vec<Rgba>,
    /// Cell width and height in data units (cells on numeric axes).
    pub cell: (f64, f64),
    /// Hit-test index of all points but the last (scatter, cells, lines and
    /// areas with unsorted x); the last point is checked on its own so
    /// `update_last` leaves the index valid.
    pub grid: Option<GridIndex>,
    /// Largest finite value of the size column (CSS px), as of the last
    /// `refresh_full`.
    pub max_size: f32,
    /// Largest category number used along x and y, plus one.
    pub cat_x: usize,
    pub cat_y: usize,
}

impl MarkEntry {
    pub fn new(id: MarkId, kind: MarkKind, opts: MarkOptions, auto_color: Rgba) -> Self {
        Self {
            id,
            kind,
            opts,
            data: MarkData::default(),
            auto_color,
            x_sorted: true,
            ext_x: None,
            ext_y: None,
            stack_ext: None,
            head: Stats::default(),
            domain: None,
            lut: Vec::new(),
            cell: (1.0, 1.0),
            grid: None,
            max_size: 0.0,
            cat_x: 0,
            cat_y: 0,
        }
    }

    pub fn color(&self) -> Rgba {
        self.opts.color.unwrap_or(self.auto_color)
    }

    /// The color of point `i`: its color column, its value on the color
    /// scale, or the mark color.
    pub fn point_color(&self, i: usize) -> Rgba {
        if let Some(&c) = self.data.color.as_ref().and_then(|c| c.get(i))
            && c != 0
        {
            return Rgba::from_u32(c);
        }
        self.data
            .value
            .as_ref()
            .and_then(|c| c.get(i))
            .and_then(|&v| self.value_color(v))
            .unwrap_or_else(|| self.color())
    }

    /// Color of value `v` on the mark's color scale (`None` without values
    /// or for a non-finite `v`). Within one 8-bit step of
    /// [`ColorScale::color_at`].
    pub fn value_color(&self, v: f64) -> Option<Rgba> {
        let domain = self.domain?;
        if !v.is_finite() {
            return None;
        }
        let kind = self
            .opts
            .color_scale
            .as_ref()
            .map_or(ScaleKind::Sequential, |s| s.kind);
        let t = kind.position(v, domain).clamp(0.0, 1.0);
        let k = (t * (LUT_LEN - 1) as f32).round() as usize;
        self.lut.get(k).copied()
    }

    /// Whether the mark's bars grow along x.
    pub fn horizontal(&self) -> bool {
        self.opts.orientation == Orientation::Horizontal
            && matches!(self.kind, MarkKind::Bars | MarkKind::Waterfall)
    }

    /// Whether the mark's values grow from a base (bars, waterfalls, areas):
    /// the automatic range keeps zero on the axis edge.
    pub fn anchored(&self) -> bool {
        matches!(
            self.kind,
            MarkKind::Bars | MarkKind::Waterfall | MarkKind::Area
        )
    }

    /// Band of bars or cells.
    pub fn band(&self) -> f64 {
        let b = self.opts.band;
        if b.is_finite() && b > 0.0 {
            f64::from(b.min(1.0))
        } else if self.kind == MarkKind::Cells {
            1.0
        } else {
            0.8
        }
    }

    /// Recomputes everything derived from data and options, including the
    /// cell size and the color-scale domain (allocates; not for hot paths).
    pub fn refresh_full(&mut self) {
        // Only bars (along their categories), waterfalls and cells use it.
        let (along_x, along_y) = match self.kind {
            MarkKind::Cells => (true, true),
            MarkKind::Bars | MarkKind::Waterfall => (!self.horizontal(), self.horizontal()),
            MarkKind::Line | MarkKind::Scatter | MarkKind::Area => (false, false),
        };
        let gap = |on: bool, col: &[f64]| if on { min_gap(col) } else { 1.0 };
        self.cell = self
            .opts
            .cell_size
            .unwrap_or_else(|| (gap(along_x, &self.data.x), gap(along_y, &self.data.y)));
        let domain = self
            .opts
            .color_scale
            .as_ref()
            .map_or(ScaleDomain::Auto { quantile: 0.01 }, |s| s.domain);
        self.domain = resolve_domain(domain, self.data.value.as_deref().unwrap_or(&[]));
        self.lut.clear();
        if self.domain.is_some() {
            let default;
            let scale = if let Some(s) = &self.opts.color_scale {
                s
            } else {
                default = ColorScale::sequential();
                &default
            };
            let last = (LUT_LEN - 1) as f32;
            self.lut
                .extend((0..LUT_LEN).map(|k| scale.color_at(k as f32 / last)));
        }
        self.refresh();
        let indexed = match self.kind {
            MarkKind::Scatter | MarkKind::Cells => true,
            MarkKind::Line | MarkKind::Area => !self.x_sorted,
            MarkKind::Bars | MarkKind::Waterfall => false,
        };
        self.max_size = self.data.size.as_ref().map_or(0.0, |s| {
            s.iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(0.0, f32::max)
        });
        let n = self.data.len().saturating_sub(1);
        self.grid = if indexed {
            GridIndex::build(&self.data.x, &self.data.y, n)
        } else {
            None
        };
    }

    /// Recomputes sortedness, extents and category counts in O(n) without
    /// allocating; the cell size and color domain are kept.
    pub fn refresh(&mut self) {
        let n = self.data.len();
        self.head = self.stats(0..n.saturating_sub(1));
        self.refresh_last();
    }

    /// As [`refresh`](Self::refresh) after only the last point changed: O(1)
    /// (the statistics of the other points are kept).
    pub fn refresh_last(&mut self) {
        let n = self.data.len();
        let s = self.head.then(&self.stats(n.saturating_sub(1)..n));
        self.x_sorted = s.sorted;
        self.cat_x = s.cat_x as usize;
        self.cat_y = s.cat_y as usize;
        let hz = self.horizontal();
        let (vals, cats) = if hz { (s.x, s.y) } else { (s.y, s.x) };
        let gap = if hz { self.cell.1 } else { self.cell.0 };
        let slots = cats.map(|(a, b)| (a - gap / 2.0, b + gap / 2.0));
        let along = |v, c| if hz { (v, c) } else { (c, v) };
        (self.ext_x, self.ext_y) = match self.kind {
            MarkKind::Line | MarkKind::Scatter => (s.x, s.y),
            MarkKind::Area => (s.x, merge(s.y, s.lower)),
            // Values, and where the bars start.
            MarkKind::Bars => along(merge(vals, s.lower), slots),
            MarkKind::Waterfall => along((s.n > 0).then_some(s.run), slots),
            MarkKind::Cells => {
                let (w, h) = self.cell_size();
                (
                    s.x.map(|(a, b)| (a - w / 2.0, b + w / 2.0)),
                    s.y.map(|(a, b)| (a - h / 2.0, b + h / 2.0)),
                )
            }
        };
    }

    /// Statistics of the points in `range`.
    fn stats(&self, range: core::ops::Range<usize>) -> Stats {
        let d = &self.data;
        let base = self.opts.base_value;
        let lower = |i: usize| {
            d.y0.as_ref().map_or(base, |c| {
                c.get(i).copied().filter(|v| !v.is_nan()).unwrap_or(base)
            })
        };
        let vals = if self.horizontal() { &d.x } else { &d.y };
        let mut s = Stats {
            n: range.len(),
            ..Stats::default()
        };
        let mut prev = f64::NEG_INFINITY;
        for i in range {
            let (x, y) = (d.x[i], d.y[i]);
            if s.first_x.is_none() {
                s.first_x = Some(x);
            } else if x.is_nan() || prev.is_nan() || x < prev {
                s.sorted = false;
            }
            prev = x;
            s.last_x = x;
            if x.is_finite() {
                s.cat_x = s.cat_x.max(x + 1.0);
                s.x = merge(s.x, Some((x, x)));
            }
            if y.is_finite() {
                s.cat_y = s.cat_y.max(y + 1.0);
                s.y = merge(s.y, Some((y, y)));
            }
            match self.kind {
                MarkKind::Area => s.lower = merge(s.lower, finite(lower(i))),
                MarkKind::Bars if vals[i].is_finite() => {
                    s.lower = merge(s.lower, finite(lower(i)));
                }
                MarkKind::Waterfall => {
                    let step = d
                        .step
                        .as_ref()
                        .and_then(|s| s.get(i))
                        .copied()
                        .unwrap_or_default();
                    if step == WaterfallStep::Delta && vals[i].is_finite() {
                        s.total += vals[i];
                    }
                    s.run = (s.run.0.min(s.total), s.run.1.max(s.total));
                }
                _ => {}
            }
        }
        s
    }

    /// Cell width and height in data units on numeric axes.
    pub fn cell_size(&self) -> (f64, f64) {
        self.cell
    }
}

/// Smallest positive gap between distinct values (1 if there is none).
/// O(n log n); called when data or options change, not per frame.
fn min_gap(col: &[f64]) -> f64 {
    let mut v: Vec<f64> = col.iter().copied().filter(|x| x.is_finite()).collect();
    if !v.windows(2).all(|w| w[0] <= w[1]) {
        v.sort_unstable_by(f64::total_cmp);
    }
    let gap = v
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0.0)
        .fold(f64::INFINITY, f64::min);
    if gap.is_finite() { gap } else { 1.0 }
}

pub(crate) fn merge(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => Some((a0.min(b0), a1.max(b1))),
        (x, None) | (None, x) => x,
    }
}

/// Checks the columns of `d` for a mark of `kind`.
pub(crate) fn validate(kind: MarkKind, d: &MarkData) -> Result<()> {
    let n = d.x.len();
    let same = |len: Option<usize>, name: &'static str| match len {
        Some(l) if l != n => Err(Error::InvalidArgument(name)),
        _ => Ok(()),
    };
    if d.y.len() != n {
        return Err(Error::InvalidArgument("x and y must have the same length"));
    }
    same(d.y0.as_ref().map(Vec::len), "y0 must have the length of x")?;
    same(
        d.value.as_ref().map(Vec::len),
        "value must have the length of x",
    )?;
    same(
        d.size.as_ref().map(Vec::len),
        "size must have the length of x",
    )?;
    same(
        d.color.as_ref().map(Vec::len),
        "color must have the length of x",
    )?;
    same(
        d.step.as_ref().map(Vec::len),
        "step must have the length of x",
    )?;
    same(
        d.label.as_ref().map(Vec::len),
        "label must have the length of x",
    )?;
    if kind != MarkKind::Bars && kind != MarkKind::Waterfall && d.x.iter().any(|v| !v.is_finite()) {
        return Err(Error::InvalidArgument("x must be finite"));
    }
    Ok(())
}

/// Checks that the category column of a mark holds category numbers.
pub(crate) fn validate_categories(col: &[f64]) -> Result<()> {
    if col
        .iter()
        .any(|&v| !(v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v < 1e9))
    {
        return Err(Error::InvalidArgument(
            "category numbers must be whole numbers from 0",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Changing the last point and combining the kept statistics gives
        /// what a full rescan gives.
        #[test]
        fn refresh_last_matches_refresh(
            kind in 0_usize..6,
            horizontal in any::<bool>(),
            pts in proptest::collection::vec((prop_oneof![9 => -50.0_f64..50.0, 1 => Just(f64::NAN)], prop_oneof![9 => -50.0_f64..50.0, 1 => Just(f64::NAN)], prop_oneof![9 => -5.0_f64..5.0, 1 => Just(f64::NAN)]), 1..40),
            last in (-60.0_f64..60.0, prop_oneof![9 => -60.0_f64..60.0, 1 => Just(f64::NAN)], -6.0_f64..6.0),
            sorted in any::<bool>(),
        ) {
            let kind = [MarkKind::Line, MarkKind::Area, MarkKind::Scatter, MarkKind::Bars, MarkKind::Waterfall, MarkKind::Cells][kind];
            let opts = MarkOptions {
                orientation: if horizontal { Orientation::Horizontal } else { Orientation::Vertical },
                ..MarkOptions::default()
            };
            let mut x: Vec<f64> = pts.iter().map(|p| p.0).collect();
            if sorted {
                x = (0..x.len()).map(|i| i as f64).collect();
            }
            let mut d = MarkData::xy(x, pts.iter().map(|p| p.1).collect());
            d.y0 = Some(pts.iter().map(|p| p.2).collect());
            d.step = Some((0..pts.len()).map(|i| if i % 7 == 6 { WaterfallStep::Subtotal } else { WaterfallStep::Delta }).collect());
            let mut a = MarkEntry::new(MarkId(1), kind, opts, Rgba::hex(0));
            a.data = d;
            a.refresh_full();
            let i = a.data.len() - 1;
            a.data.x[i] = last.0;
            a.data.y[i] = last.1;
            if let Some(c) = a.data.y0.as_mut() {
                c[i] = last.2;
            }
            let mut b = a.clone();
            a.refresh_last();
            b.refresh();
            let key = |m: &MarkEntry| format!("{:?}", (m.x_sorted, m.cat_x, m.cat_y, m.ext_x, m.ext_y));
            prop_assert_eq!(key(&a), key(&b));
        }
    }

    #[test]
    fn validation() {
        assert!(validate(MarkKind::Line, &MarkData::xy(vec![1.0], vec![1.0, 2.0])).is_err());
        assert!(validate(MarkKind::Line, &MarkData::xy(vec![f64::NAN], vec![1.0])).is_err());
        assert!(validate(MarkKind::Line, &MarkData::xy(vec![1.0], vec![f64::NAN])).is_ok());
        let mut d = MarkData::xy(vec![0.0, 1.0], vec![1.0, 2.0]);
        d.color = Some(vec![0]);
        assert!(validate(MarkKind::Scatter, &d).is_err());
        assert!(validate_categories(&[0.0, 1.0, 2.0]).is_ok());
        assert!(validate_categories(&[0.5]).is_err());
        assert!(validate_categories(&[-1.0]).is_err());
    }

    #[test]
    fn extents_include_bar_starts_and_waterfall_totals() {
        let mut bars = MarkEntry::new(
            MarkId(1),
            MarkKind::Bars,
            MarkOptions::default(),
            Rgba::hex(0),
        );
        bars.data = MarkData::xy(vec![0.0, 1.0, 2.0], vec![5.0, 7.0, 3.0]);
        bars.refresh();
        assert_eq!(bars.ext_y, Some((0.0, 7.0)));
        assert_eq!(bars.ext_x, Some((-0.5, 2.5)));
        assert_eq!(bars.cat_x, 3);
        let mut wf = MarkEntry::new(
            MarkId(2),
            MarkKind::Waterfall,
            MarkOptions::default(),
            Rgba::hex(0),
        );
        wf.data = MarkData::xy(vec![0.0, 1.0, 2.0, 3.0], vec![10.0, -4.0, -8.0, f64::NAN]);
        wf.data.step = Some(vec![
            WaterfallStep::Delta,
            WaterfallStep::Delta,
            WaterfallStep::Delta,
            WaterfallStep::Total,
        ]);
        wf.refresh();
        assert_eq!(wf.ext_y, Some((-2.0, 10.0)));
    }

    #[test]
    fn cell_size_from_gaps() {
        let mut c = MarkEntry::new(
            MarkId(1),
            MarkKind::Cells,
            MarkOptions::default(),
            Rgba::hex(0),
        );
        c.data = MarkData::xy(vec![0.0, 2.0, 4.0, 0.0], vec![1.0, 1.0, 1.0, 1.5]);
        c.refresh_full();
        assert_eq!(c.cell_size(), (2.0, 0.5));
        assert_eq!(c.ext_x, Some((-1.0, 5.0)));
    }
}
