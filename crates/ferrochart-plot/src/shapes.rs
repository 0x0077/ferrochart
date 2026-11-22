//! Areas, bars, waterfalls and cells turned into immediate frame primitives.
//!
//! Areas are emitted as one convex piece per segment (renderers fill
//! polygons as triangle fans): a trapezoid, or two triangles where the edge
//! crosses its base so each side takes its own color.
//!

use core::fmt::Write as _;

use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::frame::{Frame, LayerId, LineDash, StrokeStyle, TextAlign};
use ferrochart_core::geom::{Css, PointPx, Px, RectPx, Rgba};
use ferrochart_core::text::{StackStr, text_width};
use ferrochart_core::theme::Theme;

use crate::axis::{Axis, AxisKind, value_format};
use crate::color::contrast;
use crate::draw::{Ctx, Scratch, clamp_px, flush, visible_range};
use crate::mark::{MarkEntry, MarkKind, WaterfallStep};

const GAP: [f32; 3] = [f32::NAN; 3];

fn pt(x: f32, y: f32) -> PointPx {
    PointPx { x, y }
}

/// Pixel position of `v` on `a`; values without one (≤ 0 on a log axis)
/// go to the low end of the view.
fn edge_px(a: &Axis, v: f64) -> f32 {
    let p = a.to_px(v);
    clamp_px(if p.is_finite() {
        p
    } else {
        a.to_px(a.view.0.min(a.view.1))
    })
}

/// The pixel span between `a` and `b` with rounded edges, at least 1 px.
fn snap(a: f32, b: f32) -> (f32, f32) {
    let (lo, hi) = (a.min(b).round(), a.max(b).round());
    (lo, if hi - lo < 1.0 { lo + 1.0 } else { hi })
}

/// Half width of a slot in data units and the offset of its center from the
/// data value: categories are 1 wide and centered at `k + 0.5`, numeric
/// slots are `gap` wide and centered at the value.
fn slot(a: &Axis, gap: f64) -> (f64, f64) {
    if a.opts.kind == AxisKind::Category {
        (0.5, 0.5)
    } else {
        (gap * 0.5, 0.0)
    }
}

/// A rectangle from a span along the category axis and one along the value
/// axis.
fn rect_of(horizontal: bool, cat: (f32, f32), val: (f32, f32)) -> RectPx {
    let (c, v) = (cat, val);
    if horizontal {
        RectPx {
            x: v.0,
            y: c.0,
            w: v.1 - v.0,
            h: c.1 - c.0,
        }
    } else {
        RectPx {
            x: c.0,
            y: v.0,
            w: c.1 - c.0,
            h: v.1 - v.0,
        }
    }
}

fn intersects(a: RectPx, b: RectPx) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// The value column of `m` (x for horizontal bars).
fn values(m: &MarkEntry) -> &[f64] {
    if m.horizontal() { &m.data.x } else { &m.data.y }
}

/// Where point `i` of an unstacked bar or area starts: `y0`, or the base value.
fn lower(m: &MarkEntry, i: usize) -> f64 {
    m.data
        .y0
        .as_ref()
        .and_then(|c| c.get(i))
        .copied()
        .filter(|v| !v.is_nan())
        .unwrap_or(m.opts.base_value)
}

/// Sum at index `i` of the visible marks of the same kind and stack drawn
/// before `m`. Bars add positive and negative values apart (`v` picks the
/// side); areas add everything.
fn stack_base(all: &[MarkEntry], m: &MarkEntry, i: usize, v: f64) -> f64 {
    let mut sum = 0.0;
    for o in all {
        if o.id == m.id {
            break;
        }
        if o.kind != m.kind || o.opts.stack != m.opts.stack || !o.opts.visible {
            continue;
        }
        let w = values(o).get(i).copied().unwrap_or(f64::NAN);
        if w.is_finite() && (m.kind == MarkKind::Area || (w >= 0.0) == (v >= 0.0)) {
            sum += w;
        }
    }
    sum
}

/// Column of `m` among the bars of its group, and the number of columns:
/// bars of one stack share a column, other bars get their own.
fn group_column(all: &[MarkEntry], m: &MarkEntry) -> (usize, usize) {
    let Some(g) = m.opts.group else {
        return (0, 1);
    };
    let key = |o: &MarkEntry| o.opts.stack.map_or((1, o.id.0), |s| (0, s));
    let member = |o: &&MarkEntry| {
        o.kind == MarkKind::Bars
            && o.opts.visible
            && o.opts.group == Some(g)
            && o.horizontal() == m.horizontal()
    };
    let (mut n, mut col) = (0, 0);
    for (k, o) in all.iter().filter(member).enumerate() {
        if all.iter().filter(member).take(k).any(|p| key(p) == key(o)) {
            continue;
        }
        if key(o) == key(m) {
            col = n;
        }
        n += 1;
    }
    (col, n.max(1))
}

/// Writes the text of point `i`: its label, or `v` in the mark's value format.
fn value_text<'a>(m: &'a MarkEntry, i: usize, v: f64, buf: &'a mut StackStr<64>) -> &'a str {
    if let Some(t) = m.data.label.as_ref().and_then(|l| l.get(i)) {
        return t;
    }
    value_format(m.opts.value_format, v).write(buf, v);
    buf.as_str()
}

/// `text` centered in `rect` in black or white for `bg`, if it fits.
fn text_inside(ctx: &Ctx<'_>, b: &mut FrameBuilder<'_>, rect: RectPx, text: &str, bg: Rgba) {
    if text.is_empty() {
        return;
    }
    let w = text_width(ctx.text, text, ctx.font);
    if w + ctx.pad <= rect.w && ctx.line_h <= rect.h {
        let label = Label {
            font: ctx.font,
            color: contrast(bg),
            align: TextAlign::Center,
            bg: None,
        };
        b.text(
            pt(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5),
            text,
            label,
        );
    }
}

/// `text` beyond the end of a bar that runs from pixel `fp` to `tp` along
/// its value axis, if it fits in `avail` px across the bar.
#[allow(clippy::too_many_arguments)]
fn text_beyond(
    ctx: &Ctx<'_>,
    b: &mut FrameBuilder<'_>,
    horizontal: bool,
    rect: RectPx,
    (fp, tp): (f32, f32),
    avail: f32,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let gap = (ctx.pad * 0.5).round();
    let label = |align| Label {
        font: ctx.font,
        color: ctx.theme.text,
        align,
        bg: None,
    };
    if horizontal {
        if ctx.line_h > avail {
            return;
        }
        let y = rect.y + rect.h * 0.5;
        if tp >= fp {
            b.text(pt(rect.right() + gap, y), text, label(TextAlign::Left));
        } else {
            b.text(pt(rect.x - gap, y), text, label(TextAlign::Right));
        }
    } else {
        if text_width(ctx.text, text, ctx.font) > avail {
            return;
        }
        let x = rect.x + rect.w * 0.5;
        let y = if tp <= fp {
            rect.y - gap - ctx.line_h * 0.5
        } else {
            rect.bottom() + gap + ctx.line_h * 0.5
        };
        b.text(pt(x, y), text, label(TextAlign::Center));
    }
}

/// One bar.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bar {
    pub rect: RectPx,
    pub value: f64,
    pub from: f64,
    pub to: f64,
    /// Pixel positions of `from` and `to` along the value axis.
    pub px: (f32, f32),
}

/// Where the bars of a mark go: the same for drawing and hit testing.
pub(crate) struct BarGeom<'a> {
    all: &'a [MarkEntry],
    m: &'a MarkEntry,
    /// Category (or numeric slot) axis and value axis.
    pub ca: &'a Axis,
    va: &'a Axis,
    plot: RectPx,
    half: f64,
    off: f64,
    width: f64,
    col: usize,
    stacked: bool,
}

impl<'a> BarGeom<'a> {
    pub fn new(
        x: &'a Axis,
        ya: &'a Axis,
        plot: RectPx,
        all: &'a [MarkEntry],
        m: &'a MarkEntry,
    ) -> Self {
        let hz = m.horizontal();
        let (ca, va) = if hz { (ya, x) } else { (x, ya) };
        let (half, off) = slot(ca, if hz { m.cell.1 } else { m.cell.0 });
        let half = half * m.band();
        let (col, ncol) = group_column(all, m);
        Self {
            all,
            m,
            ca,
            va,
            plot,
            half,
            off,
            width: 2.0 * half / ncol as f64,
            col,
            stacked: m.opts.stack.is_some(),
        }
    }

    /// The category (slot) column.
    pub fn cats(&self) -> &'a [f64] {
        if self.m.horizontal() {
            &self.m.data.y
        } else {
            &self.m.data.x
        }
    }

    /// Half the slot in data units plus the offset of its center: bars of
    /// point `i` lie within `cats[i] + off ± reach` along the slot axis.
    pub fn reach(&self) -> (f64, f64) {
        (self.half, self.off)
    }

    /// Bar `i` if it has one and it meets the plot.
    pub fn bar(&self, i: usize) -> Option<Bar> {
        let m = self.m;
        let c = self.cats().get(i).copied()?;
        let v = values(m).get(i).copied()?;
        if !(c.is_finite() && v.is_finite()) {
            return None;
        }
        let (from, to) = if self.stacked {
            let base = stack_base(self.all, m, i, v);
            (base, base + v)
        } else {
            (lower(m, i), v)
        };
        let left = c + self.off - self.half + self.width * self.col as f64;
        let cs = snap(
            clamp_px(self.ca.to_px(left)),
            clamp_px(self.ca.to_px(left + self.width)),
        );
        if !(cs.0.is_finite() && cs.1.is_finite()) {
            return None;
        }
        let px = (edge_px(self.va, from), edge_px(self.va, to));
        let rect = rect_of(m.horizontal(), cs, snap(px.0, px.1));
        intersects(rect, self.plot).then_some(Bar {
            rect,
            value: v,
            from,
            to,
            px,
        })
    }
}

/// Bars: from `y0` (or the base value, or the stack below) to `y`, side by
/// side within a group, on category or numeric axes, vertical or horizontal.
pub(crate) fn bars(ctx: &Ctx<'_>, all: &[MarkEntry], m: &MarkEntry, frame: &mut Frame) {
    let hz = m.horizontal();
    let g = BarGeom::new(ctx.x, ctx.axis(m.opts.y_axis), ctx.plot, all, m);
    let (i0, i1) = if hz {
        (0, m.data.len())
    } else {
        visible_range(g.cats(), m.x_sorted, g.ca.view)
    };
    let color = |i: usize, bar: &Bar| match m.opts.down_color {
        Some(c) if bar.to < bar.from => c,
        _ => m.point_color(i),
    };
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    for i in i0..i1 {
        if let Some(r) = g.bar(i) {
            b.rect(r.rect, color(i, &r));
        }
    }
    if !m.opts.show_values {
        return;
    }
    let band = m.band() as f32;
    for i in i0..i1 {
        let Some(r) = g.bar(i) else { continue };
        let mut buf = StackStr::<64>::new();
        let t = value_text(m, i, r.value, &mut buf);
        if g.stacked {
            text_inside(ctx, &mut b, r.rect, t, color(i, &r));
        } else {
            let across = if hz { r.rect.h } else { r.rect.w };
            text_beyond(ctx, &mut b, hz, r.rect, r.px, across / band, t);
        }
    }
}

/// Areas between `y` and `y0` (the base value, or the stack below). With
/// `down_color`, parts where `y` is below the lower edge take it.
pub(crate) fn area(
    ctx: &Ctx<'_>,
    all: &[MarkEntry],
    m: &MarkEntry,
    s: &mut Scratch,
    frame: &mut Frame,
) {
    let d = &m.data;
    let ya = ctx.axis(m.opts.y_axis);
    let (i0, i1) = visible_range(&d.x, m.x_sorted, ctx.x.view);
    let step = m.opts.step_line;
    let decimate = m.x_sorted && !step && (i1 - i0) as f32 > 4.0 * ctx.plot.w;
    let stacked = m.opts.stack.is_some();
    // In pixels, `(lower - upper) * o > 0` where the value is above the
    // lower edge.
    let o: f32 = if ya.opts.inverted { -1.0 } else { 1.0 };
    s.area.clear();
    // Decimation keeps, per pixel column, the point farthest from its lower
    // edge.
    let mut col: Option<(i64, [f32; 3])> = None;
    for i in i0..i1 {
        let v = d.y[i];
        let x = clamp_px(ctx.x.point_px(d.x[i]));
        if !(v.is_finite() && x.is_finite()) {
            if let Some((_, q)) = col.take() {
                s.area.push(q);
            }
            s.area.push(GAP);
            continue;
        }
        let (lo, hi) = if stacked {
            let base = stack_base(all, m, i, v);
            (base, base + v)
        } else {
            (lower(m, i), v)
        };
        let p = [x, edge_px(ya, hi), edge_px(ya, lo)];
        if !decimate {
            s.area.push(p);
            continue;
        }
        let cx = x.floor() as i64;
        match &mut col {
            Some((c, q)) if *c == cx => {
                if (p[2] - p[1]).abs() > (q[2] - q[1]).abs() {
                    *q = p;
                }
            }
            _ => {
                if let Some((_, q)) = col.replace((cx, p)) {
                    s.area.push(q);
                }
            }
        }
    }
    if let Some((_, q)) = col {
        s.area.push(q);
    }
    let color = m.color();
    let alpha = |c: Rgba| {
        let a = (f32::from(c.a) * m.opts.fill_alpha.clamp(0.0, 1.0)).round();
        c.with_alpha(a as u8)
    };
    let (up, down) = (alpha(color), alpha(m.opts.down_color.unwrap_or(color)));
    let fill = |f: f32| if f >= 0.0 { up } else { down };
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    for w in s.area.windows(2) {
        let ([x0, u0, l0], [x1, u1, l1]) = (w[0], w[1]);
        if x0.is_nan() || x1.is_nan() {
            continue;
        }
        // A step keeps its value to the next x; the lower edge (a stack
        // below) still runs straight between the points.
        let u1 = if step { u0 } else { u1 };
        let (f0, f1) = ((l0 - u0) * o, (l1 - u1) * o);
        if (f0 >= 0.0) == (f1 >= 0.0) {
            b.polygon(
                [pt(x0, u0), pt(x1, u1), pt(x1, l1), pt(x0, l0)],
                fill(f0 + f1),
            );
        } else {
            let t = f0 / (f0 - f1);
            let c = pt(x0 + t * (x1 - x0), u0 + t * (u1 - u0));
            b.polygon([pt(x0, u0), c, pt(x0, l0)], fill(f0));
            b.polygon([c, pt(x1, u1), pt(x1, l1)], fill(f1));
        }
    }
    // The upper edge.
    let lw = ctx.px(m.opts.line_width);
    if lw <= 0.0 {
        return;
    }
    let stroke = StrokeStyle {
        width: Px(lw),
        color,
        dash: m.opts.line_style,
    };
    s.pts.clear();
    let mut prev: Option<PointPx> = None;
    for &[x, u, _] in &s.area {
        if x.is_nan() {
            flush(&mut b, &mut s.pts, stroke);
            prev = None;
            continue;
        }
        let p = pt(x, u);
        if step && let Some(q) = prev {
            s.pts.push(pt(p.x, q.y));
        }
        s.pts.push(p);
        prev = Some(p);
    }
    flush(&mut b, &mut s.pts, stroke);
}

/// Calls `f(i, from, to, shown, color)` for the items of a waterfall in data
/// order: deltas run from the running total to the new total, subtotals and
/// totals from 0 to the running total. `shown` is the value written on the
/// item; `prev` is the running total before it.
pub(crate) fn waterfall_items(
    theme: &Theme,
    m: &MarkEntry,
    mut f: impl FnMut(usize, f64, f64, f64, f64, Rgba),
) {
    let up = m.opts.color.unwrap_or(theme.up);
    let down = m.opts.down_color.unwrap_or(theme.down);
    let total_color = m.opts.total_color.unwrap_or(m.auto_color);
    let vals = values(m);
    let mut total = 0.0_f64;
    for (i, &v) in vals.iter().enumerate() {
        let step = m
            .data
            .step
            .as_ref()
            .and_then(|s| s.get(i))
            .copied()
            .unwrap_or_default();
        let prev = total;
        match step {
            WaterfallStep::Delta => {
                if !v.is_finite() {
                    continue;
                }
                total += v;
                f(i, prev, total, v, prev, if v >= 0.0 { up } else { down });
            }
            WaterfallStep::Subtotal | WaterfallStep::Total => {
                f(i, 0.0, total, total, prev, total_color);
            }
        }
    }
}

/// The slot axis, value axis and slot span (px) of item `i` of a waterfall.
pub(crate) struct WaterfallGeom<'a> {
    pub ca: &'a Axis,
    pub va: &'a Axis,
    cats: &'a [f64],
    half: f64,
    off: f64,
}

impl<'a> WaterfallGeom<'a> {
    pub fn new(x: &'a Axis, ya: &'a Axis, m: &'a MarkEntry) -> Self {
        let hz = m.horizontal();
        let (ca, va) = if hz { (ya, x) } else { (x, ya) };
        let (half, off) = slot(ca, if hz { m.cell.1 } else { m.cell.0 });
        Self {
            ca,
            va,
            cats: if hz { &m.data.y } else { &m.data.x },
            half: half * m.band(),
            off,
        }
    }

    pub fn span(&self, i: usize) -> (f32, f32) {
        let left = self.cats.get(i).copied().unwrap_or(f64::NAN) + self.off - self.half;
        snap(
            clamp_px(self.ca.to_px(left)),
            clamp_px(self.ca.to_px(left + 2.0 * self.half)),
        )
    }

    /// The bar of an item running from `from` to `to`.
    pub fn rect(&self, horizontal: bool, i: usize, from: f64, to: f64) -> RectPx {
        rect_of(
            horizontal,
            self.span(i),
            snap(edge_px(self.va, from), edge_px(self.va, to)),
        )
    }
}

/// Waterfall bars with dashed connectors at the running total.
pub(crate) fn waterfall(ctx: &Ctx<'_>, m: &MarkEntry, frame: &mut Frame) {
    let d = &m.data;
    let hz = m.horizontal();
    let g = WaterfallGeom::new(ctx.x, ctx.axis(m.opts.y_axis), m);
    let va = g.va;
    let connector = StrokeStyle {
        width: Px(ctx.px(Css(1.0)).max(1.0)),
        color: ctx.theme.crosshair,
        dash: LineDash::Dashed,
    };
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    let mut last: Option<(f32, f32)> = None;
    waterfall_items(ctx.theme, m, |i, from, to, _, prev, color| {
        let cs = g.span(i);
        if !(cs.0.is_finite() && cs.1.is_finite()) {
            return;
        }
        if let Some(p) = last {
            let lvl = edge_px(va, prev).floor() + 0.5;
            let (a, z) = if p.1 <= cs.0 {
                (p.1, cs.0)
            } else {
                (cs.1, p.0)
            };
            if hz {
                b.line(pt(lvl, a), pt(lvl, z), connector);
            } else {
                b.line(pt(a, lvl), pt(z, lvl), connector);
            }
        }
        last = Some(cs);
        let rect = g.rect(hz, i, from, to);
        if intersects(rect, ctx.plot) {
            b.rect(rect, color);
        }
    });
    if !m.opts.show_values {
        return;
    }
    let band = m.band() as f32;
    waterfall_items(ctx.theme, m, |i, from, to, shown, _, _| {
        let px = (edge_px(va, from), edge_px(va, to));
        let rect = g.rect(hz, i, from, to);
        if !intersects(rect, ctx.plot) {
            return;
        }
        let mut buf = StackStr::<64>::new();
        let delta = d
            .step
            .as_ref()
            .and_then(|s| s.get(i))
            .copied()
            .unwrap_or_default()
            == WaterfallStep::Delta;
        if delta && shown > 0.0 && d.label.is_none() {
            let _ = buf.write_char('+');
        }
        let t = value_text(m, i, shown, &mut buf);
        let across = if hz { rect.h } else { rect.w };
        text_beyond(ctx, &mut b, hz, rect, px, across / band, t);
    });
}

/// Where the cells of a mark go.
pub(crate) struct CellGeom<'a> {
    x: &'a Axis,
    ya: &'a Axis,
    plot: RectPx,
    m: &'a MarkEntry,
    /// Half sizes and center offsets in data units.
    pub hx: f64,
    pub hy: f64,
    pub ox: f64,
    pub oy: f64,
}

impl<'a> CellGeom<'a> {
    pub fn new(x: &'a Axis, ya: &'a Axis, plot: RectPx, m: &'a MarkEntry) -> Self {
        let band = m.band();
        let (hx, ox) = slot(x, m.cell.0);
        let (hy, oy) = slot(ya, m.cell.1);
        Self {
            x,
            ya,
            plot,
            m,
            hx: hx * band,
            hy: hy * band,
            ox,
            oy,
        }
    }

    /// Whether cell `i` is drawn: a non-zero color, or a finite value, or no
    /// value column.
    pub fn shown(&self, i: usize) -> bool {
        let d = &self.m.data;
        if d.color
            .as_ref()
            .and_then(|c| c.get(i))
            .is_some_and(|&c| c != 0)
        {
            return true;
        }
        match d.value.as_ref() {
            Some(v) => self.m.domain.is_some() && v.get(i).is_some_and(|v| v.is_finite()),
            None => true,
        }
    }

    /// The rectangle of cell `i` if it is shown and meets the plot.
    pub fn rect(&self, i: usize) -> Option<RectPx> {
        let d = &self.m.data;
        if !self.shown(i) {
            return None;
        }
        let (cx, cy) = (d.x.get(i)? + self.ox, d.y.get(i)? + self.oy);
        let xs = snap(
            clamp_px(self.x.to_px(cx - self.hx)),
            clamp_px(self.x.to_px(cx + self.hx)),
        );
        let ys = snap(
            clamp_px(self.ya.to_px(cy - self.hy)),
            clamp_px(self.ya.to_px(cy + self.hy)),
        );
        if !(xs.0.is_finite() && xs.1.is_finite() && ys.0.is_finite() && ys.1.is_finite()) {
            return None;
        }
        let rect = RectPx {
            x: xs.0,
            y: ys.0,
            w: xs.1 - xs.0,
            h: ys.1 - ys.0,
        };
        intersects(rect, self.plot).then_some(rect)
    }
}

/// Cells of a grid colored by value (heatmaps).
pub(crate) fn cells(ctx: &Ctx<'_>, m: &MarkEntry, frame: &mut Frame) {
    let d = &m.data;
    let g = CellGeom::new(ctx.x, ctx.axis(m.opts.y_axis), ctx.plot, m);
    let color = |i: usize| m.point_color(i);
    let mut b = FrameBuilder::new(frame, LayerId::Main);
    for i in 0..d.len() {
        if let Some(rect) = g.rect(i) {
            b.rect(rect, color(i));
        }
    }
    if !m.opts.show_values {
        return;
    }
    for i in 0..d.len() {
        // Too small for any text: skip before formatting.
        let Some(rect) = g.rect(i).filter(|r| ctx.line_h <= r.h && ctx.pad < r.w) else {
            continue;
        };
        let v = d.value.as_ref().and_then(|c| c.get(i)).copied();
        if v.is_none() && d.label.is_none() {
            continue;
        }
        let mut buf = StackStr::<64>::new();
        let t = value_text(m, i, v.unwrap_or(f64::NAN), &mut buf);
        text_inside(ctx, &mut b, rect, t, color(i));
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::mark::{MarkData, MarkId, MarkOptions};

    fn mark(id: u32, kind: MarkKind, opts: MarkOptions, y: Vec<f64>) -> MarkEntry {
        let mut m = MarkEntry::new(MarkId(id), kind, opts, Rgba::hex(0));
        let x = (0..y.len()).map(|i| i as f64).collect();
        m.data = MarkData::xy(x, y);
        m.refresh_full();
        m
    }

    #[test]
    fn groups_share_slots_and_stacks_share_columns() {
        let opts = |group, stack| MarkOptions {
            group,
            stack,
            ..MarkOptions::default()
        };
        let all = [
            mark(1, MarkKind::Bars, opts(Some(1), None), vec![1.0]),
            mark(2, MarkKind::Bars, opts(Some(1), Some(7)), vec![1.0]),
            mark(3, MarkKind::Bars, opts(Some(1), Some(7)), vec![1.0]),
            mark(4, MarkKind::Bars, opts(Some(1), None), vec![1.0]),
            mark(5, MarkKind::Bars, opts(None, None), vec![1.0]),
        ];
        assert_eq!(group_column(&all, &all[0]), (0, 3));
        assert_eq!(group_column(&all, &all[2]), (1, 3));
        assert_eq!(group_column(&all, &all[3]), (2, 3));
        assert_eq!(group_column(&all, &all[4]), (0, 1));
    }

    #[test]
    fn waterfall_runs_totals() {
        let mut m = mark(
            1,
            MarkKind::Waterfall,
            MarkOptions::default(),
            vec![10.0, -4.0, f64::NAN, 3.0, f64::NAN],
        );
        m.data.step = Some(vec![
            WaterfallStep::Delta,
            WaterfallStep::Delta,
            WaterfallStep::Subtotal,
            WaterfallStep::Delta,
            WaterfallStep::Total,
        ]);
        let mut seen = Vec::new();
        waterfall_items(&Theme::light(), &m, |i, from, to, shown, prev, _| {
            seen.push((i, from, to, shown, prev));
        });
        assert_eq!(
            seen,
            [
                (0, 0.0, 10.0, 10.0, 0.0),
                (1, 10.0, 6.0, -4.0, 10.0),
                (2, 0.0, 6.0, 6.0, 6.0),
                (3, 6.0, 9.0, 3.0, 6.0),
                (4, 0.0, 9.0, 9.0, 9.0),
            ]
        );
    }

    proptest! {
        /// The top of the last stacked bar is the sum of the values on its
        /// side; areas add everything.
        #[test]
        fn stacks_add_up(
            vals in proptest::collection::vec(-100.0_f64..100.0, 1..6),
            area in any::<bool>(),
        ) {
            let kind = if area { MarkKind::Area } else { MarkKind::Bars };
            let opts = MarkOptions { stack: Some(1), ..MarkOptions::default() };
            let all: Vec<MarkEntry> = vals
                .iter()
                .enumerate()
                .map(|(k, &v)| mark(k as u32 + 1, kind, opts.clone(), vec![v]))
                .collect();
            let last = all.last().unwrap();
            let v = *vals.last().unwrap();
            let top = stack_base(&all, last, 0, v) + v;
            let expected: f64 = vals
                .iter()
                .filter(|&&w| area || (w >= 0.0) == (v >= 0.0))
                .sum();
            prop_assert!((top - expected).abs() < 1e-9);
        }

        /// The last total of a waterfall is the sum of its finite deltas.
        #[test]
        fn waterfall_final_total(vals in proptest::collection::vec(prop_oneof![Just(f64::NAN), -50.0_f64..50.0], 1..20)) {
            let mut m = mark(1, MarkKind::Waterfall, MarkOptions::default(), vals.clone());
            let mut steps = vec![WaterfallStep::Delta; vals.len()];
            steps.push(WaterfallStep::Total);
            m.data.x.push(vals.len() as f64);
            m.data.y.push(f64::NAN);
            m.data.step = Some(steps);
            let mut last = f64::NAN;
            waterfall_items(&Theme::light(), &m, |_, _, to, _, _, _| last = to);
            let sum: f64 = vals.iter().filter(|v| v.is_finite()).sum();
            prop_assert!((last - sum).abs() < 1e-9);
        }
    }
}
