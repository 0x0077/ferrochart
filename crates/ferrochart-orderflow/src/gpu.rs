//! Native wgpu drawing of the heatmap (feature `gpu`).
//!
//! The cells live in storage buffers, uploaded incrementally from the
//! series' change log. Each frame a compute shader aggregates the visible
//! columns into a "band × pixel column" buffer (atomic max), and one quad
//! over the plot colors each pixel from it. The pixel layout and the color
//! range come from the core side ([`Heatmap`]'s layout and
//! `CustomDraw::params`), so the output matches the portable path; the
//! shaders contain no chart logic.

use core::any::Any;

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::data::{DataDelta, SeriesId};
use ferrochart_core::frame::CustomDraw;
use ferrochart_render_wgpu::{GpuContext, GpuSeries, GpuSeriesCtx, WgpuRenderer};

use crate::heatmap::{HEATMAP_KIND, Heatmap};

const AGGREGATE_WGSL: &str = include_str!("shaders/heatmap_aggregate.wgsl");
const DRAW_WGSL: &str = include_str!("shaders/heatmap_draw.wgsl");

/// Storage buffers the compute stage binds.
const STORAGE_BUFFERS: u32 = 4;
/// Workgroups per dispatch dimension (the WebGPU minimum limit).
const MAX_GROUPS: u32 = 65_535;

/// Whether `ctx`'s device can run the native heatmap: compute shaders,
/// storage buffers in fragment shaders and at least four storage buffers
/// per stage. Devices created with the WebGL2 limits (e.g. by a host that
/// asked for them) cannot.
pub fn supports_native_heatmap(ctx: &GpuContext) -> bool {
    let flags = ctx.adapter.get_downlevel_capabilities().flags;
    let limits = ctx.device.limits();
    flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS | wgpu::DownlevelFlags::FRAGMENT_STORAGE)
        && limits.max_storage_buffers_per_shader_stage >= STORAGE_BUFFERS
        && limits.max_compute_workgroups_per_dimension >= MAX_GROUPS
        && limits.max_compute_invocations_per_workgroup >= 64
}

/// Registers the native order flow series that `ctx`'s device supports
/// with `renderer` (pass the context the renderer was created with).
/// Returns whether the native heatmap was registered; without it the
/// heatmap draws through its portable path.
///
/// # Examples
///
/// ```no_run
/// use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
///
/// let ctx = GpuContext::headless()?;
/// let mut renderer = WgpuRenderer::offscreen(&ctx);
/// ferrochart_orderflow::register_gpu(&ctx, &mut renderer);
/// // Then, before each frame: chart.set_native_kinds(renderer.native_kinds()).
/// # Ok::<(), ferrochart_core::render::RenderError>(())
/// ```
pub fn register_gpu(ctx: &GpuContext, renderer: &mut WgpuRenderer) -> bool {
    if !supports_native_heatmap(ctx) {
        return false;
    }
    renderer.register_series(Box::new(HeatmapGpu::default()));
    true
}

/// The native implementation of [`Heatmap`] (kind
/// [`HEATMAP_KIND`]); register it with [`register_gpu`], which checks
/// that the device supports it.
#[derive(Default)]
pub struct HeatmapGpu {
    pipes: Option<Pipes>,
    series: Vec<(SeriesId, SeriesGpu)>,
    /// Cells interleaved for upload.
    scratch: Vec<[u32; 2]>,
}

impl core::fmt::Debug for HeatmapGpu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HeatmapGpu")
            .field("series", &self.series.len())
            .finish_non_exhaustive()
    }
}

struct Pipes {
    format: wgpu::TextureFormat,
    aggregate: wgpu::ComputePipeline,
    aggregate_layout: wgpu::BindGroupLayout,
    draw: wgpu::RenderPipeline,
    draw_layout: wgpu::BindGroupLayout,
}

/// A buffer that grows by powers of two.
struct Grow {
    buffer: Option<wgpu::Buffer>,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Grow {
    fn new(usage: wgpu::BufferUsages, label: &'static str) -> Self {
        Self {
            buffer: None,
            usage,
            label,
        }
    }

    /// Makes room for `bytes`; returns whether the buffer was replaced
    /// (its contents are lost).
    fn reserve(&mut self, device: &wgpu::Device, bytes: u64) -> bool {
        if self.buffer.as_ref().is_some_and(|b| b.size() >= bytes) {
            return false;
        }
        let size = bytes.max(256).next_power_of_two();
        self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(self.label),
            size,
            usage: self.usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        true
    }
}

struct SeriesGpu {
    /// Generation of the cells on the GPU.
    synced: Option<u64>,
    cells: Grow,
    col_start: Grow,
    layout: Grow,
    grid: Grow,
    uniform: Grow,
    binds: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
    /// Whether the last `prepare` left something to draw.
    active: bool,
}

impl SeriesGpu {
    fn new() -> Self {
        let storage = wgpu::BufferUsages::STORAGE;
        Self {
            synced: None,
            cells: Grow::new(storage, "heatmap cells"),
            col_start: Grow::new(storage, "heatmap columns"),
            layout: Grow::new(storage, "heatmap layout"),
            grid: Grow::new(storage, "heatmap grid"),
            uniform: Grow::new(wgpu::BufferUsages::UNIFORM, "heatmap uniform"),
            binds: None,
            active: false,
        }
    }
}

fn layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BufferBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl Pipes {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        use wgpu::BufferBindingType as B;
        use wgpu::ShaderStages as S;
        let ro = B::Storage { read_only: true };
        let aggregate_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heatmap aggregate"),
            entries: &[
                layout_entry(0, S::COMPUTE, B::Uniform),
                layout_entry(1, S::COMPUTE, ro),
                layout_entry(2, S::COMPUTE, ro),
                layout_entry(3, S::COMPUTE, ro),
                layout_entry(4, S::COMPUTE, B::Storage { read_only: false }),
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heatmap draw"),
            entries: &[
                layout_entry(0, S::VERTEX | S::FRAGMENT, B::Uniform),
                layout_entry(1, S::FRAGMENT, ro),
                layout_entry(2, S::FRAGMENT, ro),
            ],
        });
        let module = |label, src: &'static str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let pipeline_layout = |label, layout: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            })
        };
        let aggregate_module = module("heatmap aggregate", AGGREGATE_WGSL);
        let aggregate = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("heatmap aggregate"),
            layout: Some(&pipeline_layout("heatmap aggregate", &aggregate_layout)),
            module: &aggregate_module,
            entry_point: Some("aggregate"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let draw_module = module("heatmap draw", DRAW_WGSL);
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("heatmap draw"),
            layout: Some(&pipeline_layout("heatmap draw", &draw_layout)),
            vertex: wgpu::VertexState {
                module: &draw_module,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_module,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                // Blended like the renderer's rectangles.
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            format,
            aggregate,
            aggregate_layout,
            draw,
            draw_layout,
        }
    }
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn bytes<T>(n: usize) -> u64 {
    (n * size_of::<T>()) as u64
}

impl HeatmapGpu {
    /// Uploads the cells changed since the last sync; returns whether a
    /// buffer was replaced.
    fn upload_cells(
        scratch: &mut Vec<[u32; 2]>,
        s: &mut SeriesGpu,
        ctx: &GpuSeriesCtx<'_>,
        draw: &CustomDraw,
        data: ChartDataView<'_>,
        heatmap: &Heatmap,
    ) -> bool {
        let (col_start, level, a, _) = heatmap.grid().raw();
        let (cols, cells) = (col_start.len(), level.len());
        let mut from = match s.synced.map(|g| data.changes_since(draw.series, g)) {
            None | Some(DataDelta::Full) => 0,
            Some(DataDelta::None) => cols,
            Some(DataDelta::Range(span)) => usize::try_from(span.start.max(0)).unwrap_or(0),
        };
        let mut replaced = s.cells.reserve(ctx.device, bytes::<[u32; 2]>(cells.max(1)));
        replaced |= s.col_start.reserve(ctx.device, bytes::<u32>(cols + 1));
        if replaced {
            from = 0;
        }
        s.synced = Some(draw.generation);
        let (Some(cell_buf), Some(col_buf)) = (&s.cells.buffer, &s.col_start.buffer) else {
            return replaced;
        };
        if from >= cols {
            return false;
        }
        let first_cell = col_start.get(from).map_or(cells, |&c| c as usize);
        scratch.clear();
        scratch.extend(
            level[first_cell..]
                .iter()
                .zip(&a[first_cell..])
                .map(|(&l, &v)| [l.cast_unsigned(), v.to_bits()]),
        );
        if !scratch.is_empty() {
            ctx.queue.write_buffer(
                cell_buf,
                bytes::<[u32; 2]>(first_cell),
                bytemuck::cast_slice(scratch),
            );
        }
        if from < cols {
            ctx.queue.write_buffer(
                col_buf,
                bytes::<u32>(from),
                bytemuck::cast_slice(&col_start[from..]),
            );
        }
        // The end of the last column.
        let total = u32::try_from(cells).unwrap_or(u32::MAX);
        ctx.queue
            .write_buffer(col_buf, bytes::<u32>(cols), bytemuck::cast_slice(&[total]));
        replaced
    }
}

impl GpuSeries for HeatmapGpu {
    fn kind(&self) -> &'static str {
        HEATMAP_KIND
    }

    #[allow(clippy::too_many_lines)]
    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        if self.pipes.as_ref().is_none_or(|p| p.format != ctx.format) {
            self.pipes = Some(Pipes::new(ctx.device, ctx.format));
            for (_, s) in &mut self.series {
                s.binds = None;
            }
        }
        let Some(pipes) = &self.pipes else {
            return;
        };
        let n = if let Some(n) = self.series.iter().position(|(id, _)| *id == draw.series) {
            n
        } else {
            self.series.push((draw.series, SeriesGpu::new()));
            self.series.len() - 1
        };
        let s = &mut self.series[n].1;
        s.active = false;
        let Some(any) = data.custom(draw.series) else {
            return;
        };
        let any: &dyn Any = any;
        let Some(heatmap) = any.downcast_ref::<Heatmap>() else {
            return;
        };
        let Some(l) = heatmap.layout() else {
            return;
        };
        let (ncols, nlevels, bands) = (l.col_px.len(), l.level_band.len(), l.band_rows.len());
        if !l.valid || ncols == 0 || bands == 0 {
            return;
        }
        let limits = ctx.device.limits();
        let grid_bytes = bytes::<u32>(l.w as usize * bands);
        if grid_bytes > limits.max_storage_buffer_binding_size || u32::try_from(ncols).is_err() {
            return;
        }

        let mut replaced = Self::upload_cells(&mut self.scratch, s, ctx, draw, *data, heatmap);

        // Layout: column pixel ranges, level bands, row bands.
        let off_level = 2 * ncols;
        let off_row = off_level + nlevels;
        replaced |= s
            .layout
            .reserve(ctx.device, bytes::<u32>(off_row + l.row_band.len()));
        replaced |= s.grid.reserve(ctx.device, grid_bytes);
        replaced |= s.uniform.reserve(ctx.device, bytes::<u32>(UNIFORM_WORDS));
        let (Some(cells), Some(cols), Some(layout), Some(grid), Some(uniform)) = (
            &s.cells.buffer,
            &s.col_start.buffer,
            &s.layout.buffer,
            &s.grid.buffer,
            &s.uniform.buffer,
        ) else {
            return;
        };
        ctx.queue
            .write_buffer(layout, 0, bytemuck::cast_slice(&l.col_px));
        ctx.queue.write_buffer(
            layout,
            bytes::<u32>(off_level),
            bytemuck::cast_slice(&l.level_band),
        );
        ctx.queue.write_buffer(
            layout,
            bytes::<u32>(off_row),
            bytemuck::cast_slice(&l.row_band),
        );

        let stops = heatmap.options().colors.stops;
        let mut u = [0_u32; UNIFORM_WORDS];
        let f = f32::to_bits;
        u[..16].copy_from_slice(&[
            u32::try_from(l.first).unwrap_or(u32::MAX),
            ncols as u32,
            l.level_lo.cast_unsigned(),
            nlevels as u32,
            l.w,
            l.h,
            off_level as u32,
            off_row as u32,
            f(l.x0 as f32),
            f(l.y0 as f32),
            f(ctx.viewport[0]),
            f(ctx.viewport[1]),
            f(draw.params[0]),
            f(draw.params[1]),
            f(draw.params[2]),
            0,
        ]);
        for (k, c) in stops.iter().enumerate() {
            u[16 + 4 * k..20 + 4 * k]
                .copy_from_slice(&[c.r, c.g, c.b, c.a].map(|v| f(f32::from(v))));
        }
        ctx.queue.write_buffer(uniform, 0, bytemuck::cast_slice(&u));

        if replaced || s.binds.is_none() {
            let aggregate = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("heatmap aggregate"),
                layout: &pipes.aggregate_layout,
                entries: &[
                    entry(0, uniform),
                    entry(1, cells),
                    entry(2, cols),
                    entry(3, layout),
                    entry(4, grid),
                ],
            });
            let draw_bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("heatmap draw"),
                layout: &pipes.draw_layout,
                entries: &[entry(0, uniform), entry(1, layout), entry(2, grid)],
            });
            s.binds = Some((aggregate, draw_bind));
        }
        let Some((aggregate, _)) = &s.binds else {
            return;
        };

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("heatmap aggregate"),
            });
        encoder.clear_buffer(grid, 0, Some(grid_bytes));
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("heatmap aggregate"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipes.aggregate);
            pass.set_bind_group(0, aggregate, &[]);
            // 64 columns per workgroup.
            let groups = (ncols as u32).div_ceil(64);
            pass.dispatch_workgroups(groups.min(MAX_GROUPS), groups.div_ceil(MAX_GROUPS), 1);
        }
        ctx.queue.submit([encoder.finish()]);
        s.active = true;
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &CustomDraw) {
        let Some(pipes) = &self.pipes else {
            return;
        };
        let Some((_, s)) = self.series.iter().find(|(id, _)| *id == draw.series) else {
            return;
        };
        let (true, Some((_, bind))) = (s.active, &s.binds) else {
            return;
        };
        pass.set_pipeline(&pipes.draw);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// 16 scalars and 5 color stops.
const UNIFORM_WORDS: usize = 16 + 4 * 5;
