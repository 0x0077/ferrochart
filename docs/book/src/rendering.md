# How rendering works

## The loop

Every binding runs the same loop:

```rust,ignore
if chart.needs_redraw() != Invalidation::None {
    chart.build_frame(now_ms, &renderer.measurer()); // &mut chart, ends here
    renderer.render(chart.frame(), &chart.data_view())?; // two shared borrows
}
```

`needs_redraw` stays `None` while nothing changes, so an idle chart costs
nothing. Building a frame takes about 20 µs, independent of the data size.

## Frames

A `Frame` has three layers — background (grid), main (series, axes) and overlay
(crosshair) — each a flat list of draw commands with a revision number.
Series are not expanded into rectangles: the frame says *draw this series,
visible range i..j, with this transform*, and the renderer reads the data
columns directly.

When only the crosshair moves, only the overlay's revision changes and the GPU
renderer reuses a cached texture of the other two layers.

## Data on the GPU

The wgpu renderer keeps series data in GPU buffers, in chunks of 4096 bars. It
uploads a chunk the first time it is drawn and afterwards only the ranges that
changed. Panning and zooming change a few numbers per chunk; nothing is
re-uploaded.

Prices are stored relative to a reference price per chunk, so 32-bit floats on
the GPU keep full precision even for large prices with small ticks.

## Same pixels everywhere

The coordinate math lives in `ferrochart_core::transform_helpers` and is
mirrored operation by operation in the WGSL shaders, so the CPU and GPU
renderers round identically. Golden-image tests compare both against the same
reference images. (One subtlety: GPU compilers may fuse `a * b + c` into a
single instruction with different rounding, so the transforms are written as
`(a + b) * c`, which cannot be fused.)
