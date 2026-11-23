//! CPU implementation of the series transforms that the wgpu shaders perform.
//!
//! **Consistency rule**: the step from
//! `XTransform`/`YTransform` to pixels is done in `f32`, with exactly the operation
//! order the shaders use (no FMA), so that both backends round identically at
//! pixel boundaries.
//!
//! Shader compilers may fuse `a * b + c` into an FMA (Metal compiles with
//! fast-math by default, and WGSL cannot forbid it), which rounds differently
//! from a separate multiply and add. The per-element transforms are therefore
//! written **add first, multiply last** — `x = (d + shift) * step`,
//! `y = (v_rel + bias) * a` — which no compiler can contract. The folded
//! constants `shift` and `bias` are computed here, in f64, for both backends. Each function names its shader counterpart; the shaders are
//! in `crates/ferrochart-render-wgpu/src/shaders/series.wgsl` and must
//! mirror these functions line by line. Change both together.
//!

use crate::data::OhlcLevelView;
use crate::data::lod::{f32_down, f32_up};
use crate::frame::{LineDash, PriceMapping, XTransform, YTransform};
use crate::geom::{PointPx, Px, RectPx};

/// log2 of the number of elements per GPU chunk.
pub const CHUNK_SHIFT: u32 = 12;
/// Elements per GPU chunk. GPU buffers hold one more (the junction element).
pub const CHUNK_LEN: i64 = 1 << CHUNK_SHIFT;

/// Chunk number of element `e` of LOD `level`. Level 0 elements are
/// logical indices; level `k >= 2` elements are bucket numbers, which already
/// count from the anchor.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::ANCHOR;
/// use ferrochart_core::transform_helpers::{chunk_of, chunk_start, CHUNK_LEN};
///
/// assert_eq!(chunk_of(0, ANCHOR + CHUNK_LEN), 1);
/// assert_eq!(chunk_start(0, 1), ANCHOR + CHUNK_LEN);
/// assert_eq!(chunk_of(3, 5000), 1);
/// assert_eq!(chunk_start(3, 1), CHUNK_LEN);
/// ```
pub fn chunk_of(level: u8, e: i64) -> i64 {
    let from_anchor = if level == 0 {
        e - crate::data::ANCHOR
    } else {
        e
    };
    from_anchor >> CHUNK_SHIFT
}

/// First element of chunk `c` of LOD `level`.
pub fn chunk_start(level: u8, c: i64) -> i64 {
    let from_anchor = c << CHUNK_SHIFT;
    if level == 0 {
        from_anchor + crate::data::ANCHOR
    } else {
        from_anchor
    }
}

/// The reference price a renderer assigns to chunk `c` when it first writes it:
/// the first finite, positive open in the chunk. The GPU backend freezes
/// it; the stateless CPU backend recomputes it, which gives the same value
/// unless the chunk's first bars changed after the GPU first saw them.
pub fn initial_chunk_ref(view: &OhlcLevelView<'_>, c: i64) -> Option<f64> {
    let start = chunk_start(view.level, c).max(view.first);
    let end = (chunk_start(view.level, c) + CHUNK_LEN).min(view.end());
    let opens = || (start..end).filter_map(|j| view.open.at(view.first, j));
    // Value series may be zero or negative (oscillators), and a value bucket's
    // first element may be a gap: fall back to the first finite open, then the
    // first finite high. In log mode such a chunk only holds gaps anyway.
    opens()
        .find(|p| p.is_finite() && *p > 0.0)
        .or_else(|| opens().find(|p| p.is_finite()))
        .or_else(|| {
            (start..end)
                .filter_map(|j| view.high.at(view.first, j))
                .find(|p| p.is_finite())
        })
}

/// `[open, high, low, close]` relative to `chunk_ref`, exactly as stored in the
/// GPU buffers: open/close rounded to nearest, high rounded up and low rounded
/// down, so the drawn range always contains the true range.
pub fn ohlc_rel(yt: &YTransform, ohlc: [f64; 4], chunk_ref: f64) -> [f32; 4] {
    let r = map(yt.mapping, chunk_ref);
    let [o, h, l, c] = ohlc.map(|v| map(yt.mapping, v) - r);
    [o as f32, f32_up(h), f32_down(l), c as f32].map(gap_or)
}

/// Stored instead of a relative value that is not finite (a NaN gap of a value
/// series, the log of a non-positive price). Shaders cannot rely on NaN
/// checks under fast-math, so gaps are this finite sentinel. Shader: `GAP`.
pub const GAP: f32 = 3.0e38;

/// `v` if finite, else [`GAP`].
fn gap_or(v: f32) -> f32 {
    if v.is_finite() { v } else { GAP }
}

/// Whether a stored relative value is a gap. Shader: `is_gap()`.
pub fn is_gap(v_rel: f32) -> bool {
    // NaN compares false, so it counts as a gap too.
    v_rel.abs() >= 1.0e38 || v_rel.is_nan()
}

/// Up/down decision on the relative values. Shader: `is_up_rel()`.
pub fn is_up_rel(open_rel: f32, close_rel: f32) -> bool {
    close_rel >= open_rel
}

/// Pixel snapping used everywhere: round half up, `floor(v + 0.5)`.
/// Shader: `snap()`.
pub fn snap(v: f32) -> f32 {
    (v + 0.5).floor()
}

/// `offset / step`: the x offset in elements, so that
/// `x = (d + shift) * step` (no multiply-add to fuse). The per-draw uniform.
pub fn x_shift(xt: &XTransform) -> f32 {
    if xt.step == 0.0 {
        return 0.0;
    }
    (f64::from(xt.offset) / f64::from(xt.step)) as f32
}

/// x of the center of element `j`: `(d + shift) * step` with `d = j - base`.
/// Shader: `x_center()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::XTransform;
/// use ferrochart_core::transform_helpers::x_center;
///
/// let xt = XTransform { base: 100, step: 4.0, offset: 10.0 };
/// assert_eq!(x_center(&xt, 102), 18.0);
/// ```
pub fn x_center(xt: &XTransform, j: i64) -> f32 {
    let d = (j - xt.base) as f32;
    let shifted = d + x_shift(xt);
    shifted * xt.step
}

fn map(mapping: PriceMapping, v: f64) -> f64 {
    match mapping {
        PriceMapping::Linear => v,
        PriceMapping::Log => v.ln(),
    }
}

/// `f(value) - f(chunk_ref)` as f32: what the GPU buffers store. Computed
/// on the CPU in both backends.
pub fn v_rel(yt: &YTransform, value: f64, chunk_ref: f64) -> f32 {
    (map(yt.mapping, value) - map(yt.mapping, chunk_ref)) as f32
}

/// The per-chunk uniform: `f(chunk_ref) - f(view_ref) + b / a`, so that
/// `y = (v_rel + bias) * a` equals `a * (f(value) - f(view_ref)) + b`.
pub fn y_bias(yt: &YTransform, chunk_ref: f64) -> f32 {
    if yt.a == 0.0 {
        return 0.0;
    }
    let delta = map(yt.mapping, chunk_ref) - map(yt.mapping, yt.view_ref);
    (delta + f64::from(yt.b) / f64::from(yt.a)) as f32
}

/// `y = (v_rel + bias) * a` in f32, in this order. Shader: `y_of()`.
pub fn y(yt: &YTransform, v_rel: f32, bias: f32) -> f32 {
    let t = v_rel + bias;
    t * yt.a
}

/// y of a single price, with `view_ref` as the chunk reference.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{PriceMapping, YTransform};
/// use ferrochart_core::transform_helpers::y_of_price;
///
/// let yt = YTransform { a: -2.0, b: 50.0, view_ref: 100.0, mapping: PriceMapping::Linear };
/// assert_eq!(y_of_price(&yt, 110.0), 30.0);
/// ```
pub fn y_of_price(yt: &YTransform, price: f64) -> f32 {
    y(yt, v_rel(yt, price, yt.view_ref), y_bias(yt, yt.view_ref))
}

/// How bars are drawn at a given element step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarMetrics {
    /// Body width in pixels: `max(1, floor(step * 0.8))`, made odd by subtracting 1.
    pub body: f32,
    /// Whether bodies are drawn (`step >= 1.5`); otherwise only high-low wicks.
    pub show_body: bool,
    /// Whether body borders are drawn (`step >= 3`).
    pub show_border: bool,
}

/// Bar metrics for element spacing `step`. Shader: `bar_metrics()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::transform_helpers::bar_metrics;
///
/// assert_eq!(bar_metrics(10.0).body, 7.0); // floor(8) = 8, even -> 7
/// assert_eq!(bar_metrics(6.0).body, 3.0);  // floor(4.8) = 4 -> 3
/// assert!(!bar_metrics(1.2).show_body);
/// ```
pub fn bar_metrics(step: f32) -> BarMetrics {
    let mut body = (step * 0.8).floor().max(1.0);
    if body % 2.0 == 0.0 {
        body -= 1.0;
    }
    BarMetrics {
        body,
        show_body: step >= 1.5,
        show_border: step >= 3.0,
    }
}

/// Up/down rule: a bar is "up" when `close >= open`. Shader: `is_up()`.
pub fn is_up(open: f64, close: f64) -> bool {
    close >= open
}

/// Pixel geometry of one candle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandleGeom {
    /// 1-px wide high-low line.
    pub wick: RectPx,
    /// Body, if bodies are drawn at this step.
    pub body: Option<RectPx>,
}

/// Vertical pixel span `[snap(min), snap(max))`, at least 1 px tall.
fn v_span(y0: f32, y1: f32) -> (f32, f32) {
    let top = snap(y0.min(y1));
    let bottom = snap(y0.max(y1));
    (top, (bottom - top).max(1.0))
}

/// Candle geometry from the element's center x and the y of its prices.
/// Shader: `candle_vertex()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::transform_helpers::{bar_metrics, candle};
///
/// let g = candle(10.3, bar_metrics(6.0), [20.0, 10.0, 40.0, 30.0]);
/// assert_eq!((g.wick.x, g.wick.w, g.wick.y, g.wick.h), (10.0, 1.0, 10.0, 30.0));
/// let body = g.body.unwrap();
/// assert_eq!((body.x, body.w, body.y, body.h), (9.0, 3.0, 20.0, 10.0));
/// ```
pub fn candle(xc: f32, m: BarMetrics, ys: [f32; 4]) -> CandleGeom {
    let [y_open, y_high, y_low, y_close] = ys;
    let column = xc.floor();
    let (wy, wh) = v_span(y_high, y_low);
    let wick = RectPx {
        x: column,
        y: wy,
        w: 1.0,
        h: wh,
    };
    let body = m.show_body.then(|| {
        let (by, bh) = v_span(y_open, y_close);
        RectPx {
            x: column - (m.body - 1.0) * 0.5,
            y: by,
            w: m.body,
            h: bh,
        }
    });
    CandleGeom { wick, body }
}

/// A histogram column from `y_value` down (or up) to `y_base`. Shader:
/// `histogram_vertex()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::transform_helpers::{bar_metrics, histogram};
///
/// let r = histogram(10.3, bar_metrics(6.0), 80.2, 100.0);
/// assert_eq!((r.x, r.w, r.y, r.h), (9.0, 3.0, 80.0, 20.0));
/// ```
pub fn histogram(xc: f32, m: BarMetrics, y_value: f32, y_base: f32) -> RectPx {
    let column = xc.floor();
    let width = if m.show_body { m.body } else { 1.0 };
    let (y, h) = v_span(y_value, y_base);
    RectPx {
        x: column - (width - 1.0) * 0.5,
        y,
        w: width,
        h,
    }
}

/// OHLC bar ("American line"): the high-low line, and when bodies are shown at
/// this step, the open tick on the left and the close tick on the right
/// (1 px thick, `(body - 1) / 2` long). Shader: `vs_ohlc_bar`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::transform_helpers::{bar_metrics, ohlc_bar};
///
/// let [wick, open, close] = ohlc_bar(10.3, bar_metrics(10.0), [20.0, 10.0, 40.0, 30.0]);
/// let (wick, open, close) = (wick.unwrap(), open.unwrap(), close.unwrap());
/// assert_eq!((wick.x, wick.y, wick.h), (10.0, 10.0, 30.0));
/// assert_eq!((open.x, open.w, open.y, open.h), (7.0, 3.0, 20.0, 1.0));
/// assert_eq!((close.x, close.w, close.y), (11.0, 3.0, 30.0));
/// ```
pub fn ohlc_bar(xc: f32, m: BarMetrics, ys: [f32; 4]) -> [Option<RectPx>; 3] {
    let [y_open, y_high, y_low, y_close] = ys;
    let column = xc.floor();
    let (wy, wh) = v_span(y_high, y_low);
    let wick = RectPx {
        x: column,
        y: wy,
        w: 1.0,
        h: wh,
    };
    let half = (m.body - 1.0) * 0.5;
    if !m.show_body || half < 1.0 {
        return [Some(wick), None, None];
    }
    let (oy, _) = v_span(y_open, y_open);
    let (cy, _) = v_span(y_close, y_close);
    let open = RectPx {
        x: column - half,
        y: oy,
        w: half,
        h: 1.0,
    };
    let close = RectPx {
        x: column + 1.0,
        y: cy,
        w: half,
        h: 1.0,
    };
    [Some(wick), Some(open), Some(close)]
}

/// Alpha coverage in 1/255 steps, as both backends blend it:
/// `floor(coverage * 255 + 0.5)`. Shader: `coverage_8()`.
pub fn coverage_8(coverage: f32) -> u8 {
    (coverage * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Coverage of the pixel centered at `p` by the segment `a`→`b` drawn
/// `2 × half_width` wide with round caps (consecutive segments of a polyline
/// join without gaps). Anti-aliased over one pixel. Shader:
/// `segment_coverage()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{PointPx, Px};
/// use ferrochart_core::transform_helpers::segment_coverage;
///
/// let (a, b) = (PointPx { x: 0.0, y: 10.0 }, PointPx { x: 20.0, y: 10.0 });
/// assert_eq!(segment_coverage(PointPx { x: 5.5, y: 10.5 }, a, b, Px(1.0)), 1.0);
/// assert_eq!(segment_coverage(PointPx { x: 5.5, y: 13.5 }, a, b, Px(1.0)), 0.0);
/// ```
pub fn segment_coverage(p: PointPx, a: PointPx, b: PointPx, half_width: Px) -> f32 {
    let pax = p.x - a.x;
    let pay = p.y - a.y;
    let bax = b.x - a.x;
    let bay = b.y - a.y;
    let len2 = bax * bax + bay * bay;
    let mut h = 0.0;
    if len2 > 0.0 {
        h = ((pax * bax + pay * bay) / len2).clamp(0.0, 1.0);
    }
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    let d = (dx * dx + dy * dy).sqrt();
    (half_width.0 + 0.5 - d).clamp(0.0, 1.0)
}

/// Direction and length of the line `a`→`b` (the length is at least 1e-4,
/// so the direction is always defined). Shader: `vs_line()`.
pub fn line_axis(a: PointPx, b: PointPx) -> (PointPx, f32) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt().max(1e-4);
    (
        PointPx {
            x: dx / len,
            y: dy / len,
        },
        len,
    )
}

/// Coverage of a pixel by an immediate line (`LineInst`) of length
/// `len` and `width`, butt ends, anti-aliased over one pixel: `along` is the
/// pixel center's distance along the line from its start, `across` its
/// signed distance from the axis. Dashes are `4w` on, `4w` off and dots `w`
/// on, `2w` off, measured along the line. Shader: `line_coverage()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::LineDash;
/// use ferrochart_core::geom::Px;
/// use ferrochart_core::transform_helpers::line_coverage;
///
/// assert_eq!(line_coverage(5.0, 0.0, 10.0, Px(1.0), LineDash::Solid), 1.0);
/// assert_eq!(line_coverage(5.0, 2.0, 10.0, Px(1.0), LineDash::Solid), 0.0);
/// assert_eq!(line_coverage(5.0, 0.0, 10.0, Px(1.0), LineDash::Dashed), 0.0);
/// ```
pub fn line_coverage(along: f32, across: f32, len: f32, width: Px, dash: LineDash) -> f32 {
    let half = width.0 * 0.5;
    let cov_across = (half + 0.5 - across.abs()).clamp(0.0, 1.0);
    let cov_along = (along.min(len - along) + 0.5).clamp(0.0, 1.0);
    let on = match dash {
        LineDash::Solid => true,
        LineDash::Dashed => {
            let t = along / (8.0 * width.0);
            t - t.floor() < 0.5
        }
        LineDash::Dotted => {
            let t = along / (3.0 * width.0);
            t - t.floor() < 1.0 / 3.0
        }
    };
    if on { cov_across * cov_along } else { 0.0 }
}

/// Built-in icon: filled circle (markers).
pub const ICON_CIRCLE: u16 = 0;
/// Built-in icon: filled square.
pub const ICON_SQUARE: u16 = 1;
/// Built-in icon: arrow pointing up.
pub const ICON_ARROW_UP: u16 = 2;
/// Built-in icon: arrow pointing down.
pub const ICON_ARROW_DOWN: u16 = 3;

/// Coverage of the pixel whose center is `(dx, dy)` from the center of a
/// built-in icon `size` pixels across (the "atlas" is these signed
/// distance functions), anti-aliased over one pixel; 0 for unknown icons.
/// Shader: `icon_coverage()` in `prims.wgsl`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::transform_helpers::{ICON_ARROW_UP, ICON_CIRCLE, icon_coverage};
///
/// assert_eq!(icon_coverage(ICON_CIRCLE, 0.5, 0.5, 10.0), 1.0);
/// assert_eq!(icon_coverage(ICON_CIRCLE, 6.5, 0.5, 10.0), 0.0);
/// // The tip of an up arrow is at the top (y grows downwards).
/// assert!(icon_coverage(ICON_ARROW_UP, 0.0, -4.0, 10.0) > 0.0);
/// assert_eq!(icon_coverage(ICON_ARROW_UP, 4.0, -4.0, 10.0), 0.0);
/// ```
pub fn icon_coverage(icon: u16, dx: f32, dy: f32, size: f32) -> f32 {
    match icon_distance(icon, dx, dy, size) {
        Some(d) => (0.5 - d).clamp(0.0, 1.0),
        None => 0.0,
    }
}

/// Signed distance in pixels from `(dx, dy)` to the edge of a built-in icon
/// `size` pixels across: negative inside; `None` for unknown icons. Shader:
/// `icon_distance()`.
pub fn icon_distance(icon: u16, dx: f32, dy: f32, size: f32) -> Option<f32> {
    let h = size * 0.5;
    let ax = dx.abs();
    let d = match icon {
        ICON_CIRCLE => (dx * dx + dy * dy).sqrt() - h,
        ICON_SQUARE => {
            let s = h * 0.8;
            (ax - s).max(dy.abs() - s)
        }
        ICON_ARROW_UP | ICON_ARROW_DOWN => {
            let ty = if icon == ICON_ARROW_UP { dy } else { -dy };
            // Head: the triangle (0, -h), (±h, 0); shaft: |x| ≤ 0.35h, 0 ≤ y ≤ h.
            let head = ty.max((ax - ty - h) * 0.707_106_77);
            let q = h * 0.5;
            let shaft = (ax - h * 0.35).max((ty - q).abs() - q);
            head.min(shaft)
        }
        _ => return None,
    };
    Some(d)
}

/// Half the side of the square an image fills in icon `icon` (r21): the
/// circle's radius, the square's half side; `None` for icons that take no
/// image (arrows). Shader: `icon_image_half()`.
pub fn icon_image_half(icon: u16, size: f32) -> Option<f32> {
    match icon {
        ICON_CIRCLE => Some(size * 0.5),
        ICON_SQUARE => Some(size * 0.5 * 0.8),
        _ => None,
    }
}

/// Bilinear sample of a 64×64 premultiplied tile at continuous texel
/// coordinates `(u, v)` (texel centers at `k + 0.5`), clamped to the tile,
/// in 0..=1. Four loads and f32 weights, the same in the shader
/// (`sample_tile()`), so the backends agree exactly.
pub fn sample_tile(tile: &crate::icon::IconTile, u: f32, v: f32) -> [f32; 4] {
    let last = (crate::icon::ICON_TEXELS - 1) as f32;
    let x = (u - 0.5).clamp(0.0, last);
    let y = (v - 0.5).clamp(0.0, last);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x1, y1) = ((x0 + 1.0).min(last), (y0 + 1.0).min(last));
    let at = |x: f32, y: f32, c: usize| -> f32 {
        let k = (y as usize * crate::icon::ICON_TEXELS + x as usize) * 4 + c;
        f32::from(tile[k]) / 255.0
    };
    let mut out = [0.0; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
        let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
        *o = top * (1.0 - fy) + bottom * fy;
    }
    out
}

/// Premultiplied color (0..=1) of the pixel centered `(dx, dy)` from the
/// center of an icon with an image and/or a border (r21): the image
/// over the fill, clipped to the shape, and the border inside the edge.
/// Icons without both are drawn by [`icon_coverage`] alone. Shader:
/// `icon_shade()`.
pub fn icon_shade(
    icon: &crate::frame::IconInst,
    dx: f32,
    dy: f32,
    tile: Option<&crate::icon::IconTile>,
) -> [f32; 4] {
    let Some(d) = icon_distance(icon.icon, dx, dy, icon.size.0) else {
        return [0.0; 4];
    };
    let premul = |c: crate::geom::Rgba| -> [f32; 4] {
        let a = f32::from(c.a) / 255.0;
        [
            f32::from(c.r) / 255.0 * a,
            f32::from(c.g) / 255.0 * a,
            f32::from(c.b) / 255.0 * a,
            a,
        ]
    };
    let fill = premul(icon.color);
    let mut inner = fill;
    if let (Some(tile), Some(half)) = (tile, icon_image_half(icon.icon, icon.size.0)) {
        let n = crate::icon::ICON_TEXELS as f32;
        let u = (dx / (2.0 * half) + 0.5) * n;
        let v = (dy / (2.0 * half) + 0.5) * n;
        let s = sample_tile(tile, u, v);
        for c in 0..4 {
            inner[c] = s[c] + fill[c] * (1.0 - s[3]);
        }
    }
    let cov = (0.5 - d).clamp(0.0, 1.0);
    let mut out = [0.0; 4];
    if icon.border.a > 0 && icon.border_width.0 > 0.0 {
        let ic = (0.5 - (d + icon.border_width.0)).clamp(0.0, 1.0);
        let b = premul(icon.border);
        for c in 0..4 {
            out[c] = inner[c] * ic + b[c] * (cov - ic);
        }
    } else {
        for c in 0..4 {
            out[c] = inner[c] * cov;
        }
    }
    out
}

/// A premultiplied color (0..=1) as straight 8-bit RGBA, the way both
/// backends blend it; alpha 0 for nothing to draw. Shader: `unpremul8()`.
pub fn unpremultiply_8(c: [f32; 4]) -> crate::geom::Rgba {
    let a = (c[3] * 255.0 + 0.5).floor().clamp(0.0, 255.0);
    if a <= 0.0 {
        return crate::geom::Rgba::default();
    }
    let ch = |v: f32| ((v / c[3]) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
    crate::geom::Rgba {
        r: ch(c[0]),
        g: ch(c[1]),
        b: ch(c[2]),
        a: a as u8,
    }
}

/// Whether the dash pattern is "on" at pixel center `x` for a line of the
/// given width (patterns follow the x axis, which suits the mostly horizontal
/// series lines). Dashes: `4w` on, `4w` off; dots: `w` on, `w` off. Shader:
/// `dash_on()`.
pub fn dash_on(x: Px, width: Px, dash: LineDash) -> bool {
    let period = match dash {
        LineDash::Solid => return true,
        LineDash::Dashed => 4.0 * width.0,
        LineDash::Dotted => width.0,
    };
    let k = (x.0 / period).floor();
    k - 2.0 * (k * 0.5).floor() < 0.5
}

/// Coverage of the pixel centered at `p` by the fill between the edge `a`→`b`
/// and the horizontal line `base_y`, within the column range `[a.x, b.x)`
/// (half-open, so adjacent columns never overlap). Both horizontal edges are
/// anti-aliased. Shader: `fill_coverage()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{PointPx, Px};
/// use ferrochart_core::transform_helpers::fill_coverage;
///
/// let (a, b) = (PointPx { x: 0.0, y: 10.0 }, PointPx { x: 10.0, y: 10.0 });
/// assert_eq!(fill_coverage(PointPx { x: 2.5, y: 15.5 }, a, b, Px(20.0)), 1.0);
/// assert_eq!(fill_coverage(PointPx { x: 2.5, y: 5.5 }, a, b, Px(20.0)), 0.0);
/// assert_eq!(fill_coverage(PointPx { x: 10.5, y: 15.5 }, a, b, Px(20.0)), 0.0);
/// ```
pub fn fill_coverage(p: PointPx, a: PointPx, b: PointPx, base_y: Px) -> f32 {
    if !(p.x >= a.x && p.x < b.x) {
        return 0.0;
    }
    let t = (p.x - a.x) / (b.x - a.x);
    let edge = a.y + (b.y - a.y) * t;
    let lo = edge.min(base_y.0);
    let hi = edge.max(base_y.0);
    ((p.y - lo).min(hi - p.y) + 0.5).clamp(0.0, 1.0)
}

/// Coverage of the pixel centered at `p` by a band between the upper edge
/// `a`→`b` and the lower edge from `(a.x, lower_a)` to `(b.x, lower_b)`,
/// within `[a.x, b.x)` like [`fill_coverage`], and whether the upper series
/// is below the lower one there (`crossed`). `scale` is the y transform's
/// `a`: its sign says which way values grow on screen. Shader:
/// `band_coverage()`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::PointPx;
/// use ferrochart_core::transform_helpers::band_coverage;
///
/// let (a, b) = (PointPx { x: 0.0, y: 10.0 }, PointPx { x: 10.0, y: 10.0 });
/// // Lower edge at y = 20: inside between 10 and 20 (y grows downwards, a < 0).
/// assert_eq!(band_coverage(PointPx { x: 5.5, y: 15.5 }, a, b, 20.0, 20.0, -1.0), (1.0, false));
/// assert_eq!(band_coverage(PointPx { x: 5.5, y: 25.5 }, a, b, 20.0, 20.0, -1.0).0, 0.0);
/// // The "lower" series drawn above it: crossed.
/// assert!(band_coverage(PointPx { x: 5.5, y: 7.5 }, a, b, 5.0, 5.0, -1.0).1);
/// ```
pub fn band_coverage(
    p: PointPx,
    a: PointPx,
    b: PointPx,
    lower_a: f32,
    lower_b: f32,
    scale: f32,
) -> (f32, bool) {
    if !(p.x >= a.x && p.x < b.x) {
        return (0.0, false);
    }
    let t = (p.x - a.x) / (b.x - a.x);
    let upper = a.y + (b.y - a.y) * t;
    let lower = lower_a + (lower_b - lower_a) * t;
    let lo = upper.min(lower);
    let hi = upper.max(lower);
    let coverage = ((p.y - lo).min(hi - p.y) + 0.5).clamp(0.0, 1.0);
    (coverage, (lower - upper) * scale > 0.0)
}

/// The line segments of one element of a value series. `cur` and `next` are
/// the stored relative `[first, max, min, last]` of elements `j` and `j + 1`
/// (both relative to element `j`'s chunk reference), `xj`/`xn` their center
/// x. Segment 0 is the max-min range of an LOD bucket (absent at level 0,
/// where max == min); segment 1 connects `last(j)` to `first(j + 1)`, or for a
/// step line runs horizontally to `xn`, and segment 2 is then the vertical
/// step. Gaps break the line. Shader: `vs_line_series`.
pub fn line_segments(
    yt: &YTransform,
    bias: f32,
    xj: f32,
    xn: f32,
    cur: [f32; 4],
    next: [f32; 4],
    step: bool,
) -> [Option<(PointPx, PointPx)>; 3] {
    let [_, hi, lo, last] = cur;
    let first_n = next[0];
    let p = |x: f32, v: f32| PointPx {
        x,
        y: y(yt, v, bias),
    };
    let mut out = [None; 3];
    if !is_gap(hi) && !is_gap(lo) && hi.to_bits() != lo.to_bits() {
        out[0] = Some((p(xj, hi), p(xj, lo)));
    }
    if !is_gap(last) && !is_gap(first_n) {
        if step {
            out[1] = Some((p(xj, last), p(xn, last)));
            out[2] = Some((p(xn, last), p(xn, first_n)));
        } else {
            out[1] = Some((p(xj, last), p(xn, first_n)));
        }
    }
    out
}

/// The top edge of the fill under one element of an area or baseline series
/// (see [`line_segments`]); `None` across gaps. Shader: `vs_fill_series`.
pub fn fill_edge(
    yt: &YTransform,
    bias: f32,
    xj: f32,
    xn: f32,
    cur: [f32; 4],
    next: [f32; 4],
    step: bool,
) -> Option<(PointPx, PointPx)> {
    let (last, first_n) = (cur[3], next[0]);
    if is_gap(last) || is_gap(first_n) {
        return None;
    }
    let y0 = y(yt, last, bias);
    let y1 = if step { y0 } else { y(yt, first_n, bias) };
    Some((PointPx { x: xj, y: y0 }, PointPx { x: xn, y: y1 }))
}

/// The y a histogram column of a value series reaches: of the element's max
/// and min, the one farther from the base (so LOD buckets keep their spikes).
/// `None` for a gap. Shader: `vs_value_histogram`.
pub fn histogram_value_y(yt: &YTransform, bias: f32, cur: [f32; 4], base_y: f32) -> Option<f32> {
    let (hi, lo) = (cur[1], cur[2]);
    if is_gap(hi) || is_gap(lo) {
        return None;
    }
    let (y_hi, y_lo) = (y(yt, hi, bias), y(yt, lo, bias));
    Some(if (y_hi - base_y).abs() >= (y_lo - base_y).abs() {
        y_hi
    } else {
        y_lo
    })
}

/// Position of `y` in a vertical gradient from `top` (0) to `bottom` (1).
/// Shader: `gradient_t()`.
pub fn gradient_t(y: Px, top: Px, bottom: Px) -> f32 {
    let h = bottom.0 - top.0;
    if h <= 0.0 {
        return 0.0;
    }
    ((y.0 - top.0) / h).clamp(0.0, 1.0)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn body_is_odd_and_fits_in_step() {
        for i in 1..2000 {
            let step = i as f32 * 0.05;
            let m = bar_metrics(step);
            assert!(m.body >= 1.0);
            assert_eq!(m.body % 2.0, 1.0, "step {step}");
            assert!(m.body <= step.max(1.0), "step {step}");
        }
    }

    #[test]
    fn log_mapping_is_relative() {
        let yt = YTransform {
            a: -100.0,
            b: 0.0,
            view_ref: 10.0,
            mapping: PriceMapping::Log,
        };
        // One e-fold above the reference moves 100 px up.
        assert!((y_of_price(&yt, 10.0 * core::f64::consts::E) + 100.0).abs() < 1e-3);
    }
}
