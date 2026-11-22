// Immediate primitives: rectangles, anti-aliased (dashed) lines, glyphs, and
// the composite of the cached main layer.
//
// Rectangles and glyph quads arrive already snapped to whole pixels, matching
// the CPU backend's rasterization; only slanted or dashed lines are
// anti-aliased.

// The first 16 bytes of the per-draw uniform (`DrawUniform`). Uniform structs
// must be a multiple of 16 bytes on WebGL2.
struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> g: Globals;

const CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
);

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, 0.0, 1.0);
}

// ---- rects -----------------------------------------------------------------

struct RectOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

@vertex
fn vs_rect(@builtin(vertex_index) vi: u32, @location(0) rect: vec4<f32>,
           @location(1) color: vec4<f32>) -> RectOut {
    var out: RectOut;
    out.pos = to_clip(rect.xy + CORNERS[vi] * rect.zw);
    out.color = color;
    return out;
}

@fragment
fn fs_rect(in: RectOut) -> @location(0) vec4<f32> {
    return in.color;
}

// ---- triangles (polygon fills) ---------------------------------------------

@vertex
fn vs_triangle(@builtin(vertex_index) vi: u32, @location(0) ab: vec4<f32>,
               @location(1) c: vec2<f32>, @location(2) color: vec4<f32>) -> RectOut {
    var p = c;
    if vi == 0u {
        p = ab.xy;
    } else if vi == 1u {
        p = ab.zw;
    }
    var out: RectOut;
    out.pos = to_clip(p);
    out.color = color;
    return out;
}

// ---- lines -----------------------------------------------------------------

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    // (distance along the segment, distance from its axis)
    @location(1) local: vec2<f32>,
    // (length, half width, dash kind)
    @location(2) @interpolate(flat) shape: vec3<f32>,
}

@vertex
fn vs_line(@builtin(vertex_index) vi: u32, @location(0) ends: vec4<f32>,
           @location(1) width: f32, @location(2) dash: u32,
           @location(3) color: vec4<f32>) -> LineOut {
    let a = ends.xy;
    let b = ends.zw;
    let len = max(length(b - a), 1e-4);
    let dir = (b - a) / len;
    let normal = vec2<f32>(-dir.y, dir.x);
    let half = width * 0.5;
    let c = CORNERS[vi];
    // One pixel of fringe for anti-aliasing.
    let along = mix(-1.0, len + 1.0, c.x);
    let across = mix(-(half + 1.0), half + 1.0, c.y);
    var out: LineOut;
    out.pos = to_clip(a + dir * along + normal * across);
    out.color = color;
    out.local = vec2<f32>(along, across);
    out.shape = vec3<f32>(len, half, f32(dash));
    return out;
}

// Mirrors `transform_helpers::line_coverage` (same f32 operation order).
fn line_coverage(along: f32, across: f32, len: f32, width: f32, dash: f32) -> f32 {
    let half = width * 0.5;
    let cov_across = clamp(half + 0.5 - abs(across), 0.0, 1.0);
    let cov_along = clamp(min(along, len - along) + 0.5, 0.0, 1.0);
    var on = true;
    if dash == 1.0 {
        // Dashed: 4w on, 4w off.
        let t = along / (8.0 * width);
        on = t - floor(t) < 0.5;
    } else if dash == 2.0 {
        // Dotted: w on, 2w off.
        let t = along / (3.0 * width);
        on = t - floor(t) < 1.0 / 3.0;
    }
    return select(0.0, cov_across * cov_along, on);
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    let cov = line_coverage(in.local.x, in.local.y, in.shape.x, in.shape.y * 2.0, in.shape.z);
    let a = in.color.a * cov;
    if a <= 0.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb, a);
}

// ---- icons -----------------------------------------------------------------

struct IconOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    // (center, size, icon)
    @location(1) @interpolate(flat) shape: vec4<f32>,
}

@vertex
fn vs_icon(@builtin(vertex_index) vi: u32, @location(0) center: vec2<f32>,
           @location(1) size: f32, @location(2) icon: u32,
           @location(3) color: vec4<f32>) -> IconOut {
    // One pixel of fringe; covers the same pixels as the CPU backend's box.
    let reach = size * 0.5 + 1.0;
    let lo = floor(center - vec2<f32>(reach));
    let hi = ceil(center + vec2<f32>(reach));
    var out: IconOut;
    out.pos = to_clip(lo + CORNERS[vi] * (hi - lo));
    out.color = color;
    out.shape = vec4<f32>(center, size, f32(icon));
    return out;
}

// Mirrors `transform_helpers::icon_coverage` line by line.
fn icon_coverage(icon: u32, dx: f32, dy: f32, size: f32) -> f32 {
    let h = size * 0.5;
    let ax = abs(dx);
    var d: f32;
    if icon == 0u {
        d = sqrt(dx * dx + dy * dy) - h;
    } else if icon == 1u {
        let s = h * 0.8;
        d = max(ax - s, abs(dy) - s);
    } else if icon == 2u || icon == 3u {
        let ty = select(-dy, dy, icon == 2u);
        let head = max(ty, (ax - ty - h) * 0.70710677);
        let q = h * 0.5;
        let shaft = max(ax - h * 0.35, abs(ty - q) - q);
        d = min(head, shaft);
    } else {
        return 0.0;
    }
    return clamp(0.5 - d, 0.0, 1.0);
}

@fragment
fn fs_icon(in: IconOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy - in.shape.xy;
    let coverage = icon_coverage(u32(in.shape.w), p.x, p.y, in.shape.z);
    // Same integer math as the CPU backend: alpha = a * coverage / 255.
    let a8 = floor(in.color.a * 255.0 + 0.5);
    let c8 = clamp(floor(coverage * 255.0 + 0.5), 0.0, 255.0);
    let a = floor(a8 * c8 / 255.0) / 255.0;
    if a <= 0.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb, a);
}

// ---- text ------------------------------------------------------------------

@group(1) @binding(0) var tex: texture_2d<f32>;

struct GlyphOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    // (dst origin, atlas origin)
    @location(1) @interpolate(flat) origin: vec4<f32>,
}

@vertex
fn vs_glyph(@builtin(vertex_index) vi: u32, @location(0) dst: vec4<f32>,
            @location(1) src: vec2<f32>, @location(2) color: vec4<f32>) -> GlyphOut {
    var out: GlyphOut;
    out.pos = to_clip(dst.xy + CORNERS[vi] * dst.zw);
    out.color = color;
    out.origin = vec4<f32>(dst.xy, src);
    return out;
}

@fragment
fn fs_glyph(in: GlyphOut) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(floor(in.pos.xy - in.origin.xy + in.origin.zw));
    let coverage = textureLoad(tex, texel, 0).r;
    // Same integer math as the CPU backend: alpha = a * coverage / 255.
    let a8 = floor(in.color.a * 255.0 + 0.5);
    let c8 = floor(coverage * 255.0 + 0.5);
    let a = floor(a8 * c8 / 255.0) / 255.0;
    if a <= 0.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb, a);
}

// ---- composite -------------------------------------------------------------

struct FullOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FullOut {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: FullOut;
    out.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_composite(in: FullOut) -> @location(0) vec4<f32> {
    return textureLoad(tex, vec2<i32>(in.pos.xy), 0);
}
