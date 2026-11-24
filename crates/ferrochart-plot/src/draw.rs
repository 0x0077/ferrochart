//! Marks, rules and labels turned into immediate frame primitives.
//!

use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::{
    FontStyleRef, Frame, IconInst, LabelBg, LayerId, StrokeStyle, TextAlign,
};
use ferrochart_core::geom::{Css, PointPx, Px, RectPx, Rgba};
use ferrochart_core::text::{TextMeasurer, text_width};
use ferrochart_core::theme::Theme;

use crate::axis::{Axis, AxisId, PxMap};
use crate::color::contrast;
use crate::mark::{LabelOptions, MarkEntry, MarkKind, RuleOptions};
use crate::shapes;

/// Pixel coordinates are clamped to ±`FAR` before the f32 conversion: far
/// outside any target, small enough for the backends' arithmetic.
const FAR: f32 = 1.0e7;

/// Scatter diameter when neither a size column nor `point_size` is set.
const DEFAULT_POINT_CSS: f32 = 6.0;

/// Scatter labels stop after this many (overlap checks are linear).
const MAX_SCATTER_LABELS: usize = 2000;

/// The axes and plot area of the last layout: what hit testing reads.
#[derive(Clone, Copy)]
pub(crate) struct Geometry<'a> {
    pub x: &'a Axis,
    pub left: &'a Axis,
    pub right: &'a Axis,
    pub plot: RectPx,
    pub dpr: f32,
    pub theme: &'a Theme,
    /// Extra hit radius for a finger, CSS px.
    pub touch_slop: f32,
}

/// Everything mark drawing reads: the geometry plus text and colors.
pub(crate) struct Ctx<'a> {
    pub geo: Geometry<'a>,
    pub font: FontStyleRef,
    pub line_h: f32,
    pub pad: f32,
    pub text: &'a dyn TextMeasurer,
}

impl<'a> core::ops::Deref for Ctx<'a> {
    type Target = Geometry<'a>;

    fn deref(&self) -> &Geometry<'a> {
        &self.geo
    }
}

impl Geometry<'_> {
    pub fn axis(&self, id: AxisId) -> &Axis {
        match id {
            AxisId::X => self.x,
            AxisId::Left => self.left,
            AxisId::Right => self.right,
        }
    }

    pub fn px(&self, v: Css) -> f32 {
        v.to_px(self.dpr).0
    }
}

/// Buffers reused from frame to frame (no allocation once warm).
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    /// Vertices of the polyline being built.
    pub pts: Vec<PointPx>,
    /// Rectangles of the scatter labels written so far.
    pub placed: Vec<RectPx>,
    /// Running sums of stacks, positive and negative.
    pub pos: Vec<f64>,
    pub neg: Vec<f64>,
    /// Area vertices: x, upper and lower edge (NaN: a gap).
    pub area: Vec<[f32; 3]>,
}

/// `v` as an f32 pixel coordinate (NaN stays NaN).
pub(crate) fn clamp_px(v: f64) -> f32 {
    (v as f32).clamp(-FAR, FAR)
}

/// Indices `i0..i1` of the points of a sorted `x` column that can be visible
/// in `view`, plus one neighbor on each side so lines reach the edges.
pub(crate) fn visible_range(x: &[f64], sorted: bool, view: (f64, f64)) -> (usize, usize) {
    let n = x.len();
    if !sorted {
        return (0, n);
    }
    let (lo, hi) = (view.0.min(view.1), view.0.max(view.1));
    let i0 = x.partition_point(|&v| v < lo).saturating_sub(1);
    let i1 = (x.partition_point(|&v| v <= hi) + 1).min(n);
    (i0, i1.max(i0))
}

/// Draws the visible marks in order.
pub(crate) fn marks(ctx: &Ctx<'_>, marks: &[MarkEntry], s: &mut Scratch, frame: &mut Frame) {
    for m in marks
        .iter()
        .filter(|m| m.opts.visible && !m.data.is_empty())
    {
        match m.kind {
            MarkKind::Line => line(ctx, m, s, frame),
            MarkKind::Scatter => scatter(ctx, m, s, frame),
            MarkKind::Area => shapes::area(ctx, marks, m, s, frame),
            MarkKind::Bars => shapes::bars(ctx, marks, m, frame),
            MarkKind::Waterfall => shapes::waterfall(ctx, m, frame),
            MarkKind::Cells => shapes::cells(ctx, m, frame),
        }
    }
}

pub(crate) fn flush(b: &mut FrameBuilder<'_>, pts: &mut Vec<PointPx>, stroke: StrokeStyle) {
    if pts.len() >= 2 {
        b.polyline(pts.iter().copied(), stroke);
    }
    pts.clear();
}

/// One pixel column of a decimated line: indices of its first, lowest,
/// highest and last points. Extremes are compared in data space (the value
/// to pixel map is monotonic); only the kept points are converted.
#[derive(Clone, Copy)]
struct Column {
    /// The column (whole pixels).
    x: i64,
    first: usize,
    lo: (usize, f64),
    hi: (usize, f64),
    last: usize,
}

impl Column {
    fn new(x: i64, i: usize, y: f64) -> Self {
        Self {
            x,
            first: i,
            lo: (i, y),
            hi: (i, y),
            last: i,
        }
    }

    fn add(&mut self, i: usize, y: f64) {
        if y < self.lo.1 {
            self.lo = (i, y);
        }
        if y > self.hi.1 {
            self.hi = (i, y);
        }
        self.last = i;
    }

    /// Appends the column's points in index order, without repeats.
    fn emit(&self, point: impl Fn(usize) -> PointPx, out: &mut Vec<PointPx>) {
        let mut v = [self.first, self.lo.0, self.hi.0, self.last];
        v.sort_unstable();
        let mut prev = usize::MAX;
        for i in v {
            if i != prev {
                out.push(point(i));
                prev = i;
            }
        }
    }
}

/// A sorted line with more points than pixel columns: per column, the
/// first, lowest, highest and last points, so the output does not
/// grow with the data.
fn decimated_line(
    m: &MarkEntry,
    (i0, i1): (usize, usize),
    (mx, my): (PxMap, PxMap),
    pts: &mut Vec<PointPx>,
    b: &mut FrameBuilder<'_>,
    stroke: StrokeStyle,
) {
    let d = &m.data;
    let point = |i: usize| PointPx {
        x: clamp_px(mx.px(d.x[i])),
        y: clamp_px(my.px(d.y[i])),
    };
    let mut col: Option<Column> = None;
    for i in i0..i1 {
        let (x, y) = (clamp_px(mx.px(d.x[i])), d.y[i]);
        if !(x.is_finite() && my.has_px(y)) {
            if let Some(c) = col.take() {
                c.emit(point, pts);
            }
            flush(b, pts, stroke);
            continue;
        }
        let cx = x.floor() as i64;
        match col.as_mut() {
            Some(c) if c.x == cx => c.add(i, y),
            _ => {
                if let Some(c) = col.replace(Column::new(cx, i, y)) {
                    c.emit(point, pts);
                }
            }
        }
    }
    if let Some(c) = col {
        c.emit(point, pts);
    }
    flush(b, pts, stroke);
}

fn line(ctx: &Ctx<'_>, m: &MarkEntry, s: &mut Scratch, frame: &mut Frame) {
    let d = &m.data;
    let ya = ctx.axis(m.opts.y_axis);
    let (i0, i1) = visible_range(&d.x, m.x_sorted, ctx.x.view);
    let stroke = StrokeStyle {
        width: m.opts.line_width.to_px(ctx.dpr),
        color: m.color(),
        dash: m.opts.line_style,
    };
    let decimate = m.x_sorted && !m.opts.step_line && (i1 - i0) as f32 > 4.0 * ctx.plot.w;
    let (mx, my) = (ctx.x.point_map(), ya.point_map());
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    s.pts.clear();
    if decimate {
        decimated_line(m, (i0, i1), (mx, my), &mut s.pts, &mut b, stroke);
        return;
    }
    let mut prev: Option<PointPx> = None;
    for i in i0..i1 {
        let (px, py) = (mx.px(d.x[i]), my.px(d.y[i]));
        if !(px.is_finite() && py.is_finite()) {
            flush(&mut b, &mut s.pts, stroke);
            prev = None;
            continue;
        }
        let p = PointPx {
            x: clamp_px(px),
            y: clamp_px(py),
        };
        if m.opts.step_line
            && let Some(q) = prev
        {
            s.pts.push(PointPx { x: p.x, y: q.y });
        }
        s.pts.push(p);
        prev = Some(p);
    }
    flush(&mut b, &mut s.pts, stroke);
    // Vertex symbols (not on decimated lines: they would cover it).
    let size = ctx.px(m.opts.point_size);
    if size > 0.0 {
        let r = size * 0.5;
        for i in i0..i1 {
            let p = PointPx {
                x: clamp_px(mx.px(d.x[i])),
                y: clamp_px(my.px(d.y[i])),
            };
            if near_plot(ctx.plot, p, r) {
                b.icon(IconInst {
                    pos: p,
                    size: Px(size),
                    icon: m.opts.icon,
                    color: m.point_color(i),
                    ..Default::default()
                });
            }
        }
    }
}

/// `p` is inside the plot grown by `r` (NaN is not).
fn near_plot(plot: RectPx, p: PointPx, r: f32) -> bool {
    p.x >= plot.x - r && p.x <= plot.right() + r && p.y >= plot.y - r && p.y <= plot.bottom() + r
}

fn overlaps(a: RectPx, b: RectPx) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

fn scatter(ctx: &Ctx<'_>, m: &MarkEntry, s: &mut Scratch, frame: &mut Frame) {
    let d = &m.data;
    let ya = ctx.axis(m.opts.y_axis);
    let (i0, i1) = visible_range(&d.x, m.x_sorted, ctx.x.view);
    let default_css = if m.opts.point_size.0 > 0.0 {
        m.opts.point_size.0
    } else {
        DEFAULT_POINT_CSS
    };
    let diameter = |i: usize| {
        let c = d
            .size
            .as_ref()
            .and_then(|c| c.get(i))
            .copied()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(default_css);
        ctx.px(Css(c))
    };
    let (mx, my) = (ctx.x.point_map(), ya.point_map());
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    for i in i0..i1 {
        let p = PointPx {
            x: clamp_px(mx.px(d.x[i])),
            y: clamp_px(my.px(d.y[i])),
        };
        let size = diameter(i);
        if size > 0.0 && near_plot(ctx.plot, p, size * 0.5) {
            b.icon(IconInst {
                pos: p,
                size: Px(size),
                icon: m.opts.icon,
                color: m.point_color(i),
                ..Default::default()
            });
        }
    }
    let Some(labels) = d.label.as_ref() else {
        return;
    };
    s.placed.clear();
    let gap = ctx.px(Css(4.0));
    let label = Label {
        font: ctx.font,
        color: ctx.theme.text,
        align: TextAlign::Left,
        bg: None,
    };
    for i in i0..i1 {
        let Some(t) = labels.get(i).filter(|t| !t.is_empty()) else {
            continue;
        };
        let p = PointPx {
            x: clamp_px(mx.px(d.x[i])),
            y: clamp_px(my.px(d.y[i])),
        };
        if !near_plot(ctx.plot, p, 0.0) {
            continue;
        }
        let pos = PointPx {
            x: p.x + diameter(i) * 0.5 + gap,
            y: p.y,
        };
        let r = RectPx {
            x: pos.x,
            y: pos.y - ctx.line_h * 0.5,
            w: text_width(ctx.text, t, ctx.font),
            h: ctx.line_h,
        };
        if s.placed.iter().any(|&q| overlaps(q, r)) {
            continue;
        }
        if s.placed.len() >= MAX_SCATTER_LABELS {
            break;
        }
        s.placed.push(r);
        b.text(pos, t, label);
    }
}

/// The color of rule `r`.
fn rule_color(theme: &Theme, r: &RuleOptions) -> Rgba {
    r.color.unwrap_or(theme.crosshair)
}

/// Opacity of rule bands: 12% of the rule color's.
fn band_alpha(c: Rgba) -> u8 {
    (f32::from(c.a) * 0.12).round() as u8
}

/// A rule line or band inside the plot (the caller clips to the plot).
pub(crate) fn rule(ctx: &Ctx<'_>, r: &RuleOptions, b: &mut FrameBuilder<'_>) {
    let plot = ctx.plot;
    let a = ctx.axis(r.axis);
    let color = rule_color(ctx.theme, r);
    let p0 = clamp_px(a.to_px(r.value));
    if let Some(to) = r.to {
        let p1 = clamp_px(a.to_px(to));
        if !(p0.is_finite() && p1.is_finite()) {
            return;
        }
        let (lo, hi) = (p0.min(p1), p0.max(p1));
        let fill = color.with_alpha(band_alpha(color));
        let rect = if r.axis == AxisId::X {
            let (lo, hi) = (lo.max(plot.x), hi.min(plot.right()));
            RectPx {
                x: lo.round(),
                y: plot.y,
                w: (hi.round() - lo.round()).max(0.0),
                h: plot.h,
            }
        } else {
            let (lo, hi) = (lo.max(plot.y), hi.min(plot.bottom()));
            RectPx {
                x: plot.x,
                y: lo.round(),
                w: plot.w,
                h: (hi.round() - lo.round()).max(0.0),
            }
        };
        b.rect(rect, fill);
        return;
    }
    if !p0.is_finite() {
        return;
    }
    let stroke = StrokeStyle {
        width: r.line_width.to_px(ctx.dpr),
        color,
        dash: r.line_style,
    };
    let p = p0.floor() + 0.5;
    if r.axis == AxisId::X {
        b.line(
            PointPx { x: p, y: plot.y },
            PointPx {
                x: p,
                y: plot.bottom(),
            },
            stroke,
        );
    } else {
        b.line(
            PointPx { x: plot.x, y: p },
            PointPx {
                x: plot.right(),
                y: p,
            },
            stroke,
        );
    }
}

/// The label of rule `r` on its axis (outside the plot clip).
pub(crate) fn rule_label(ctx: &Ctx<'_>, r: &RuleOptions, b: &mut FrameBuilder<'_>) {
    let Some(text) = r.label.as_deref().filter(|t| !t.is_empty()) else {
        return;
    };
    let a = ctx.axis(r.axis);
    if !a.opts.visible {
        return;
    }
    let plot = ctx.plot;
    let p = clamp_px(a.to_px(r.value));
    let color = rule_color(ctx.theme, r);
    let label = |align| Label {
        font: ctx.font,
        color: if r.label_bg { contrast(color) } else { color },
        align,
        bg: r.label_bg.then_some(LabelBg {
            color,
            padding: Px((ctx.pad * 0.5).round()),
        }),
    };
    match r.axis {
        AxisId::X => {
            if p >= plot.x && p <= plot.right() {
                let y = plot.bottom() + ctx.pad + ctx.line_h * 0.5;
                b.text(PointPx { x: p, y }, text, label(TextAlign::Center));
            }
        }
        AxisId::Left => {
            if p >= plot.y && p <= plot.bottom() {
                b.text(
                    PointPx {
                        x: plot.x - ctx.pad,
                        y: p,
                    },
                    text,
                    label(TextAlign::Right),
                );
            }
        }
        AxisId::Right => {
            if p >= plot.y && p <= plot.bottom() {
                b.text(
                    PointPx {
                        x: plot.right() + ctx.pad,
                        y: p,
                    },
                    text,
                    label(TextAlign::Left),
                );
            }
        }
    }
}

/// Size of label symbols.
const LABEL_ICON_CSS: f32 = 8.0;

/// A text or symbol at data coordinates.
pub(crate) fn label(ctx: &Ctx<'_>, l: &LabelOptions, b: &mut FrameBuilder<'_>) {
    let ya = ctx.axis(l.y_axis);
    let p = PointPx {
        x: clamp_px(ctx.x.point_px(l.x)),
        y: clamp_px(ya.point_px(l.y)),
    };
    if !(p.x.is_finite() && p.y.is_finite()) {
        return;
    }
    let color = l.color.unwrap_or(ctx.theme.text);
    if let Some(icon) = l.icon {
        b.icon(IconInst {
            pos: p,
            size: Px(ctx.px(Css(LABEL_ICON_CSS))),
            icon,
            color,
            ..Default::default()
        });
    }
    if !l.text.is_empty() {
        let pos = PointPx {
            x: p.x + ctx.px(l.offset.0),
            y: p.y + ctx.px(l.offset.1),
        };
        let label = Label {
            font: ctx.font,
            color,
            align: l.align,
            bg: None,
        };
        b.text(pos, &l.text, label);
    }
}

/// Size of the highlight around a hovered point, beyond the point (CSS px),
/// and the smallest highlighted point.
const HALO_CSS: f32 = 8.0;
const MIN_HIGHLIGHT_CSS: f32 = 8.0;

/// Highlights point `i` of `m` drawn as `shape`: a halo and a symbol for
/// points, a tint over bars and cells.
pub(crate) fn highlight(
    ctx: &Ctx<'_>,
    m: &MarkEntry,
    i: usize,
    shape: crate::hit::Shape,
    b: &mut FrameBuilder<'_>,
) {
    match shape {
        crate::hit::Shape::Point { pos, size } => {
            let color = m.point_color(i);
            let size = size.max(ctx.px(Css(MIN_HIGHLIGHT_CSS)));
            let icon = if m.kind == MarkKind::Scatter || m.opts.point_size.0 > 0.0 {
                m.opts.icon
            } else {
                ferrochart_core::transform_helpers::ICON_CIRCLE
            };
            b.icon(IconInst {
                pos,
                size: Px(size + ctx.px(Css(HALO_CSS))),
                icon,
                color: color.with_alpha((f32::from(color.a) * 0.3).round() as u8),
                ..Default::default()
            });
            b.icon(IconInst {
                pos,
                size: Px(size),
                icon,
                color,
                ..Default::default()
            });
        }
        crate::hit::Shape::Rect(r) => b.rect(r, ctx.theme.text.with_alpha(40)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_of_sorted_x() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(visible_range(&x, true, (1.5, 3.5)), (1, 5));
        assert_eq!(visible_range(&x, true, (-10.0, 10.0)), (0, 6));
        assert_eq!(visible_range(&x, true, (10.0, 20.0)), (5, 6));
        assert_eq!(visible_range(&x, false, (1.5, 3.5)), (0, 6));
    }

    #[test]
    fn decimated_column_keeps_extremes_in_order() {
        let ys = [5.0, 9.0, 1.0, 4.0];
        let mut c = Column::new(10, 0, ys[0]);
        for (i, &y) in ys.iter().enumerate().skip(1) {
            c.add(i, y);
        }
        let mut out = Vec::new();
        c.emit(
            |i| PointPx {
                x: 10.0,
                y: ys[i] as f32,
            },
            &mut out,
        );
        let got: Vec<f32> = out.iter().map(|q| q.y).collect();
        assert_eq!(got, [5.0, 9.0, 1.0, 4.0]);
        // A column of one point emits it once.
        out.clear();
        Column::new(3, 2, 1.0).emit(
            |i| PointPx {
                x: 3.0,
                y: ys[i] as f32,
            },
            &mut out,
        );
        assert_eq!(out.len(), 1);
    }
}
