// Heatmap aggregation: each visible column's
// cells raise the pixel columns it covers in its level's band to their
// value (atomic max on the bits of non-negative f32, which order like the
// floats). Same rules as `Heatmap::draw_cells` (portable path).

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
    stops: array<vec4<f32>, 5>,
};

@group(0) @binding(0) var<uniform> u: U;
// Per cell: (level as i32 bits, size as f32 bits).
@group(0) @binding(1) var<storage, read> cells: array<vec2<u32>>;
// Per column: index of its first cell; one more entry: the cell count.
@group(0) @binding(2) var<storage, read> col_start: array<u32>;
// [p0, p1) per visible column, then the band of each level, then the band
// of each plot row (-1: none).
@group(0) @binding(3) var<storage, read> pixel_map: array<u32>;
// Band × pixel column maxima.
@group(0) @binding(4) var<storage, read_write> grid: array<atomic<u32>>;

// One invocation per visible column (columns hold few cells: trades, or
// a book's levels).
@compute @workgroup_size(64)
fn aggregate(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let col = (wg.x + wg.y * 65535u) * 64u + li;
    if (col >= u.ncols) {
        return;
    }
    let p0 = pixel_map[2u * col];
    let p1 = pixel_map[2u * col + 1u];
    if (p0 >= p1) {
        return;
    }
    let point = u.first_col + col;
    let s = col_start[point];
    let e = col_start[point + 1u];
    for (var k = s; k < e; k += 1u) {
        let c = cells[k];
        let rel = bitcast<i32>(c.x) - u.level_lo;
        if (rel < 0 || u32(rel) >= u.nlevels) {
            continue;
        }
        let band = bitcast<i32>(pixel_map[u.off_level + u32(rel)]);
        let v = bitcast<f32>(c.y);
        if (band < 0 || !(v > 0.0)) {
            continue;
        }
        let base = u32(band) * u.w;
        for (var p = p0; p < p1; p += 1u) {
            atomicMax(&grid[base + p], c.y);
        }
    }
}
