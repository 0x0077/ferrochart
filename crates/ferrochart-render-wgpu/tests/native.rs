//! Native custom series: a plugin drawn by its
//! own wgpu pipeline must match its portable path (immediate primitives)
//! within the golden-image tolerance.
//!
//! Needs a GPU adapter; without one the test prints a notice and passes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::any::Any;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use ferrochart_core::chart::{ChartDataView, SeriesData, SeriesOptions};
use ferrochart_core::data::{DataChange, Timestamp};
use ferrochart_core::drawing::FrameBuilder;
use ferrochart_core::frame::{CustomDraw, SeriesKind};
use ferrochart_core::geom::{RectPx, Rgba, SizeCss};
use ferrochart_core::plugin::{AxisMode, CustomSeries, MinMaxIndex, SeriesCtx};
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk;
use ferrochart_core::transform_helpers::y_of_price;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, GpuSeries, GpuSeriesCtx, WgpuRenderer};
use wgpu::util::DeviceExt;

const W: f32 = 800.0;
const H: f32 = 450.0;
const CHANNEL_TOLERANCE: u8 = 2;
const PIXEL_FRACTION: f64 = 0.001;
const BAR_COLOR: u32 = 0x007B_1FA2;

/// `BarsGpu::prepare` calls.
static PREPARED: AtomicUsize = AtomicUsize::new(0);

fn ctx() -> Option<&'static GpuContext> {
    static CTX: OnceLock<Option<GpuContext>> = OnceLock::new();
    CTX.get_or_init(|| match GpuContext::headless() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("no GPU adapter ({e}); native series test skipped");
            None
        }
    })
    .as_ref()
}

/// The rectangle of one bar; shared by both paths, so they differ only in
/// how x and y are computed (f64 on the portable path, the f32 transforms
/// of `CustomDraw` natively).
fn bar_rect(x: f32, y_hi: f32, y_lo: f32) -> RectPx {
    let (y0, y1) = (y_hi.round(), y_lo.round());
    RectPx {
        x: x.round() - 2.0,
        y: y0,
        w: 5.0,
        h: (y1 - y0).max(1.0),
    }
}

/// A vertical bar from `lo` to `hi` per point.
#[derive(Default)]
struct Bars {
    time: Vec<Timestamp>,
    range: Vec<(f64, f64)>,
    index: MinMaxIndex,
}

impl Bars {
    fn push(&mut self, t: Timestamp, lo: f64, hi: f64) {
        self.time.push(t);
        self.range.push((lo, hi));
        self.index.push(lo, hi).unwrap();
    }
}

impl CustomSeries for Bars {
    fn kind(&self) -> &'static str {
        "test.bars"
    }
    fn axis_mode(&self) -> AxisMode {
        AxisMode::Slots
    }
    fn times(&self) -> &[Timestamp] {
        &self.time
    }
    fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.index.query(from, to)
    }
    fn build(&self, ctx: &SeriesCtx<'_>, from: usize, to: usize, out: &mut FrameBuilder<'_>) {
        for i in from..to {
            let (lo, hi) = self.range[i];
            let (Some(x), Some(y0), Some(y1)) = (
                ctx.x_of_point(i),
                ctx.coords().y_of(hi),
                ctx.coords().y_of(lo),
            ) else {
                continue;
            };
            out.rect(bar_rect(x, y0, y1), ctx.style().up);
        }
    }
}

const SHADER: &str = r"
struct U { viewport: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var<uniform> u: U;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) v: u32, @location(0) rect: vec4<f32>, @location(1) color: vec4<f32>) -> Out {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let p = rect.xy + corners[v] * rect.zw;
    let ndc = vec2(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0);
    return Out(vec4(ndc, 0.0, 1.0), color);
}

@fragment
fn fs(o: Out) -> @location(0) vec4<f32> {
    return o.color;
}
";

/// `[x, y, w, h, r, g, b, a]`.
type Instance = [f32; 8];

/// The native implementation of [`Bars`].
struct BarsGpu {
    prepared: &'static AtomicUsize,
    pipeline: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    /// Instances and the viewport bind group of the last `prepare`.
    state: Option<(wgpu::Buffer, wgpu::BindGroup, u32)>,
    scratch: Vec<Instance>,
}

impl BarsGpu {
    fn pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test bars"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("test bars"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test bars"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("test bars"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
        (pipeline, layout)
    }
}

impl GpuSeries for BarsGpu {
    fn kind(&self) -> &'static str {
        "test.bars"
    }

    fn prepare(&mut self, ctx: &GpuSeriesCtx<'_>, draw: &CustomDraw, data: &ChartDataView<'_>) {
        self.prepared.fetch_add(1, Ordering::Relaxed);
        let (_, layout) = self
            .pipeline
            .get_or_insert_with(|| Self::pipeline(ctx.device, ctx.format));
        let any: &dyn Any = data.custom(draw.series).unwrap();
        let bars = any.downcast_ref::<Bars>().unwrap();
        let c = ctx.frame.styles[draw.style.0 as usize].up;
        let color = [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0);
        let xt = draw.x_transform;
        self.scratch.clear();
        for i in draw.points.start..draw.points.end {
            let i = i as usize;
            let (lo, hi) = bars.range[i];
            let idx = data.custom_index(draw.series, i).unwrap();
            let x = ((idx - xt.base as f64) as f32) * xt.step + xt.offset;
            let r = bar_rect(
                x,
                y_of_price(&draw.y_transform, hi),
                y_of_price(&draw.y_transform, lo),
            );
            self.scratch
                .push([r.x, r.y, r.w, r.h, color[0], color[1], color[2], color[3]]);
        }
        let instances = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("test bars"),
                contents: bytemuck::cast_slice(&self.scratch),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let viewport = [ctx.viewport[0], ctx.viewport[1], 0.0, 0.0];
        let uniform = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("test bars viewport"),
                contents: bytemuck::cast_slice(&viewport),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("test bars"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let count = u32::try_from(self.scratch.len()).unwrap();
        self.state = Some((instances, bind, count));
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, _: &CustomDraw) {
        let (Some((pipeline, _)), Some((instances, bind, count))) = (&self.pipeline, &self.state)
        else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, instances.slice(..));
        pass.draw(0..6, 0..*count);
    }
}

fn scene() -> Chart {
    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: W, h: H }, 1.0);
    let pane = chart.main_pane();
    let candles = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    let data = random_walk(7, 80, 1_704_067_200_000, 3_600_000).unwrap();
    let mut bars = Bars::default();
    for i in (0..data.time.len()).step_by(3) {
        bars.push(data.time[i], data.low[i] - 0.5, data.high[i] + 0.5);
    }
    chart.set_data(candles, SeriesData::Ohlc(data)).unwrap();
    let opts = SeriesOptions {
        color: Some(Rgba::hex(BAR_COLOR)),
        ..SeriesOptions::default()
    };
    let id = chart.add_custom_series(pane, Box::new(bars), opts).unwrap();
    // A real-time change, so the frame is not the first generation.
    chart
        .update_custom(id, |b: &mut Bars| {
            let last = b.range.len() - 1;
            b.range[last].1 += 1.0;
            b.index.set_last(b.range[last].0, b.range[last].1);
            DataChange::UpdateLast
        })
        .unwrap();
    chart.fit_content();
    chart
}

fn render(chart: &mut Chart, renderer: &mut WgpuRenderer) -> Vec<u8> {
    chart.set_native_kinds(renderer.native_kinds());
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    renderer.read_pixels().unwrap()
}

#[test]
fn native_series_match_the_portable_path() {
    let Some(ctx) = ctx() else {
        return;
    };
    let mut chart = scene();
    let portable = render(&mut chart, &mut WgpuRenderer::offscreen(ctx));

    let mut renderer = WgpuRenderer::offscreen(ctx);
    renderer.register_series(Box::new(BarsGpu {
        prepared: &PREPARED,
        pipeline: None,
        state: None,
        scratch: Vec::new(),
    }));
    assert_eq!(renderer.native_kinds(), &["test.bars"]);
    let native = render(&mut chart, &mut renderer);
    assert_eq!(PREPARED.load(Ordering::Relaxed), 1);
    assert!(renderer.take_gpu_error().is_none());

    let c = Rgba::hex(BAR_COLOR);
    let bar_pixels = |img: &[u8]| {
        img.chunks_exact(4)
            .filter(|p| p[..3] == [c.r, c.g, c.b])
            .count()
    };
    assert!(bar_pixels(&portable) > 1000, "the bars are drawn");
    let differing = portable
        .chunks_exact(4)
        .zip(native.chunks_exact(4))
        .filter(|(x, y)| {
            x.iter()
                .zip(y.iter())
                .any(|(p, q)| p.abs_diff(*q) > CHANNEL_TOLERANCE)
        })
        .count();
    let limit = f64::from(W * H) * PIXEL_FRACTION;
    assert!(
        differing as f64 <= limit,
        "{differing} pixels differ (limit {limit:.0})"
    );
    assert_eq!(bar_pixels(&portable), bar_pixels(&native));
}
