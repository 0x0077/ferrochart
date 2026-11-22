//! The first 15 drawing tools: lines, channels,
//! shapes, text, measurements, Fibonacci tools and positions. Long and short
//! positions are one tool with two kinds.

use core::fmt::Write;

use crate::error::{Error, Result};
use crate::frame::{LabelBg, LineDash, StrokeStyle, TextAlign};
use crate::geom::{PointPx, RectPx, Rgba};
use crate::text::StackStr;

use super::{
    Anchor, AnchorCount, DrawingCtx, DrawingData, DrawingStyle, DrawingTool, FrameBuilder, HitPart,
    Label, Param, clip_line, crisp, distance_to_segment, point_in_polygon,
};

/// Kinds of the first batch.
pub(crate) const FIRST_KINDS: [&str; 16] = [
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
];

const SHAPES: [Shape; 16] = [
    Shape::Trend,
    Shape::Ray,
    Shape::Extended,
    Shape::HLine,
    Shape::HRay,
    Shape::VLine,
    Shape::Channel,
    Shape::Rect,
    Shape::Ellipse,
    Shape::Text,
    Shape::PriceRange,
    Shape::DateRange,
    Shape::FibRetracement,
    Shape::FibExtension,
    Shape::Position { long: true },
    Shape::Position { long: false },
];

/// Default Fibonacci retracement levels.
const RETRACEMENT: [f64; 7] = [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
/// Default Fibonacci extension levels.
const EXTENSION: [f64; 8] = [0.0, 0.382, 0.618, 1.0, 1.272, 1.618, 2.0, 2.618];
/// At most this many levels.
const MAX_LEVELS: usize = 24;
/// Segments of an ellipse.
const ELLIPSE_SEGMENTS: usize = 64;
/// Profit and loss areas of positions.
const PROFIT: Rgba = Rgba::hex(0x0026_A69A).with_alpha(50);
const LOSS: Rgba = Rgba::hex(0x00EF_5350).with_alpha(50);
const WHITE: Rgba = Rgba::hex(0x00FF_FFFF);
/// Gap between a line and its label (CSS px).
const LABEL_GAP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Trend,
    Ray,
    Extended,
    HLine,
    HRay,
    VLine,
    Channel,
    Rect,
    Ellipse,
    Text,
    PriceRange,
    DateRange,
    FibRetracement,
    FibExtension,
    Position { long: bool },
}

impl Shape {
    /// Clicks that create it.
    fn required(self) -> usize {
        match self {
            Self::HLine | Self::HRay | Self::VLine | Self::Text => 1,
            Self::Channel | Self::FibExtension => 3,
            _ => 2,
        }
    }

    /// Most anchors it has.
    fn max_anchors(self) -> usize {
        match self {
            Self::Position { .. } => 3,
            s => s.required(),
        }
    }
}

struct Builtin {
    kind: &'static str,
    shape: Shape,
    anchors: Vec<Anchor>,
    style: DrawingStyle,
    text: String,
    levels: Vec<f64>,
}

/// Factory of the built-in kinds.
pub(crate) fn builtin(data: &DrawingData) -> Result<Box<dyn DrawingTool>> {
    let k = FIRST_KINDS
        .iter()
        .position(|&k| k == data.kind)
        .ok_or(Error::InvalidArgument("unknown drawing kind"))?;
    let shape = SHAPES[k];
    if data.anchors.len() > shape.max_anchors() {
        return Err(Error::InvalidArgument("too many anchors"));
    }
    let levels = if is_fib(shape) {
        match data.param("levels") {
            Some(Param::Numbers(v)) if v.len() <= MAX_LEVELS && v.iter().all(|x| x.is_finite()) => {
                v.clone()
            }
            Some(_) => {
                return Err(Error::InvalidArgument(
                    "levels must be at most 24 finite numbers",
                ));
            }
            None if shape == Shape::FibExtension => EXTENSION.to_vec(),
            None => RETRACEMENT.to_vec(),
        }
    } else {
        Vec::new()
    };
    let text = match (shape, data.param("text")) {
        (Shape::Text, Some(Param::Text(s))) => s.clone(),
        (Shape::Text, Some(_)) => return Err(Error::InvalidArgument("text must be a text")),
        (Shape::Text, None) => "Text".into(),
        _ => String::new(),
    };
    Ok(Box::new(Builtin {
        kind: FIRST_KINDS[k],
        shape,
        anchors: data.anchors.clone(),
        style: data.style,
        text,
        levels,
    }))
}

fn is_fib(s: Shape) -> bool {
    matches!(s, Shape::FibRetracement | Shape::FibExtension)
}

fn rect_of(p: PointPx, q: PointPx) -> RectPx {
    RectPx {
        x: p.x.min(q.x),
        y: p.y.min(q.y),
        w: (p.x - q.x).abs(),
        h: (p.y - q.y).abs(),
    }
}

fn inflate(r: RectPx, d: f32) -> RectPx {
    RectPx {
        x: r.x - d,
        y: r.y - d,
        w: r.w + 2.0 * d,
        h: r.h + 2.0 * d,
    }
}

fn near_rect_border(p: PointPx, r: RectPx, tol: f32) -> bool {
    inflate(r, tol).contains(p) && !inflate(r, -tol).contains(p)
}

/// The outline of `r`, on pixel centers for crisp 1-px lines.
fn outline(out: &mut FrameBuilder<'_>, r: RectPx, stroke: StrokeStyle) {
    let (x0, x1) = (crisp(r.x, stroke.width), crisp(r.right(), stroke.width));
    let (y0, y1) = (crisp(r.y, stroke.width), crisp(r.bottom(), stroke.width));
    let p = [
        PointPx { x: x0, y: y0 },
        PointPx { x: x1, y: y0 },
        PointPx { x: x1, y: y1 },
        PointPx { x: x0, y: y1 },
    ];
    for k in 0..4 {
        out.line(p[k], p[(k + 1) % 4], stroke);
    }
}

/// A point of the ellipse inscribed in `r`, at angle `k / n` turns.
fn ellipse_point(r: RectPx, k: usize, n: usize) -> PointPx {
    let t = core::f32::consts::TAU * k as f32 / n as f32;
    PointPx {
        x: r.x + r.w * 0.5 * (1.0 + t.cos()),
        y: r.y + r.h * 0.5 * (1.0 + t.sin()),
    }
}

fn write_duration(out: &mut impl Write, ms: i64) {
    let ms = ms.unsigned_abs();
    let (d, h, m) = (
        ms / 86_400_000,
        ms % 86_400_000 / 3_600_000,
        ms % 3_600_000 / 60_000,
    );
    let _ = match (d, h, m) {
        (0, 0, 0) => write!(out, "{}s", ms / 1000),
        (0, 0, m) => write!(out, "{m}m"),
        (0, h, 0) => write!(out, "{h}h"),
        (0, h, m) => write!(out, "{h}h {m}m"),
        (d, 0, _) => write!(out, "{d}d"),
        (d, h, _) => write!(out, "{d}d {h}h"),
    };
}

fn write_signed_price(ctx: &DrawingCtx<'_>, out: &mut impl Write, v: f64) {
    if v > 0.0 {
        let _ = out.write_char('+');
    }
    ctx.write_price(out, v);
}

impl Builtin {
    fn label(&self, ctx: &DrawingCtx<'_>, align: TextAlign, bg: Option<Rgba>) -> Label {
        Label {
            font: ctx.font(self.style.font_size),
            color: if bg.is_some() { WHITE } else { self.style.text },
            align,
            bg: bg.map(|color| LabelBg {
                color,
                padding: ctx.px(crate::geom::Css(3.0)),
            }),
        }
    }

    fn point(&self, ctx: &DrawingCtx<'_>, k: usize) -> Option<PointPx> {
        ctx.point(*self.anchors.get(k)?)
    }

    fn two(&self, ctx: &DrawingCtx<'_>) -> Option<(PointPx, PointPx)> {
        Some((self.point(ctx, 0)?, self.point(ctx, 1)?))
    }

    /// Ends of a line tool inside the plot.
    fn line_ends(&self, ctx: &DrawingCtx<'_>) -> Option<(PointPx, PointPx)> {
        let (p, q) = self.two(ctx)?;
        let (back, fwd) = match self.shape {
            Shape::Ray => (false, true),
            Shape::Extended => (true, true),
            _ => (false, false),
        };
        clip_line(p, q, ctx.plot(), back, fwd)
    }

    /// The two lines of a parallel channel: `a`–`b` and its copy through `c`.
    fn channel(&self, ctx: &DrawingCtx<'_>) -> Option<[PointPx; 4]> {
        let (a, b) = self.two(ctx)?;
        let c = self.point(ctx, 2)?;
        let line_y = if (b.x - a.x).abs() > f32::EPSILON {
            a.y + (b.y - a.y) * (c.x - a.x) / (b.x - a.x)
        } else {
            a.y
        };
        let dy = c.y - line_y;
        Some([
            a,
            b,
            PointPx {
                x: b.x,
                y: b.y + dy,
            },
            PointPx {
                x: a.x,
                y: a.y + dy,
            },
        ])
    }

    /// Entry, target and stop of a position (the stop mirrors the target
    /// while it is being created).
    fn position(&self) -> Option<(Anchor, Anchor, Anchor)> {
        let (e, t) = (*self.anchors.first()?, *self.anchors.get(1)?);
        let s = self.anchors.get(2).copied().unwrap_or(Anchor {
            time: t.time,
            price: 2.0 * e.price - t.price,
        });
        Some((e, t, s))
    }

    /// Screen geometry of a position: x range, entry, target and stop y.
    fn position_geom(&self, ctx: &DrawingCtx<'_>) -> Option<(f32, f32, f32, f32, f32)> {
        let (e, t, s) = self.position()?;
        let (x0, x1) = (ctx.x_of(e.time)?, ctx.x_of(t.time)?);
        let (x0, x1) = (x0.min(x1), x0.max(x1).max(x0.min(x1) + 1.0));
        Some((
            x0,
            x1,
            ctx.y_of(e.price)?,
            ctx.y_of(t.price)?,
            ctx.y_of(s.price)?,
        ))
    }

    /// Prices and screen y of the Fibonacci levels, and their x range.
    fn fib_x(&self, ctx: &DrawingCtx<'_>) -> Option<(f32, f32)> {
        let (a, b) = self.two(ctx)?;
        match self.shape {
            Shape::FibExtension => {
                let c = self.point(ctx, 2)?;
                let w = (b.x - a.x).abs().max(ctx.bar_spacing().0 * 10.0);
                Some((c.x, c.x + w))
            }
            _ => Some((a.x.min(b.x), a.x.max(b.x))),
        }
    }

    fn fib_price(&self, level: f64) -> Option<f64> {
        let (a, b) = (self.anchors.first()?, self.anchors.get(1)?);
        Some(match self.shape {
            Shape::FibExtension => self.anchors.get(2)?.price + (b.price - a.price) * level,
            _ => b.price + (a.price - b.price) * level,
        })
    }

    fn text_box(&self, ctx: &DrawingCtx<'_>) -> Option<RectPx> {
        let p = self.point(ctx, 0)?;
        let font = ctx.font(self.style.font_size);
        let (w, h) = (ctx.text_width(&self.text, font), ctx.line_height(font));
        let pad = ctx.px(crate::geom::Css(3.0)).0;
        Some(RectPx {
            x: p.x - pad,
            y: p.y - h * 0.5 - pad,
            w: w + 2.0 * pad,
            h: h + 2.0 * pad,
        })
    }

    fn build_fib(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        let Some((x0, x1)) = self.fib_x(ctx) else {
            return;
        };
        let mut prev: Option<f32> = None;
        for (k, &level) in self.levels.iter().enumerate() {
            let Some(y) = self.fib_price(level).and_then(|p| ctx.y_of(p)) else {
                continue;
            };
            if let Some(py) = prev {
                // Alternate the band shade.
                let a = if k % 2 == 0 {
                    self.style.fill.a
                } else {
                    self.style.fill.a / 2
                };
                out.rect(
                    RectPx {
                        x: x0,
                        y: py.min(y),
                        w: x1 - x0,
                        h: (py - y).abs(),
                    },
                    self.style.fill.with_alpha(a),
                );
            }
            prev = Some(y);
        }
        let label = self.label(ctx, TextAlign::Right, None);
        let gap = ctx.px(crate::geom::Css(LABEL_GAP)).0;
        for &level in &self.levels {
            let Some(price) = self.fib_price(level) else {
                continue;
            };
            let Some(y) = ctx.y_of(price) else {
                continue;
            };
            let y = crisp(y, stroke.width);
            out.line(PointPx { x: x0, y }, PointPx { x: x1, y }, stroke);
            let mut s = StackStr::<64>::new();
            let _ = write!(s, "{level} (");
            ctx.write_price(&mut s, price);
            let _ = s.write_char(')');
            out.text(PointPx { x: x0 - gap, y }, s.as_str(), label);
        }
        let dashed = StrokeStyle {
            dash: LineDash::Dashed,
            ..stroke
        };
        let pts = (0..self.shape.required()).filter_map(|k| self.point(ctx, k));
        out.polyline(pts, dashed);
    }

    fn build_position(
        &self,
        ctx: &DrawingCtx<'_>,
        out: &mut FrameBuilder<'_>,
        stroke: StrokeStyle,
        long: bool,
    ) {
        let (Some((x0, x1, ye, yt, ys)), Some((e, t, s))) =
            (self.position_geom(ctx), self.position())
        else {
            return;
        };
        let band = |y: f32| RectPx {
            x: x0,
            y: ye.min(y),
            w: x1 - x0,
            h: (ye - y).abs(),
        };
        out.rect(band(yt), PROFIT);
        out.rect(band(ys), LOSS);
        let y = crisp(ye, stroke.width);
        out.line(PointPx { x: x0, y }, PointPx { x: x1, y }, stroke);
        let sign = if long { 1.0 } else { -1.0 };
        let pct = |p: f64| sign * (p - e.price) / e.price.abs() * 100.0;
        let xc = (x0 + x1) * 0.5;
        let font = ctx.font(self.style.font_size);
        let h = ctx.line_height(font);
        let inside = |y: f32| {
            if y < ye {
                y + h * 0.5 + 2.0
            } else {
                y - h * 0.5 - 2.0
            }
        };
        for (name, a, y, color) in [("Target", t, yt, PROFIT), ("Stop", s, ys, LOSS)] {
            let mut label = StackStr::<64>::new();
            let _ = write!(label, "{name} ");
            ctx.write_price(&mut label, a.price);
            let _ = write!(label, " ({:+.2}%)", pct(a.price));
            let pos = PointPx {
                x: xc,
                y: inside(y),
            };
            out.text(
                pos,
                label.as_str(),
                self.label(ctx, TextAlign::Center, Some(color.with_alpha(230))),
            );
        }
        let risk = (e.price - s.price).abs();
        let mut ratio = StackStr::<32>::new();
        if risk > 0.0 {
            let _ = write!(ratio, "R/R {:.2}", (t.price - e.price).abs() / risk);
        }
        if !ratio.as_str().is_empty() {
            out.text(
                PointPx { x: xc, y: ye },
                ratio.as_str(),
                self.label(ctx, TextAlign::Center, Some(self.style.line)),
            );
        }
    }

    fn build_range(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>, stroke: StrokeStyle) {
        let Some((p, q)) = self.two(ctx) else {
            return;
        };
        let r = rect_of(p, q);
        out.rect(r, self.style.fill);
        let head = ctx.px(crate::geom::Css(5.0)).0;
        let mut s = StackStr::<64>::new();
        let (from, to, label_y) = if self.shape == Shape::PriceRange {
            let x = crisp(r.x + r.w * 0.5, stroke.width);
            let (from, to) = (PointPx { x, y: p.y }, PointPx { x, y: q.y });
            let (a, b) = (self.anchors[0].price, self.anchors[1].price);
            write_signed_price(ctx, &mut s, b - a);
            if a != 0.0 {
                let _ = write!(s, " ({:+.2}%)", (b - a) / a.abs() * 100.0);
            }
            let h = ctx.line_height(ctx.font(self.style.font_size));
            let off = if q.y <= p.y { -h } else { h };
            (from, to, q.y + off)
        } else {
            let y = crisp(r.y + r.h * 0.5, stroke.width);
            let (from, to) = (PointPx { x: p.x, y }, PointPx { x: q.x, y });
            let bars = match (
                ctx.index_of_time(self.anchors[0].time),
                ctx.index_of_time(self.anchors[1].time),
            ) {
                (Some(a), Some(b)) => (b - a).round() as i64,
                _ => 0,
            };
            let _ = write!(s, "{bars} bars, ");
            write_duration(&mut s, self.anchors[1].time - self.anchors[0].time);
            let h = ctx.line_height(ctx.font(self.style.font_size));
            (from, to, r.bottom() + h)
        };
        out.line(from, to, stroke);
        // Arrow head at `to`.
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let len = dx.hypot(dy);
        if len > head {
            let (ux, uy) = (dx / len, dy / len);
            for side in [-1.0, 1.0] {
                let tip = PointPx {
                    x: to.x - head * ux + side * head * 0.6 * uy,
                    y: to.y - head * uy - side * head * 0.6 * ux,
                };
                out.line(to, tip, stroke);
            }
        }
        out.text(
            PointPx {
                x: r.x + r.w * 0.5,
                y: label_y,
            },
            s.as_str(),
            self.label(ctx, TextAlign::Center, Some(self.style.line)),
        );
    }
}

impl DrawingTool for Builtin {
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
        AnchorCount::Fixed(self.shape.required())
    }

    fn style(&self) -> &DrawingStyle {
        &self.style
    }

    fn save(&self) -> DrawingData {
        let mut d = DrawingData::new(self.kind, self.anchors.clone());
        d.style = self.style;
        if self.shape == Shape::Text {
            d.set("text", Param::Text(self.text.clone()));
        }
        if is_fib(self.shape) {
            d.set("levels", Param::Numbers(self.levels.clone()));
        }
        d
    }

    fn finish(&mut self) {
        if matches!(self.shape, Shape::Position { .. })
            && self.anchors.len() == 2
            && let Some((_, _, s)) = self.position()
        {
            self.anchors.push(s);
        }
    }

    fn move_handle(&mut self, handle: usize, to: Anchor) {
        if !matches!(self.shape, Shape::Position { .. }) {
            if let Some(a) = self.anchors.get_mut(handle) {
                *a = to;
            }
            return;
        }
        self.finish();
        match handle {
            0 => {
                if let Some(a) = self.anchors.get_mut(0) {
                    *a = to;
                }
            }
            1 | 2 => {
                if let Some(a) = self.anchors.get_mut(handle) {
                    a.price = to.price;
                }
            }
            3 => {
                for a in self.anchors.iter_mut().skip(1) {
                    a.time = to.time;
                }
            }
            _ => {}
        }
    }

    fn build(&self, ctx: &DrawingCtx<'_>, out: &mut FrameBuilder<'_>) {
        let stroke = ctx.stroke(&self.style);
        let plot = ctx.plot();
        match self.shape {
            Shape::Trend | Shape::Ray | Shape::Extended => {
                if let Some((p, q)) = self.line_ends(ctx) {
                    out.line(p, q, stroke);
                }
            }
            Shape::HLine | Shape::HRay => {
                let Some(y) = self.anchors.first().and_then(|a| ctx.y_of(a.price)) else {
                    return;
                };
                let y = crisp(y, stroke.width);
                let x0 = if self.shape == Shape::HRay {
                    match self.point(ctx, 0) {
                        Some(p) => p.x.max(plot.x),
                        None => return,
                    }
                } else {
                    plot.x
                };
                if x0 < plot.right() {
                    out.line(PointPx { x: x0, y }, PointPx { x: plot.right(), y }, stroke);
                }
                let mut s = StackStr::<32>::new();
                ctx.write_price(&mut s, self.anchors[0].price);
                let gap = ctx.px(crate::geom::Css(LABEL_GAP)).0;
                let h = ctx.line_height(ctx.font(self.style.font_size));
                out.text(
                    PointPx {
                        x: plot.right() - gap,
                        y: y - h * 0.5 - gap * 0.5,
                    },
                    s.as_str(),
                    self.label(ctx, TextAlign::Right, None),
                );
            }
            Shape::VLine => {
                let Some(x) = self.anchors.first().and_then(|a| ctx.x_of(a.time)) else {
                    return;
                };
                let x = crisp(x, stroke.width);
                out.line(
                    PointPx { x, y: plot.y },
                    PointPx {
                        x,
                        y: plot.bottom(),
                    },
                    stroke,
                );
            }
            Shape::Channel => {
                let Some(q) = self.channel(ctx) else {
                    return;
                };
                out.polygon(q, self.style.fill);
                out.line(q[0], q[1], stroke);
                out.line(q[3], q[2], stroke);
                let mid = |a: PointPx, b: PointPx| PointPx {
                    x: (a.x + b.x) * 0.5,
                    y: (a.y + b.y) * 0.5,
                };
                let dashed = StrokeStyle {
                    dash: LineDash::Dashed,
                    ..stroke
                };
                out.line(mid(q[0], q[3]), mid(q[1], q[2]), dashed);
            }
            Shape::Rect => {
                if let Some((p, q)) = self.two(ctx) {
                    let r = rect_of(p, q);
                    out.rect(r, self.style.fill);
                    outline(out, r, stroke);
                }
            }
            Shape::Ellipse => {
                if let Some((p, q)) = self.two(ctx) {
                    let r = rect_of(p, q);
                    let n = ELLIPSE_SEGMENTS;
                    out.polygon((0..n).map(|k| ellipse_point(r, k, n)), self.style.fill);
                    out.polyline((0..=n).map(|k| ellipse_point(r, k, n)), stroke);
                }
            }
            Shape::Text => {
                let Some(p) = self.point(ctx, 0) else {
                    return;
                };
                let label = Label {
                    font: ctx.font(self.style.font_size),
                    color: self.style.text,
                    align: TextAlign::Left,
                    bg: (self.style.fill.a > 0).then(|| LabelBg {
                        color: self.style.fill,
                        padding: ctx.px(crate::geom::Css(3.0)),
                    }),
                };
                out.text(p, &self.text, label);
            }
            Shape::PriceRange | Shape::DateRange => self.build_range(ctx, out, stroke),
            Shape::FibRetracement | Shape::FibExtension => self.build_fib(ctx, out, stroke),
            Shape::Position { long } => self.build_position(ctx, out, stroke, long),
        }
    }

    fn hit_test(&self, ctx: &DrawingCtx<'_>, p: PointPx) -> Option<HitPart> {
        let tol = ctx.hit_tolerance();
        let plot = ctx.plot();
        let filled = self.style.fill.a > 0;
        let hit = match self.shape {
            Shape::Trend | Shape::Ray | Shape::Extended => self
                .line_ends(ctx)
                .is_some_and(|(a, b)| distance_to_segment(p, a, b) <= tol),
            Shape::HLine | Shape::HRay => {
                let x0 = if self.shape == Shape::HRay {
                    self.point(ctx, 0).map_or(f32::INFINITY, |a| a.x)
                } else {
                    plot.x
                };
                self.anchors
                    .first()
                    .and_then(|a| ctx.y_of(a.price))
                    .is_some_and(|y| (p.y - y).abs() <= tol && p.x >= x0 - tol)
            }
            Shape::VLine => self
                .anchors
                .first()
                .and_then(|a| ctx.x_of(a.time))
                .is_some_and(|x| (p.x - x).abs() <= tol),
            Shape::Channel => self.channel(ctx).is_some_and(|q| {
                distance_to_segment(p, q[0], q[1]) <= tol
                    || distance_to_segment(p, q[3], q[2]) <= tol
                    || (filled && point_in_polygon(p, &q))
            }),
            Shape::Rect => self.two(ctx).is_some_and(|(a, b)| {
                let r = rect_of(a, b);
                near_rect_border(p, r, tol) || (filled && r.contains(p))
            }),
            Shape::Ellipse => self.two(ctx).is_some_and(|(a, b)| {
                let r = rect_of(a, b);
                let (rx, ry) = ((r.w * 0.5).max(0.5), (r.h * 0.5).max(0.5));
                let (dx, dy) = (p.x - r.x - rx, p.y - r.y - ry);
                let d = (dx / rx).hypot(dy / ry);
                (d - 1.0).abs() * rx.min(ry) <= tol || (filled && d <= 1.0)
            }),
            Shape::Text => self.text_box(ctx).is_some_and(|r| r.contains(p)),
            Shape::PriceRange | Shape::DateRange => self
                .two(ctx)
                .is_some_and(|(a, b)| inflate(rect_of(a, b), tol).contains(p)),
            Shape::FibRetracement | Shape::FibExtension => {
                let ys = self
                    .levels
                    .iter()
                    .filter_map(|&l| ctx.y_of(self.fib_price(l)?));
                let (lo, hi) = ys.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), y| {
                    (lo.min(y), hi.max(y))
                });
                self.fib_x(ctx).is_some_and(|(x0, x1)| {
                    lo <= hi
                        && inflate(
                            RectPx {
                                x: x0,
                                y: lo,
                                w: x1 - x0,
                                h: hi - lo,
                            },
                            tol,
                        )
                        .contains(p)
                })
            }
            Shape::Position { .. } => self.position_geom(ctx).is_some_and(|(x0, x1, _, yt, ys)| {
                let r = RectPx {
                    x: x0,
                    y: yt.min(ys),
                    w: x1 - x0,
                    h: (yt - ys).abs(),
                };
                inflate(r, tol).contains(p)
            }),
        };
        hit.then_some(HitPart::Body)
    }

    fn handles(&self, ctx: &DrawingCtx<'_>, out: &mut Vec<PointPx>) {
        let plot = ctx.plot();
        match self.shape {
            Shape::Position { .. } => {
                if let Some((x0, x1, ye, yt, ys)) = self.position_geom(ctx) {
                    let x_entry = self.point(ctx, 0).map_or(x0, |p| p.x);
                    out.push(PointPx { x: x_entry, y: ye });
                    out.push(PointPx { x: x0, y: yt });
                    out.push(PointPx { x: x0, y: ys });
                    out.push(PointPx { x: x1, y: ye });
                }
            }
            Shape::HLine => {
                if let Some(y) = self.anchors.first().and_then(|a| ctx.y_of(a.price)) {
                    out.push(PointPx {
                        x: plot.x + plot.w * 0.5,
                        y,
                    });
                }
            }
            Shape::VLine => {
                if let Some(x) = self.anchors.first().and_then(|a| ctx.x_of(a.time)) {
                    out.push(PointPx {
                        x,
                        y: plot.y + plot.h * 0.5,
                    });
                }
            }
            _ => {
                for k in 0..self.anchors.len() {
                    if let Some(p) = self.point(ctx, k) {
                        out.push(p);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_saved_form() {
        for kind in FIRST_KINDS {
            let mut data = DrawingData::new(kind, Vec::new());
            let tool = builtin(&data).unwrap_or_else(|e| panic!("{kind}: {e:?}"));
            let n = tool.anchor_count().min();
            data.anchors = (0..n)
                .map(|k| Anchor {
                    time: k as i64 * 1000,
                    price: 100.0 + k as f64,
                })
                .collect();
            let mut tool = builtin(&data).unwrap_or_else(|e| panic!("{kind}: {e:?}"));
            tool.finish();
            let saved = tool.save();
            let again = builtin(&saved).map(|t| t.save());
            assert_eq!(again.as_ref(), Ok(&saved), "{kind}");
        }
    }

    #[test]
    fn parameters_are_validated() {
        let mut d = DrawingData::new("fib_retracement", Vec::new());
        d.set("levels", Param::Numbers(vec![0.0, f64::NAN]));
        assert!(builtin(&d).is_err());
        d.set("levels", Param::Text("x".into()));
        assert!(builtin(&d).is_err());
        let mut t = DrawingData::new("text", Vec::new());
        t.set("text", Param::Number(1.0));
        assert!(builtin(&t).is_err());
        let three = vec![
            Anchor {
                time: 0,
                price: 1.0
            };
            3
        ];
        assert!(builtin(&DrawingData::new("trend_line", three)).is_err());
        assert!(builtin(&DrawingData::new("nope", Vec::new())).is_err());
    }

    #[test]
    fn positions_mirror_the_stop() {
        let anchors = vec![
            Anchor {
                time: 0,
                price: 100.0,
            },
            Anchor {
                time: 10,
                price: 110.0,
            },
        ];
        let mut p = builtin(&DrawingData::new("long_position", anchors)).unwrap_or_else(|_| {
            panic!("position");
        });
        p.finish();
        assert_eq!(
            p.anchors()[2],
            Anchor {
                time: 10,
                price: 90.0
            }
        );
        p.move_handle(
            3,
            Anchor {
                time: 20,
                price: 0.0,
            },
        );
        assert_eq!((p.anchors()[1].time, p.anchors()[2].time), (20, 20));
        p.move_handle(
            2,
            Anchor {
                time: 99,
                price: 95.0,
            },
        );
        assert_eq!(
            p.anchors()[2],
            Anchor {
                time: 20,
                price: 95.0
            }
        );
    }

    #[test]
    fn durations_are_short() {
        let f = |ms| {
            let mut s = StackStr::<32>::new();
            write_duration(&mut s, ms);
            s.as_str().to_owned()
        };
        assert_eq!(f(90 * 60_000), "1h 30m");
        assert_eq!(f(-3 * 86_400_000 - 4 * 3_600_000), "3d 4h");
        assert_eq!(f(45 * 60_000), "45m");
        assert_eq!(f(5_000), "5s");
    }
}
