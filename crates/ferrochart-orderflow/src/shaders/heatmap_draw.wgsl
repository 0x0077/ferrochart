// Heatmap coloring: one quad over the plot;
// each pixel reads its band's maximum and maps it through the color scale.
// `scale_t` and `ramp` follow `heatmap::scale_t`/`heatmap::ramp` operation
// by operation.

struct U {
    first_col: u32,
    ncols: u32,
    level_lo: i32,
    nlevels: u32,
    w: u32,
    h: u32,
    off_level: u32,
    off_row: u32,
    x0: f32,
    y0: f32,
    viewport: vec2<f32>,
    lo: f32,
    hi: f32,
    log_scale: f32,
    pad: f32,
    // Colors 0–255.
    stops: array<vec4<f32>, 5>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> pixel_map: array<u32>;
@group(0) @binding(2) var<storage, read> grid: array<u32>;

@vertex
fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let p = vec2(u.x0, u.y0) + corners[v] * vec2(f32(u.w), f32(u.h));
    let ndc = vec2(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0);
    return vec4(ndc, 0.0, 1.0);
}

fn scale_t(v: f32) -> f32 {
    var fv = v;
    var flo = u.lo;
    var fhi = u.hi;
    if (u.log_scale > 0.5) {
        fv = log(1.0 + v);
        flo = log(1.0 + u.lo);
        fhi = log(1.0 + u.hi);
    }
    let d = fhi - flo;
    if (!(d > 0.0)) {
        return 1.0;
    }
    return clamp((fv - flo) / d, 0.0, 1.0);
}

fn ramp(t: f32) -> vec4<f32> {
    let s = t * 4.0;
    let k = clamp(floor(s), 0.0, 3.0);
    let f = s - k;
    let i = u32(k);
    let a = u.stops[i];
    let b = u.stops[i + 1u];
    return floor(a + (b - a) * f + 0.5);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let col = i32(floor(pos.x - u.x0));
    let row = i32(floor(pos.y - u.y0));
    if (col < 0 || row < 0 || u32(col) >= u.w || u32(row) >= u.h) {
        discard;
    }
    let band = bitcast<i32>(pixel_map[u.off_row + u32(row)]);
    if (band < 0) {
        discard;
    }
    let v = bitcast<f32>(grid[u32(band) * u.w + u32(col)]);
    if (!(v > 0.0)) {
        discard;
    }
    return ramp(scale_t(v)) / 255.0;
}
