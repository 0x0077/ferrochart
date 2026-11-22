// Series pipelines: candles, OHLC bars, histograms, lines, fills, points and
// bands.
//
// CONSISTENCY RULE: every function below mirrors the function
// of the same name in `ferrochart-core/src/transform_helpers.rs`, with the same
// f32 operations in the same order. Change both together.
//
// Data arrives per chunk: instance-step vertex buffers bound at the
// chunk's first visible element, so instance 0 is that element and
// `instance_index + index_offset` is the element's offset from the draw's base.

struct Draw {
    viewport: vec2<f32>,
    x_step: f32,
    x_shift: f32,
    a: f32,
    bias: f32,
    index_offset: i32,
    body: f32,
    flags: u32,
    base_y: f32,
    // Top and bottom of the series clip (area gradient).
    clip_y: vec2<f32>,
    up: vec4<f32>,
    down: vec4<f32>,
    up_wick: vec4<f32>,
    down_wick: vec4<f32>,
    up_border: vec4<f32>,
    down_border: vec4<f32>,
}
// Line and fill draws: `up`/`down` are the line colors (above/below the
// baseline), `up_wick`/`down_wick` the fill top/bottom colors, and `body` the
// line's half width.

@group(0) @binding(0) var<uniform> u: Draw;

const SHOW_BODY: u32 = 1u;
const SHOW_BORDER: u32 = 2u;
const STEP: u32 = 4u;
const BASELINE: u32 = 8u;
const AREA: u32 = 16u;
// Bits 5..6: 0 solid, 1 dashed, 2 dotted.
const DASH_SHIFT: u32 = 5u;

// transform_helpers::is_gap. GAP is a finite sentinel; NaN checks may be
// optimized away under fast-math.
fn is_gap(v: f32) -> bool {
    return abs(v) >= 1.0e38;
}

fn any_gap(v: vec4<f32>) -> bool {
    return is_gap(v.x) || is_gap(v.y) || is_gap(v.z) || is_gap(v.w);
}

// transform_helpers::snap
fn snap(v: f32) -> f32 {
    return floor(v + 0.5);
}

// transform_helpers::x_center (d = j - base). Add first, multiply last: a
// multiply followed by an add may be fused into an FMA by the compiler.
fn x_center(d: i32) -> f32 {
    let shifted = f32(d) + u.x_shift;
    return shifted * u.x_step;
}

// transform_helpers::y
fn y_of(v_rel: f32) -> f32 {
    let t = v_rel + u.bias;
    return t * u.a;
}

// transform_helpers::is_up_rel
fn is_up_rel(open_rel: f32, close_rel: f32) -> bool {
    return close_rel >= open_rel;
}

// transform_helpers::v_span -> (top, height)
fn v_span(y0: f32, y1: f32) -> vec2<f32> {
    let top = snap(min(y0, y1));
    let bottom = snap(max(y0, y1));
    return vec2<f32>(top, max(bottom - top, 1.0));
}

const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
);

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0, 0.0, 1.0);
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) fill: vec4<f32>,
    @location(1) @interpolate(flat) border: vec4<f32>,
    // Rectangle (x0, y0, x1, y1); x0 > x1 disables the border.
    @location(2) @interpolate(flat) rect: vec4<f32>,
}

fn quad(rect: vec4<f32>, corner: u32) -> vec4<f32> {
    let c = CORNERS[corner];
    return to_clip(rect.xy + c * rect.zw);
}

fn hidden_rect() -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    out.rect = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    return out;
}

// transform_helpers::candle. Vertices 0..6 draw the wick, 6..12 the body,
// so each bar is drawn wick first, like the CPU backend.
@vertex
fn vs_candle(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
             @location(0) ohlc: vec4<f32>) -> VsOut {
    if any_gap(ohlc) {
        return hidden_rect();
    }
    let xc = x_center(i32(ii) + u.index_offset);
    let y_open = y_of(ohlc.x);
    let y_high = y_of(ohlc.y);
    let y_low = y_of(ohlc.z);
    let y_close = y_of(ohlc.w);
    let up = is_up_rel(ohlc.x, ohlc.w);
    let column = floor(xc);
    var out: VsOut;
    out.rect = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    if vi < 6u {
        let w = v_span(y_high, y_low);
        out.pos = quad(vec4<f32>(column, w.x, 1.0, w.y), vi);
        out.fill = select(u.down_wick, u.up_wick, up);
        out.border = out.fill;
        return out;
    }
    if (u.flags & SHOW_BODY) == 0u {
        out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }
    let b = v_span(y_open, y_close);
    let r = vec4<f32>(column - (u.body - 1.0) * 0.5, b.x, u.body, b.y);
    out.pos = quad(r, vi - 6u);
    out.fill = select(u.down, u.up, up);
    out.border = select(u.down_border, u.up_border, up);
    if (u.flags & SHOW_BORDER) != 0u {
        out.rect = vec4<f32>(r.xy, r.xy + r.zw);
    }
    return out;
}

@fragment
fn fs_series(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let r = in.rect;
    if r.x <= r.z {
        let inner = p.x > r.x + 1.0 && p.x < r.z - 1.0 && p.y > r.y + 1.0 && p.y < r.w - 1.0;
        if !inner {
            return in.border;
        }
    }
    return in.fill;
}

// transform_helpers::histogram, colored by the bar's direction.
@vertex
fn vs_histogram(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
                @location(0) ohlc: vec4<f32>, @location(1) value: f32) -> VsOut {
    // A gap bar of the source series has no volume bar.
    if any_gap(ohlc) {
        return hidden_rect();
    }
    let xc = x_center(i32(ii) + u.index_offset);
    let up = is_up_rel(ohlc.x, ohlc.w);
    let column = floor(xc);
    let width = select(1.0, u.body, (u.flags & SHOW_BODY) != 0u);
    let s = v_span(y_of(value), u.base_y);
    var out: VsOut;
    out.pos = quad(vec4<f32>(column - (width - 1.0) * 0.5, s.x, width, s.y), vi);
    out.fill = select(u.down, u.up, up);
    out.border = out.fill;
    out.rect = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    return out;
}

// transform_helpers::ohlc_bar. Vertices 0..6 draw the high-low line, 6..12
// the open tick, 12..18 the close tick.
@vertex
fn vs_ohlc_bar(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
               @location(0) ohlc: vec4<f32>) -> VsOut {
    if any_gap(ohlc) {
        return hidden_rect();
    }
    let xc = x_center(i32(ii) + u.index_offset);
    let y_open = y_of(ohlc.x);
    let y_high = y_of(ohlc.y);
    let y_low = y_of(ohlc.z);
    let y_close = y_of(ohlc.w);
    let up = is_up_rel(ohlc.x, ohlc.w);
    let column = floor(xc);
    let part = vi / 6u;
    let corner = vi % 6u;
    var out = hidden_rect();
    out.fill = select(u.down, u.up, up);
    out.border = out.fill;
    if part == 0u {
        let w = v_span(y_high, y_low);
        out.pos = quad(vec4<f32>(column, w.x, 1.0, w.y), corner);
        return out;
    }
    let half = (u.body - 1.0) * 0.5;
    if (u.flags & SHOW_BODY) == 0u || half < 1.0 {
        return hidden_rect();
    }
    if part == 1u {
        let o = v_span(y_open, y_open);
        out.pos = quad(vec4<f32>(column - half, o.x, half, 1.0), corner);
    } else {
        let c = v_span(y_close, y_close);
        out.pos = quad(vec4<f32>(column + 1.0, c.x, half, 1.0), corner);
    }
    return out;
}

// transform_helpers::histogram_value_y + histogram, for value series. A
// per-point color with alpha 0 means the series color.
@vertex
fn vs_value_histogram(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
                      @location(0) v: vec4<f32>, @location(1) color: vec4<f32>) -> VsOut {
    let hi = v.y;
    let lo = v.z;
    if is_gap(hi) || is_gap(lo) {
        return hidden_rect();
    }
    let y_hi = y_of(hi);
    let y_lo = y_of(lo);
    let y_value = select(y_lo, y_hi, abs(y_hi - u.base_y) >= abs(y_lo - u.base_y));
    let xc = x_center(i32(ii) + u.index_offset);
    let column = floor(xc);
    let width = select(1.0, u.body, (u.flags & SHOW_BODY) != 0u);
    let s = v_span(y_value, u.base_y);
    var out = hidden_rect();
    out.pos = quad(vec4<f32>(column - (width - 1.0) * 0.5, s.x, width, s.y), vi);
    out.fill = select(u.up, color, color.a > 0.0);
    out.border = out.fill;
    return out;
}

// ---- lines and fills (per-pixel coverage) ----------------------------------

struct SegOut {
    @builtin(position) pos: vec4<f32>,
    // Segment or fill edge (a.x, a.y, b.x, b.y).
    @location(0) @interpolate(flat) seg: vec4<f32>,
}

fn hidden_seg() -> SegOut {
    var out: SegOut;
    out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}

// transform_helpers::coverage_8
fn coverage_8(coverage: f32) -> f32 {
    return clamp(floor(coverage * 255.0 + 0.5), 0.0, 255.0);
}

// CpuRenderer::blend's alpha: a = floor(a8 * c8 / 255), as for glyphs.
fn with_coverage(color: vec4<f32>, coverage: f32) -> vec4<f32> {
    let a8 = floor(color.a * 255.0 + 0.5);
    let a = floor(a8 * coverage_8(coverage) / 255.0) / 255.0;
    return vec4<f32>(color.rgb, a);
}

// transform_helpers::segment_coverage
fn segment_coverage(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, half_width: f32) -> f32 {
    let pax = p.x - a.x;
    let pay = p.y - a.y;
    let bax = b.x - a.x;
    let bay = b.y - a.y;
    let len2 = bax * bax + bay * bay;
    var h = 0.0;
    if len2 > 0.0 {
        h = clamp((pax * bax + pay * bay) / len2, 0.0, 1.0);
    }
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    let d = sqrt(dx * dx + dy * dy);
    return clamp(half_width + 0.5 - d, 0.0, 1.0);
}

// transform_helpers::dash_on
fn dash_on(x: f32, width: f32) -> bool {
    let dash = (u.flags >> DASH_SHIFT) & 3u;
    if dash == 0u {
        return true;
    }
    let period = select(width, 4.0 * width, dash == 1u);
    let k = floor(x / period);
    return k - 2.0 * floor(k * 0.5) < 0.5;
}

// transform_helpers::fill_coverage
fn fill_coverage(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, base_y: f32) -> f32 {
    if !(p.x >= a.x && p.x < b.x) {
        return 0.0;
    }
    let t = (p.x - a.x) / (b.x - a.x);
    let edge = a.y + (b.y - a.y) * t;
    let lo = min(edge, base_y);
    let hi = max(edge, base_y);
    return clamp(min(p.y - lo, hi - p.y) + 0.5, 0.0, 1.0);
}

// transform_helpers::gradient_t
fn gradient_t(y: f32, top: f32, bottom: f32) -> f32 {
    let h = bottom - top;
    if h <= 0.0 {
        return 0.0;
    }
    return clamp((y - top) / h, 0.0, 1.0);
}

// render-cpu `mix`: per-channel mix rounded to 8 bits.
fn mix8(a: vec4<f32>, b: vec4<f32>, t: f32) -> vec4<f32> {
    let x = floor(a * 255.0 + 0.5);
    let y = floor(b * 255.0 + 0.5);
    let m = floor(x + (y - x) * t + 0.5);
    return clamp(m, vec4<f32>(0.0), vec4<f32>(255.0)) / 255.0;
}

// transform_helpers::line_segments: vertices 0..6 draw segment 0 (bucket
// range), 6..12 segment 1, 12..18 segment 2 (step). `next` is element j + 1,
// read from the same chunk buffer one element further.
@vertex
fn vs_line_series(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
                  @location(0) cur: vec4<f32>, @location(1) next: vec4<f32>) -> SegOut {
    let d = i32(ii) + u.index_offset;
    let xj = x_center(d);
    let xn = x_center(d + 1);
    let hi = cur.y;
    let lo = cur.z;
    let last = cur.w;
    let first_n = next.x;
    let k = vi / 6u;
    var seg: vec4<f32>;
    if k == 0u {
        if is_gap(hi) || is_gap(lo) || bitcast<u32>(hi) == bitcast<u32>(lo) {
            return hidden_seg();
        }
        seg = vec4<f32>(xj, y_of(hi), xj, y_of(lo));
    } else {
        if is_gap(last) || is_gap(first_n) {
            return hidden_seg();
        }
        if (u.flags & STEP) != 0u {
            if k == 1u {
                seg = vec4<f32>(xj, y_of(last), xn, y_of(last));
            } else {
                seg = vec4<f32>(xn, y_of(last), xn, y_of(first_n));
            }
        } else {
            if k == 2u {
                return hidden_seg();
            }
            seg = vec4<f32>(xj, y_of(last), xn, y_of(first_n));
        }
    }
    let pad = u.body + 1.0;
    let lo_p = min(seg.xy, seg.zw) - vec2<f32>(pad);
    let hi_p = max(seg.xy, seg.zw) + vec2<f32>(pad);
    var out: SegOut;
    out.pos = quad(vec4<f32>(lo_p, hi_p - lo_p), vi % 6u);
    out.seg = seg;
    return out;
}

@fragment
fn fs_line_series(in: SegOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let hw = u.body;
    if !dash_on(p.x, hw * 2.0) {
        discard;
    }
    let below = (u.flags & BASELINE) != 0u && p.y >= u.base_y;
    let out = with_coverage(select(u.up, u.down, below), segment_coverage(p, in.seg.xy, in.seg.zw, hw));
    if out.a <= 0.0 {
        discard;
    }
    return out;
}

// ---- points ----------------------------------------------------------------

struct PointOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) center: vec2<f32>,
}

// transform_helpers::icon_coverage, ICON_CIRCLE branch.
fn circle_coverage(dx: f32, dy: f32, size: f32) -> f32 {
    let h = size * 0.5;
    let d = sqrt(dx * dx + dy * dy) - h;
    return clamp(0.5 - d, 0.0, 1.0);
}

// A dot `body` pixels across at the (last) value of each element.
@vertex
fn vs_points(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
             @location(0) v: vec4<f32>) -> PointOut {
    var out: PointOut;
    let last = v.w;
    if is_gap(last) {
        out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }
    let xc = x_center(i32(ii) + u.index_offset);
    let yc = y_of(last);
    let reach = u.body * 0.5 + 1.0;
    out.pos = quad(vec4<f32>(xc - reach, yc - reach, 2.0 * reach, 2.0 * reach), vi);
    out.center = vec2<f32>(xc, yc);
    return out;
}

@fragment
fn fs_points(in: PointOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let c = circle_coverage(p.x - in.center.x, p.y - in.center.y, u.body);
    let out = with_coverage(u.up, c);
    if out.a <= 0.0 {
        discard;
    }
    return out;
}

// ---- bands -----------------------------------------------------------------

struct BandOut {
    @builtin(position) pos: vec4<f32>,
    // Upper edge (xj, y0, xn, y1).
    @location(0) @interpolate(flat) seg: vec4<f32>,
    // Lower edge y at xj and xn.
    @location(1) @interpolate(flat) lower: vec2<f32>,
}

// transform_helpers::y with the lower series' bias (in `base_y`).
fn y_lower(v_rel: f32) -> f32 {
    let t = v_rel + u.base_y;
    return t * u.a;
}

// transform_helpers::band_coverage -> (coverage, crossed ? 1 : 0)
fn band_coverage(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, lower_a: f32, lower_b: f32,
                 scale: f32) -> vec2<f32> {
    if !(p.x >= a.x && p.x < b.x) {
        return vec2<f32>(0.0, 0.0);
    }
    let t = (p.x - a.x) / (b.x - a.x);
    let upper = a.y + (b.y - a.y) * t;
    let lower = lower_a + (lower_b - lower_a) * t;
    let lo = min(upper, lower);
    let hi = max(upper, lower);
    let coverage = clamp(min(p.y - lo, hi - p.y) + 0.5, 0.0, 1.0);
    return vec2<f32>(coverage, select(0.0, 1.0, (lower - upper) * scale > 0.0));
}

// Element j of a band: between last(j) -> first(j + 1) of both series.
@vertex
fn vs_band(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
           @location(0) cur: vec4<f32>, @location(1) next: vec4<f32>,
           @location(2) lower_cur: vec4<f32>, @location(3) lower_next: vec4<f32>) -> BandOut {
    var out: BandOut;
    let u0 = cur.w;
    let u1 = next.x;
    let l0 = lower_cur.w;
    let l1 = lower_next.x;
    if is_gap(u0) || is_gap(u1) || is_gap(l0) || is_gap(l1) {
        out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }
    let d = i32(ii) + u.index_offset;
    let xj = x_center(d);
    let xn = x_center(d + 1);
    let y0 = y_of(u0);
    let y1 = y_of(u1);
    let ya = y_lower(l0);
    let yb = y_lower(l1);
    let top = min(min(min(y0, y1), ya), yb) - 1.0;
    let bottom = max(max(max(y0, y1), ya), yb) + 1.0;
    let left = xj - 1.0;
    out.pos = quad(vec4<f32>(left, top, xn + 1.0 - left, bottom - top), vi);
    out.seg = vec4<f32>(xj, y0, xn, y1);
    out.lower = vec2<f32>(ya, yb);
    return out;
}

@fragment
fn fs_band(in: BandOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let r = band_coverage(p, in.seg.xy, in.seg.zw, in.lower.x, in.lower.y, u.a);
    // `up_wick`/`down_wick` carry `fill_top`/`fill_bottom`.
    let out = with_coverage(select(u.up_wick, u.down_wick, r.y > 0.5), r.x);
    if out.a <= 0.0 {
        discard;
    }
    return out;
}

// transform_helpers::fill_edge
@vertex
fn vs_fill_series(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32,
                  @location(0) cur: vec4<f32>, @location(1) next: vec4<f32>) -> SegOut {
    let last = cur.w;
    let first_n = next.x;
    if is_gap(last) || is_gap(first_n) {
        return hidden_seg();
    }
    let d = i32(ii) + u.index_offset;
    let xj = x_center(d);
    let xn = x_center(d + 1);
    let y0 = y_of(last);
    let y1 = select(y_of(first_n), y0, (u.flags & STEP) != 0u);
    let top = min(min(y0, y1), u.base_y) - 1.0;
    let bottom = max(max(y0, y1), u.base_y) + 1.0;
    let left = xj - 1.0;
    var out: SegOut;
    out.pos = quad(vec4<f32>(left, top, xn + 1.0 - left, bottom - top), vi);
    out.seg = vec4<f32>(xj, y0, xn, y1);
    return out;
}

@fragment
fn fs_fill_series(in: SegOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    var color: vec4<f32>;
    if (u.flags & AREA) != 0u {
        color = mix8(u.up_wick, u.down_wick, gradient_t(p.y, u.clip_y.x, u.clip_y.y));
    } else {
        color = select(u.down_wick, u.up_wick, p.y < u.base_y);
    }
    let out = with_coverage(color, fill_coverage(p, in.seg.xy, in.seg.zw, u.base_y));
    if out.a <= 0.0 {
        discard;
    }
    return out;
}
