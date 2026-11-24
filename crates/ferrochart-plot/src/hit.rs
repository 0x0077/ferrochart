//! Hit testing: the element under the pointer, in the geometry of the last
//! layout. No allocation.
//!
//! - Lines and areas with sorted x: the point nearest in x (binary search);
//!   otherwise the nearest point within a radius, through the grid index.
//! - Scatter: the nearest point whose symbol (plus a tolerance) covers the
//!   pointer.
//! - Bars, waterfalls and cells: the rectangle under the pointer.
//!
//! Marks drawn later win ties.

use ferrochart_core::geom::{Css, PointPx, RectPx};

use crate::axis::{Axis, AxisKind};
use crate::draw::{Geometry, clamp_px, visible_range};
use crate::interact::Hit;
use crate::mark::{MarkEntry, MarkKind, WaterfallStep};
use crate::shapes::{BarGeom, CellGeom, WaterfallGeom, waterfall_items};

/// Extra reach around symbols and the search radius of unsorted lines.
const TOLERANCE_CSS: f32 = 6.0;
const LINE_RADIUS_CSS: f32 = 16.0;
/// Points with a non-finite y are skipped when looking for the nearest x,
/// up to this many on each side.
const MAX_GAP_SCAN: usize = 64;
/// Scatter diameter without a size column or `point_size` (as drawn).
const DEFAULT_POINT_CSS: f32 = 6.0;

/// What the overlay highlights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    /// A point drawn with diameter `size` (0: a line vertex without symbol).
    Point { pos: PointPx, size: f32 },
    /// A bar or a cell.
    Rect(RectPx),
}

impl Shape {
    /// The anchor of crosshairs and tooltips.
    pub fn center(self) -> PointPx {
        match self {
            Self::Point { pos, .. } => pos,
            Self::Rect(r) => PointPx {
                x: r.x + r.w * 0.5,
                y: r.y + r.h * 0.5,
            },
        }
    }
}

/// The best candidate so far: smaller distances win, then earlier finds
/// (marks are visited topmost first).
struct Best {
    dist: f32,
    found: Option<(usize, usize, Shape, f64)>,
}

impl Best {
    fn offer(&mut self, dist: f32, mark: usize, index: usize, shape: Shape, y: f64) {
        if dist < self.dist {
            self.dist = dist;
            self.found = Some((mark, index, shape, y));
        }
    }
}

/// The data values a point would need to be drawn within `r` px of pixel
/// position `p` on `a` (points sit at slot centers on category axes).
fn data_range(a: &Axis, p: f32, r: f32) -> (f64, f64) {
    let (u, v) = (a.value_at(f64::from(p - r)), a.value_at(f64::from(p + r)));
    let off = if a.opts.kind == AxisKind::Category {
        0.5
    } else {
        0.0
    };
    (u.min(v) - off, u.max(v) - off)
}

fn point_of(x: &Axis, ya: &Axis, m: &MarkEntry, i: usize) -> PointPx {
    PointPx {
        x: clamp_px(x.point_px(m.data.x[i])),
        y: clamp_px(ya.point_px(m.data.y[i])),
    }
}

fn dist(a: PointPx, b: PointPx) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The element under `p`, its shape and the y it reports.
pub(crate) fn hit_at(c: &Geometry<'_>, marks: &[MarkEntry], p: PointPx) -> Option<(Hit, Shape)> {
    if !c.plot.contains(p) {
        return None;
    }
    let mut best = Best {
        dist: f32::INFINITY,
        found: None,
    };
    for (k, m) in marks.iter().enumerate().rev() {
        if !(m.opts.visible && m.opts.hoverable) || m.data.is_empty() {
            continue;
        }
        match m.kind {
            MarkKind::Line | MarkKind::Area => line(c, m, k, p, &mut best),
            MarkKind::Scatter => scatter(c, m, k, p, &mut best),
            MarkKind::Bars => bars(c, marks, m, k, p, &mut best),
            MarkKind::Waterfall => waterfall(c, m, k, p, &mut best),
            MarkKind::Cells => cells(c, m, k, p, &mut best),
        }
    }
    let (k, i, shape, y) = best.found?;
    let m = &marks[k];
    let pos = shape.center();
    Some((
        Hit {
            mark: m.id,
            index: i,
            x: m.data.x[i],
            y,
            pos: ferrochart_core::geom::PointCss {
                x: pos.x / c.dpr,
                y: pos.y / c.dpr,
            },
        },
        shape,
    ))
}

fn line(c: &Geometry<'_>, m: &MarkEntry, k: usize, p: PointPx, best: &mut Best) {
    let d = &m.data;
    let ya = c.axis(m.opts.y_axis);
    let vertex = c.px(Css(m.opts.point_size.0.max(0.0)));
    let ok = |i: usize| {
        let q = point_of(c.x, ya, m, i);
        (q.x.is_finite() && q.y.is_finite()).then_some(q)
    };
    let mut consider = |i: usize, q: PointPx| {
        best.offer(
            dist(p, q),
            k,
            i,
            Shape::Point {
                pos: q,
                size: vertex,
            },
            d.y[i],
        );
    };
    if m.x_sorted {
        // Nearest x among points with a position, within the line's extent.
        let tol = c.px(Css(TOLERANCE_CSS + c.touch_slop));
        let (v, _) = data_range(c.x, p.x, 0.0);
        let j = d.x.partition_point(|&w| w < v);
        let after = (j..d.len())
            .take(MAX_GAP_SCAN)
            .find_map(|i| ok(i).map(|q| (i, q)));
        let before = (0..j)
            .rev()
            .take(MAX_GAP_SCAN)
            .find_map(|i| ok(i).map(|q| (i, q)));
        let first = (0..d.len()).take(MAX_GAP_SCAN).find_map(ok);
        let last = (0..d.len()).rev().take(MAX_GAP_SCAN).find_map(ok);
        if let (Some(a), Some(b)) = (first, last)
            && (p.x < a.x.min(b.x) - tol || p.x > a.x.max(b.x) + tol)
        {
            return;
        }
        let pick = match (before, after) {
            (Some(b), Some(a)) => {
                if (b.1.x - p.x).abs() <= (a.1.x - p.x).abs() {
                    b
                } else {
                    a
                }
            }
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => return,
        };
        consider(pick.0, pick.1);
        return;
    }
    let r = c.px(Css(LINE_RADIUS_CSS + c.touch_slop));
    let mut near = |i: usize| {
        if let Some(q) = ok(i)
            && dist(p, q) <= r
        {
            consider(i, q);
        }
    };
    match &m.grid {
        Some(g) => {
            g.query(data_range(c.x, p.x, r), data_range(ya, p.y, r), &mut near);
            near(d.len() - 1);
        }
        None => (0..d.len()).for_each(near),
    }
}

fn scatter(c: &Geometry<'_>, m: &MarkEntry, k: usize, p: PointPx, best: &mut Best) {
    let d = &m.data;
    let ya = c.axis(m.opts.y_axis);
    let default_css = if m.opts.point_size.0 > 0.0 {
        m.opts.point_size.0
    } else {
        DEFAULT_POINT_CSS
    };
    let size = |i: usize| {
        let s = d
            .size
            .as_ref()
            .and_then(|s| s.get(i))
            .copied()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(default_css);
        c.px(Css(s))
    };
    let tol = c.px(Css(TOLERANCE_CSS + c.touch_slop));
    let mut near = |i: usize| {
        let q = point_of(c.x, ya, m, i);
        let s = size(i);
        let e = dist(p, q);
        if e.is_finite() && e <= s * 0.5 + tol {
            best.offer(e, k, i, Shape::Point { pos: q, size: s }, d.y[i]);
        }
    };
    match &m.grid {
        Some(g) => {
            // The largest symbol bounds the search box.
            let r = c.px(Css(m.max_size.max(default_css))) * 0.5 + tol;
            g.query(data_range(c.x, p.x, r), data_range(ya, p.y, r), &mut near);
            near(d.len() - 1);
        }
        None => (0..d.len()).for_each(near),
    }
}

fn bars(c: &Geometry<'_>, all: &[MarkEntry], m: &MarkEntry, k: usize, p: PointPx, best: &mut Best) {
    let ya = c.axis(m.opts.y_axis);
    let g = BarGeom::new(c.x, ya, c.plot, all, m);
    let hz = m.horizontal();
    let mut check = |i: usize| {
        if let Some(b) = g.bar(i)
            && b.rect.contains(p)
        {
            best.offer(0.0, k, i, Shape::Rect(b.rect), b.value);
        }
    };
    if !hz && m.x_sorted {
        let (half, off) = g.reach();
        let v = c.x.value_at(f64::from(p.x));
        let (i0, i1) = visible_range(g.cats(), true, (v - off - half, v - off + half));
        (i0..i1).for_each(&mut check);
    } else {
        (0..m.data.len()).for_each(&mut check);
    }
}

fn waterfall(c: &Geometry<'_>, m: &MarkEntry, k: usize, p: PointPx, best: &mut Best) {
    let g = WaterfallGeom::new(c.x, c.axis(m.opts.y_axis), m);
    let hz = m.horizontal();
    waterfall_items(c.theme, m, |i, from, to, shown, _, _| {
        let rect = g.rect(hz, i, from, to);
        if rect.contains(p) {
            let delta = m
                .data
                .step
                .as_ref()
                .and_then(|s| s.get(i))
                .copied()
                .unwrap_or_default()
                == WaterfallStep::Delta;
            best.offer(0.0, k, i, Shape::Rect(rect), if delta { shown } else { to });
        }
    });
}

fn cells(c: &Geometry<'_>, m: &MarkEntry, k: usize, p: PointPx, best: &mut Best) {
    let d = &m.data;
    let ya = c.axis(m.opts.y_axis);
    let g = CellGeom::new(c.x, ya, c.plot, m);
    let mut check = |i: usize| {
        if let Some(r) = g.rect(i)
            && r.contains(p)
        {
            best.offer(0.0, k, i, Shape::Rect(r), d.y[i]);
        }
    };
    match &m.grid {
        Some(grid) => {
            let vx = c.x.value_at(f64::from(p.x)) - g.ox;
            let vy = ya.value_at(f64::from(p.y)) - g.oy;
            grid.query((vx - g.hx, vx + g.hx), (vy - g.hy, vy + g.hy), &mut check);
            check(d.len() - 1);
        }
        None => (0..d.len()).for_each(check),
    }
}
