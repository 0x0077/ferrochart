//! The second batch of drawing tools: pitchforks,
//! Gann fan, Fibonacci time zones, XABCD, head and shoulders, Elliott waves,
//! arrow, callout, path (`AnchorCount::Open`) and brush
//! (`AnchorCount::Freehand`). The three pitchforks are one tool with three
//! kinds, as are the two Elliott waves.
//!
//! Geometry (screen space, `p0`, `p1`, … the anchors):
//!
//! - pitchfork: the median line runs from the origin through the midpoint of
//!   `p1`–`p2`, the tines through `p1` and `p2` parallel to it, all extended to
//!   the plot edge; the origin is `p0` (Andrews), `p0` moved halfway to `p1`
//!   in y (Schiff) or the midpoint of `p0`–`p1` (modified Schiff);
//! - Gann fan: rays from `p0` with slopes `k` times that of `p0`–`p1`, for
//!   `k` in 1/8, 1/4, 1/3, 1/2, 1, 2, 3, 4, 8, labelled `1x8` … `8x1`;
//! - Fibonacci time zones: vertical lines at logical index
//!   `i0 + (i1 − i0) · F` for the Fibonacci numbers 0, 1, 2, 3, 5, 8, …;
//! - patterns and waves: the polyline through the anchors with point labels
//!   (above a peak, below a trough); XABCD adds the retracement ratios
//!   between the legs, head and shoulders the neckline through `p2` and `p4`.

use core::fmt::Write;

use crate::error::{Error, Result};
use crate::frame::{LabelBg, LineDash, StrokeStyle, TextAlign};
use crate::geom::{Css, PointPx, RectPx};
use crate::text::StackStr;

use super::{
    Anchor, AnchorCount, DrawingCtx, DrawingData, DrawingStyle, DrawingTool, FrameBuilder, HitPart,
    Label, Param, clip_line, distance_to_segment, point_in_polygon,
};

/// Kinds of the second batch.
pub(crate) const SECOND_KINDS: [&str; 13] = [
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

const SHAPES: [Shape; 13] = [
    Shape::Pitchfork(Fork::Andrews),
    Shape::Pitchfork(Fork::Schiff),
    Shape::Pitchfork(Fork::ModifiedSchiff),
    Shape::GannFan,
    Shape::FibTimeZones,
    Shape::Xabcd,
    Shape::HeadShoulders,
    Shape::Elliott { impulse: true },
    Shape::Elliott { impulse: false },
    Shape::Path,
    Shape::Brush,
    Shape::Arrow,
    Shape::Callout,
];

/// Gann fan slopes relative to the 1x1 line, and their labels.
const GANN: [(f32, &str); 9] = [
    (0.125, "1x8"),
    (0.25, "1x4"),
    (1.0 / 3.0, "1x3"),
    (0.5, "1x2"),
    (1.0, "1x1"),
    (2.0, "2x1"),
    (3.0, "3x1"),
    (4.0, "4x1"),
    (8.0, "8x1"),
];

/// Fibonacci numbers of the time zones.
const FIB_ZONES: [u32; 14] = [0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377];

const XABCD_LABELS: [&str; 5] = ["X", "A", "B", "C", "D"];
const HS_LABELS: [&str; 7] = ["", "Left Shoulder", "", "Head", "", "Right Shoulder", ""];
const IMPULSE_LABELS: [&str; 6] = ["0", "1", "2", "3", "4", "5"];
const CORRECTION_LABELS: [&str; 4] = ["0", "A", "B", "C"];

/// Gap between a point and its label (CSS px).
const LABEL_GAP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fork {
    Andrews,
    Schiff,
    ModifiedSchiff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Pitchfork(Fork),
    GannFan,
    FibTimeZones,
    Xabcd,
    HeadShoulders,
    Elliott { impulse: bool },
    Path,
    Brush,
    Arrow,
    Callout,
}

impl Shape {
    fn count(self) -> AnchorCount {
        match self {
            Self::Pitchfork(_) => AnchorCount::Fixed(3),
            Self::Xabcd => AnchorCount::Fixed(5),
            Self::HeadShoulders => AnchorCount::Fixed(7),
            Self::Elliott { impulse: true } => AnchorCount::Fixed(6),
            Self::Elliott { impulse: false } => AnchorCount::Fixed(4),
            Self::Path => AnchorCount::Open { min: 2 },
            Self::Brush => AnchorCount::Freehand,
            Self::GannFan | Self::FibTimeZones | Self::Arrow | Self::Callout => {
                AnchorCount::Fixed(2)
            }
        }
    }

    /// Point labels of the patterns and waves.
    fn labels(self) -> &'static [&'static str] {
        match self {
            Self::Xabcd => &XABCD_LABELS,
            Self::HeadShoulders => &HS_LABELS,
            Self::Elliott { impulse: true } => &IMPULSE_LABELS,
            Self::Elliott { impulse: false } => &CORRECTION_LABELS,
            _ => &[],
        }
    }
}

struct Tool {
    kind: &'static str,
    shape: Shape,
    anchors: Vec<Anchor>,
    style: DrawingStyle,
    /// Callout text.
    text: String,
}

/// Factory of the second-batch kinds.
pub(crate) fn build(data: &DrawingData) -> Result<Box<dyn DrawingTool>> {
    let k = SECOND_KINDS
        .iter()
        .position(|&k| k == data.kind)
        .ok_or(Error::InvalidArgument("unknown drawing kind"))?;
    let shape = SHAPES[k];
    if data.anchors.len() > shape.count().max() {
        return Err(Error::InvalidArgument("too many anchors"));
    }
    let text = match (shape, data.param("text")) {
        (Shape::Callout, Some(Param::Text(s))) => s.clone(),
        (Shape::Callout, Some(_)) => return Err(Error::InvalidArgument("text must be a text")),
        (Shape::Callout, None) => "Callout".into(),
        _ => String::new(),
    };
    Ok(Box::new(Tool {
        kind: SECOND_KINDS[k],
        shape,
        anchors: data.anchors.clone(),
        style: data.style,
        text,
    }))
}

fn mid(a: PointPx, b: PointPx) -> PointPx {
    PointPx {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

fn add(a: PointPx, dx: f32, dy: f32) -> PointPx {
    PointPx {
        x: a.x + dx,
        y: a.y + dy,
    }
}

/// Distance from `p` to the polyline `pts`.
fn polyline_distance(p: PointPx, pts: impl Iterator<Item = PointPx>) -> f32 {
    let mut prev: Option<PointPx> = None;
    let mut best = f32::INFINITY;
    for q in pts {
        best = best.min(match prev {
            Some(a) => distance_to_segment(p, a, q),
            None => (p.x - q.x).hypot(p.y - q.y),
        });
        prev = Some(q);
    }
    best
}

impl Tool {
    fn point(&self, ctx: &DrawingCtx<'_>, k: usize) -> Option<PointPx> {
        ctx.point(*self.anchors.get(k)?)
    }

    fn points<'a>(&'a self, ctx: &'a DrawingCtx<'_>) -> impl Iterator<Item = PointPx> + 'a {
        self.anchors.iter().filter_map(|a| ctx.point(*a))
    }

    fn font_label(&self, ctx: &DrawingCtx<'_>, align: TextAlign) -> Label {
        Label {
            font: ctx.font(self.style.font_size),
            color: self.style.text,
            align,
            bg: None,
        }
    }

    /// Origin, median end (the midpoint of `p1`–`p2`), `p1` and `p2` of a
    /// pitchfork.
    fn fork(&self, ctx: &DrawingCtx<'_>, fork: Fork) -> Option<[PointPx; 4]> {
        let (p0, p1, p2) = (
            self.point(ctx, 0)?,
            self.point(ctx, 1)?,
            self.point(ctx, 2)?,
        );
        let origin = match fork {
            Fork::Andrews => p0,
            Fork::Schiff => PointPx {
                x: p0.x,
                y: (p0.y + p1.y) * 0.5,
            },
            Fork::ModifiedSchiff => mid(p0, p1),
        };
        Some([origin, mid(p1, p2), p1, p2])
    }

    /// The median and the two tines of a pitchfork, clipped to the plot.
    fn fork_lines(&self, ctx: &DrawingCtx<'_>, fork: Fork) -> [Option<(PointPx, PointPx)>; 3] {
        let Some([o, m, p1, p2]) = self.fork(ctx, fork) else {
            return [None; 3];
        };
        let (dx, dy) = (m.x - o.x, m.y - o.y);
        let plot = ctx.plot();
        [
            clip_line(o, m, plot, false, true),
            clip_line(p1, add(p1, dx, dy), plot, false, true),
            clip_line(p2, add(p2, dx, dy), plot, false, true),
        ]
    }

    /// End point of the Gann ray of slope factor `k`, one `p0`–`p1` step
    /// away from `p0`.
    fn gann_point(p0: PointPx, p1: PointPx, k: f32) -> PointPx {
        PointPx {
            x: p1.x,
            y: p0.y + (p1.y - p0.y) * k,
        }
    }

    /// x of the Fibonacci time zones, in order.
    fn zones<'a>(&'a self, ctx: &'a DrawingCtx<'_>) -> impl Iterator<Item = (u32, f32)> + 'a {
        let ends = self
            .anchors
            .first()
            .zip(self.anchors.get(1))
            .and_then(|(a, b)| Some((ctx.index_of_time(a.time)?, ctx.index_of_time(b.time)?)));
        let right = ctx.plot().right();
        FIB_ZONES.iter().map_while(move |&f| {
            let (i0, i1) = ends?;
            let x = ctx.x_of_index(i0 + (i1 - i0) * f64::from(f))?;
            // Zones of a zero-width unit all fall on the first one.
            (x <= right + 1.0 && (f < 2 || (i1 - i0).abs() > 1e-9)).then_some((f, x))
        })
    }

    /// The callout's text box (left edge at `p1`, vertically centered).
    fn callout_box(&self, ctx: &DrawingCtx<'_>) -> Option<RectPx> {
        let p = self.point(ctx, 1)?;
        let font = ctx.font(self.style.font_size);
        let (w, h) = (ctx.text_width(&self.text, font), ctx.line_height(font));
        let pad = ctx.px(Css(4.0)).0;
        Some(RectPx {
            x: p.x - pad,
            y: p.y - h * 0.5 - pad,
            w: w + 2.0 * pad,
            h: h + 2.0 * pad,
        })
    }

    /// Head of an arrow ending at `to`, coming from `from`.
    fn arrow_head(
        ctx: &DrawingCtx<'_>,
        from: PointPx,
        to: PointPx,
        width: f32,
    ) -> Option<[PointPx; 3]> {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let len = dx.hypot(dy);
        let head = ctx.px(Css(6.0)).0 + 2.0 * width;
        if len <= f32::EPSILON {
            return None;
        }
        let (ux, uy) = (dx / len, dy / len);
        let back = add(to, -head * ux, -head * uy);
        let side = head * 0.5;
        Some([
            to,
            add(back, side * uy, -side * ux),
            add(back, -side * uy, side * ux),
        ])
    }

    /// Labels of pattern points: above a peak, below a trough.
    fn point_labels(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>) {
        let label = self.font_label(ctx, TextAlign::Center);
        let h = ctx.line_height(label.font);
        let gap = ctx.px(Css(LABEL_GAP)).0;
        let n = self.anchors.len();
        for (k, name) in self.shape.labels().iter().enumerate().take(n) {
            if name.is_empty() {
                continue;
            }
            let Some(p) = self.point(ctx, k) else {
                continue;
            };
            let price = self.anchors[k].price;
            let neighbours = [k.checked_sub(1), (k + 1 < n).then_some(k + 1)];
            let peak = neighbours
                .iter()
                .flatten()
                .all(|&j| self.anchors[j].price <= price);
            let dy = h * 0.5 + gap;
            let y = if peak { p.y - dy } else { p.y + dy };
            out.text(PointPx { x: p.x, y }, name, label);
        }
    }

    /// The ratio `|leg b| / |leg a|` of anchor prices, at the middle of the
    /// dashed line from `from` to `to`.
    fn ratio(
        &self,
        ctx: &DrawingCtx<'_>,
        out: &mut FrameBuilder<'_>,
        legs: ((usize, usize), (usize, usize)),
        line: (usize, usize),
        stroke: StrokeStyle,
    ) {
        let price = |k: usize| self.anchors.get(k).map(|a| a.price);
        let (((a0, a1), (b0, b1)), (from, to)) = (legs, line);
        let (Some(a0), Some(a1), Some(b0), Some(b1)) = (price(a0), price(a1), price(b0), price(b1))
        else {
            return;
        };
        let (Some(p), Some(q)) = (self.point(ctx, from), self.point(ctx, to)) else {
            return;
        };
        let dashed = StrokeStyle {
            dash: LineDash::Dashed,
            ..stroke
        };
        out.line(p, q, dashed);
        let base = (a1 - a0).abs();
        if base > 0.0 {
            let mut s = StackStr::<16>::new();
            let _ = write!(s, "{:.3}", (b1 - b0).abs() / base);
            let mut label = self.font_label(ctx, TextAlign::Center);
            label.bg = Some(LabelBg {
                color: WHITE,
                padding: ctx.px(Css(2.0)),
            });
            out.text(mid(p, q), s.as_str(), label);
        }
    }

    fn build_fork(
        &self,
        ctx: &DrawingCtx<'_>,
        out: &mut FrameBuilder<'_>,
        fork: Fork,
        stroke: StrokeStyle,
    ) {
        let Some([o, m, p1, p2]) = self.fork(ctx, fork) else {
            return;
        };
        let lines = self.fork_lines(ctx, fork);
        // Fill between the tines, far past the plot (clipped by the caller).
        let (dx, dy) = (m.x - o.x, m.y - o.y);
        let len = dx.hypot(dy);
        if len > f32::EPSILON {
            let plot = ctx.plot();
            let t = 2.0 * (plot.w + plot.h) / len;
            out.polygon(
                [p1, add(p1, dx * t, dy * t), add(p2, dx * t, dy * t), p2],
                self.style.fill,
            );
        }
        for (a, b) in lines.into_iter().flatten() {
            out.line(a, b, stroke);
        }
        out.line(p1, p2, stroke);
        if fork != Fork::Andrews
            && let Some(p0) = self.point(ctx, 0)
        {
            // Where the origin moved from.
            let dashed = StrokeStyle {
                dash: LineDash::Dashed,
                ..stroke
            };
            out.line(p0, o, dashed);
        }
    }

    fn build_gann(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        let (Some(p0), Some(p1)) = (self.point(ctx, 0), self.point(ctx, 1)) else {
            return;
        };
        let label = self.font_label(ctx, TextAlign::Left);
        let gap = ctx.px(Css(LABEL_GAP)).0;
        for (k, name) in GANN {
            let main = name == "1x1";
            let q = Self::gann_point(p0, p1, k);
            let Some((a, b)) = clip_line(p0, q, ctx.plot(), false, true) else {
                continue;
            };
            let s = if main {
                stroke
            } else {
                StrokeStyle {
                    color: stroke
                        .color
                        .with_alpha(stroke.color.a / 2 + stroke.color.a / 4),
                    ..stroke
                }
            };
            out.line(a, b, s);
            out.text(add(q, gap, 0.0), name, label);
        }
    }

    fn build_zones(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        let plot = ctx.plot();
        let label = self.font_label(ctx, TextAlign::Left);
        let gap = ctx.px(Css(LABEL_GAP)).0;
        let h = ctx.line_height(label.font);
        for (f, x) in self.zones(ctx) {
            let x = super::crisp(x, stroke.width);
            out.line(
                PointPx { x, y: plot.y },
                PointPx {
                    x,
                    y: plot.bottom(),
                },
                stroke,
            );
            let mut s = StackStr::<8>::new();
            let _ = write!(s, "{f}");
            out.text(
                PointPx {
                    x: x + gap,
                    y: plot.y + gap + h * 0.5,
                },
                s.as_str(),
                label,
            );
        }
        // The unit: anchors 0 and 1 joined.
        if let (Some(a), Some(b)) = (self.point(ctx, 0), self.point(ctx, 1)) {
            let dashed = StrokeStyle {
                dash: LineDash::Dashed,
                ..stroke
            };
            out.line(a, b, dashed);
        }
    }

    fn build_pattern(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        match self.shape {
            Shape::Xabcd => {
                for tri in [[0, 1, 2], [2, 3, 4]] {
                    out.polygon(
                        tri.iter().filter_map(|&k| self.point(ctx, k)),
                        self.style.fill,
                    );
                }
            }
            Shape::HeadShoulders => {
                for tri in [[0, 1, 2], [2, 3, 4], [4, 5, 6]] {
                    out.polygon(
                        tri.iter().filter_map(|&k| self.point(ctx, k)),
                        self.style.fill,
                    );
                }
            }
            _ => {}
        }
        out.polyline(self.points(ctx), stroke);
        match self.shape {
            Shape::Xabcd => {
                // B retraces XA, C retraces AB, D extends BC, D retraces XA.
                self.ratio(ctx, out, ((0, 1), (1, 2)), (0, 2), stroke);
                self.ratio(ctx, out, ((1, 2), (2, 3)), (1, 3), stroke);
                self.ratio(ctx, out, ((2, 3), (3, 4)), (2, 4), stroke);
                self.ratio(ctx, out, ((0, 1), (1, 4)), (0, 4), stroke);
            }
            Shape::HeadShoulders => {
                if let Some((a, b)) = self.neckline(ctx) {
                    out.line(a, b, stroke);
                }
            }
            _ => {}
        }
        self.point_labels(ctx, out);
    }

    /// The neckline through anchors 2 and 4, from anchor 0's x to anchor 6's.
    fn neckline(&self, ctx: &DrawingCtx<'_>) -> Option<(PointPx, PointPx)> {
        let (n1, n2) = (self.point(ctx, 2)?, self.point(ctx, 4)?);
        let (x0, x1) = (self.point(ctx, 0)?.x, self.point(ctx, 6)?.x);
        let y_at = |x: f32| {
            if (n2.x - n1.x).abs() > f32::EPSILON {
                n1.y + (n2.y - n1.y) * (x - n1.x) / (n2.x - n1.x)
            } else {
                n1.y
            }
        };
        Some((
            PointPx { x: x0, y: y_at(x0) },
            PointPx { x: x1, y: y_at(x1) },
        ))
    }

    fn build_callout(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        let (Some(target), Some(r)) = (self.point(ctx, 0), self.callout_box(ctx)) else {
            return;
        };
        // Leader from the target to the nearest point of the box.
        let near = PointPx {
            x: target.x.clamp(r.x, r.right()),
            y: target.y.clamp(r.y, r.bottom()),
        };
        out.line(target, near, stroke);
        out.rect(r, self.style.line);
        let label = Label {
            font: ctx.font(self.style.font_size),
            color: WHITE,
            align: TextAlign::Left,
            bg: None,
        };
        if let Some(p) = self.point(ctx, 1) {
            out.text(p, &self.text, label);
        }
    }
}

/// Text on the line color (callout box) and background of ratio labels.
const WHITE: crate::geom::Rgba = crate::geom::Rgba::hex(0x00FF_FFFF);

impl DrawingTool for Tool {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    fn anchors_mut(&mut self) -> &mut Vec<Anchor> {
        &mut self.anchors
    }

    fn anchor_count(&self) -> AnchorCount {
        self.shape.count()
    }

    fn style(&self) -> &DrawingStyle {
        &self.style
    }

    fn save(&self) -> DrawingData {
        let mut d = DrawingData::new(self.kind, self.anchors.clone());
        d.style = self.style;
        if self.shape == Shape::Callout {
            d.set("text", Param::Text(self.text.clone()));
        }
        d
    }

    fn build(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>) {
        let stroke = ctx.stroke(&self.style);
        match self.shape {
            Shape::Pitchfork(fork) => self.build_fork(ctx, out, fork, stroke),
            Shape::GannFan => self.build_gann(ctx, out, stroke),
            Shape::FibTimeZones => self.build_zones(ctx, out, stroke),
            Shape::Xabcd | Shape::HeadShoulders | Shape::Elliott { .. } => {
                self.build_pattern(ctx, out, stroke);
            }
            Shape::Path | Shape::Brush => out.polyline(self.points(ctx), stroke),
            Shape::Arrow => {
                let (Some(a), Some(b)) = (self.point(ctx, 0), self.point(ctx, 1)) else {
                    return;
                };
                out.line(a, b, stroke);
                if let Some(head) = Self::arrow_head(ctx, a, b, stroke.width.0) {
                    out.polygon(head, stroke.color);
                }
            }
            Shape::Callout => self.build_callout(ctx, out, stroke),
        }
    }

    fn hit_test(&self, ctx: &DrawingCtx<'_>, p: PointPx) -> Option<HitPart> {
        let tol = ctx.hit_tolerance();
        let near = |a: PointPx, b: PointPx| distance_to_segment(p, a, b) <= tol;
        let filled = self.style.fill.a > 0;
        let hit = match self.shape {
            Shape::Pitchfork(fork) => {
                let lines = self.fork_lines(ctx, fork);
                lines.into_iter().flatten().any(|(a, b)| near(a, b))
                    || self.fork(ctx, fork).is_some_and(|[o, m, p1, p2]| {
                        let (dx, dy) = (m.x - o.x, m.y - o.y);
                        let plot = ctx.plot();
                        let t = 2.0 * (plot.w + plot.h) / dx.hypot(dy).max(f32::EPSILON);
                        near(p1, p2)
                            || self.point(ctx, 0).is_some_and(|p0| near(p0, o))
                            || (filled
                                && point_in_polygon(
                                    p,
                                    &[p1, add(p1, dx * t, dy * t), add(p2, dx * t, dy * t), p2],
                                ))
                    })
            }
            Shape::GannFan => match (self.point(ctx, 0), self.point(ctx, 1)) {
                (Some(p0), Some(p1)) => GANN.iter().any(|&(k, _)| {
                    clip_line(p0, Self::gann_point(p0, p1, k), ctx.plot(), false, true)
                        .is_some_and(|(a, b)| near(a, b))
                }),
                _ => false,
            },
            Shape::FibTimeZones => self.zones(ctx).any(|(_, x)| (p.x - x).abs() <= tol),
            Shape::Xabcd | Shape::HeadShoulders | Shape::Elliott { .. } | Shape::Path => {
                polyline_distance(p, self.points(ctx)) <= tol
                    || (self.shape == Shape::HeadShoulders
                        && self.neckline(ctx).is_some_and(|(a, b)| near(a, b)))
            }
            Shape::Brush => {
                polyline_distance(p, self.points(ctx))
                    <= tol + ctx.stroke(&self.style).width.0 * 0.5
            }
            Shape::Arrow => match (self.point(ctx, 0), self.point(ctx, 1)) {
                (Some(a), Some(b)) => near(a, b),
                _ => false,
            },
            Shape::Callout => {
                self.callout_box(ctx).is_some_and(|r| r.contains(p))
                    || match (self.point(ctx, 0), self.callout_box(ctx)) {
                        (Some(t), Some(r)) => near(
                            t,
                            PointPx {
                                x: t.x.clamp(r.x, r.right()),
                                y: t.y.clamp(r.y, r.bottom()),
                            },
                        ),
                        _ => false,
                    }
            }
        };
        hit.then_some(HitPart::Body)
    }

    fn handles(&self, ctx: &DrawingCtx<'_>, out: &mut Vec<PointPx>) {
        // A brush has no handles (moving one of its many points is not
        // useful); it is moved as a whole.
        if self.shape != Shape::Brush {
            out.extend(self.points(ctx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_saved_form() {
        for kind in SECOND_KINDS {
            let mut data = DrawingData::new(kind, Vec::new());
            let tool = build(&data).unwrap_or_else(|e| panic!("{kind}: {e:?}"));
            let n = tool.anchor_count().min();
            data.anchors = (0..n)
                .map(|k| Anchor {
                    time: k as i64 * 1000,
                    price: 100.0 + k as f64,
                })
                .collect();
            let saved = build(&data).map(|t| t.save());
            let again = saved
                .as_ref()
                .ok()
                .and_then(|s| build(s).ok())
                .map(|t| t.save());
            assert_eq!(again.as_ref(), saved.as_ref().ok(), "{kind}");
        }
    }

    #[test]
    fn anchor_counts_and_limits() {
        let count = |k: &str| build(&DrawingData::new(k, Vec::new())).map(|t| t.anchor_count());
        assert_eq!(count("head_and_shoulders"), Ok(AnchorCount::Fixed(7)));
        assert_eq!(count("path"), Ok(AnchorCount::Open { min: 2 }));
        assert_eq!(count("brush"), Ok(AnchorCount::Freehand));
        let a = Anchor {
            time: 0,
            price: 1.0,
        };
        assert!(build(&DrawingData::new("arrow", vec![a; 3])).is_err());
        assert!(
            build(&DrawingData::new(
                "brush",
                vec![a; super::super::MAX_ANCHORS]
            ))
            .is_ok()
        );
        assert!(
            build(&DrawingData::new(
                "brush",
                vec![a; super::super::MAX_ANCHORS + 1]
            ))
            .is_err()
        );
        let mut c = DrawingData::new("callout", Vec::new());
        c.set("text", Param::Number(1.0));
        assert!(build(&c).is_err());
    }

    #[test]
    fn gann_points_scale_the_slope() {
        let (p0, p1) = (PointPx { x: 0.0, y: 100.0 }, PointPx { x: 10.0, y: 90.0 });
        assert_eq!(Tool::gann_point(p0, p1, 2.0), PointPx { x: 10.0, y: 80.0 });
        assert_eq!(Tool::gann_point(p0, p1, 0.5), PointPx { x: 10.0, y: 95.0 });
    }
}
