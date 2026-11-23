//! wgpu rendering backend for FerroChart: the primary backend for desktop and
//! the web (Vulkan, Metal, DX12, WebGPU; WebGL2 with the `webgl` feature).
//!
//! - **Series data lives on the GPU** in chunks of 4096 elements with frozen
//!   f64 reference prices. Only chunks that are drawn are uploaded; data
//!   changes are pulled with `changes_since` and only dirty ranges rewritten.
//!   Panning and zooming update a few uniforms and issue one instanced draw per
//!   visible chunk.
//! - **Shaders mirror `transform_helpers`** operation by operation
//!   (`src/shaders/series.wgsl`), so this backend matches the CPU backend's
//!   golden images.
//! - **Layer cache**: background and main layers are rendered into an
//!   offscreen texture that is reused while their revisions do not change;
//!   crosshair-only frames redraw just the overlay on top of it.
//! - Vertex buffers are bound at the first visible element of each chunk and
//!   instances always start at 0 (no `first_instance`, so WebGL2 works the
//!   same way); no storage buffers are used.
//!
//! # Examples
//!
//! Offscreen rendering with read-back (the same mode tests use):
//!
//! ```no_run
//! use ferrochart_core::chart::{SeriesData, SeriesOptions};
//! use ferrochart_core::frame::SeriesKind;
//! use ferrochart_core::geom::SizeCss;
//! use ferrochart_core::render::Renderer;
//! use ferrochart_core::testing::random_walk;
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
//!
//! let ctx = GpuContext::headless()?;
//! let mut renderer = WgpuRenderer::offscreen(&ctx);
//! let mut chart = Chart::new(ChartOptions::default());
//! chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
//! let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
//! chart.set_data(id, SeriesData::Ohlc(random_walk(1, 1000, 0, 60_000)?))?;
//! chart.build_frame(0.0, &renderer.measurer());
//! renderer.render(chart.frame(), &chart.data_view())?;
//! let rgba = renderer.read_pixels()?;
//! assert_eq!(rgba.len(), 800 * 450 * 4);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
#![forbid(unsafe_code)]

mod atlas;
mod icon_atlas;
mod native;
mod pipelines;
mod series;

pub use native::{GpuSeries, GpuSeriesCtx};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::data::OhlcLevelView;
use ferrochart_core::frame::{
    CustomDraw, DrawCmd, FontStyleRef, Frame, Layer, LineDash, LineInst, PriceMapping, SeriesDraw,
    SeriesKind, SeriesStyle, TextAlign, TextRun,
};
use ferrochart_core::geom::{PointPx, RectPx, Rgba, SizePx};
use ferrochart_core::render::{RenderError, Renderer};
use ferrochart_core::text::TextMeasurer;
use ferrochart_core::transform_helpers::{
    CHUNK_LEN, bar_metrics, chunk_of, chunk_start, x_shift, y_bias, y_of_price,
};
use ferrochart_text::TextEngine;

use crate::atlas::Atlas;
use crate::icon_atlas::IconAtlas;
use crate::pipelines::{
    DrawUniform, GlyphInstance, IconInstance, LineInstance, Pipelines, RectInstance,
    RichIconInstance, TriangleInstance,
};
use crate::series::{COLOR_BYTES, Scratch, SeriesGpu, ohlc_offset, value_offset};

/// Polls a future to completion on the current thread. wgpu's native futures
/// are ready immediately; this avoids an executor dependency.
#[cfg(not(target_arch = "wasm32"))]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// A wgpu instance, adapter, device and queue.
#[derive(Debug)]
pub struct GpuContext {
    /// The instance.
    pub instance: wgpu::Instance,
    /// The adapter.
    pub adapter: wgpu::Adapter,
    /// The device.
    pub device: wgpu::Device,
    /// The queue.
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// A context without a window, for offscreen rendering. Blocks until the
    /// adapter and device are ready (native only; use [`request`](Self::request)
    /// on the web).
    ///
    /// # Errors
    ///
    /// [`RenderError::Backend`] if no adapter or device is available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        Self::with_instance(instance, None)
    }

    /// Blocking [`request`](Self::request) (native only: on the web the
    /// adapter and device are JS promises that only resolve after returning
    /// to the event loop).
    ///
    /// # Errors
    ///
    /// [`RenderError::Backend`] if no adapter or device is available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_instance(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        block_on(Self::request(instance, surface))
    }

    /// A context on `instance` whose adapter can present to `surface` (if
    /// given).
    ///
    /// # Errors
    ///
    /// [`RenderError::Backend`] if no adapter or device is available.
    pub async fn request(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: surface,
                ..wgpu::RequestAdapterOptions::default()
            })
            .await
            .map_err(|_| RenderError::Backend("no suitable GPU adapter"))?;
        Self::from_adapter(instance, adapter).await
    }

    /// A context on an adapter obtained from `instance`.
    ///
    /// Device limits, with the adapter's texture sizes: the downlevel
    /// defaults (storage buffers, compute shaders) when the adapter supports
    /// compute shaders, otherwise the WebGL2 downlevel defaults. The built-in
    /// pipelines only use WebGL2-level features either way; the higher
    /// limits are for native plugins such as the order flow heatmap.
    ///
    /// # Errors
    ///
    /// [`RenderError::Backend`] if the device cannot be created.
    pub async fn from_adapter(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
    ) -> Result<Self, RenderError> {
        let allowed = adapter.limits();
        let compute = wgpu::Limits::downlevel_defaults().using_resolution(allowed.clone());
        let limits = if adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
            && compute.check_limits(&allowed)
        {
            compute
        } else {
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(allowed)
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ferrochart"),
                required_limits: limits,
                ..wgpu::DeviceDescriptor::default()
            })
            .await
            .map_err(|_| RenderError::Backend("cannot create GPU device"))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

/// Adapts a [`TextEngine`] to the core's [`TextMeasurer`].
#[derive(Clone, Copy, Debug)]
pub struct EngineMeasurer<'a>(pub &'a TextEngine);

impl TextMeasurer for EngineMeasurer<'_> {
    fn ascii_advances(&self, style: FontStyleRef) -> &[f32; 128] {
        self.0.ascii_advances(style.font.0)
    }

    fn subscript_advances(&self, style: FontStyleRef) -> Option<&[f32; 10]> {
        Some(self.0.subscript_advances(style.font.0))
    }

    fn measure(&self, text: &str, style: FontStyleRef) -> SizePx {
        SizePx {
            w: self.0.width(style.font.0, text, style.size),
            h: self.line_height(style),
        }
    }

    fn line_height(&self, style: FontStyleRef) -> f32 {
        self.0.line_metrics(style.font.0, style.size).height()
    }
}

/// Integer pixel box `[x0, x1) × [y0, y1)`, rounded like the CPU backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IBox {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl IBox {
    fn of_rect(r: RectPx) -> Self {
        Self {
            x0: r.x.round() as i32,
            y0: r.y.round() as i32,
            x1: (r.x + r.w).round() as i32,
            y1: (r.y + r.h).round() as i32,
        }
    }

    fn intersect(self, o: Self) -> Self {
        Self {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        }
    }

    fn is_empty(self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
}

fn rgba(c: Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

fn rgba_f(c: Rgba) -> [f32; 4] {
    [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0)
}

/// Series pipelines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeriesPipe {
    /// Candlestick and Heikin-Ashi.
    Candles,
    OhlcBars,
    /// Volume, colored by the source bar's direction.
    Histogram,
    /// Histogram of a value series, with optional per-point colors.
    ValueHistogram,
    /// The line of line, area and baseline series.
    Line,
    /// The fill of area and baseline series.
    Fill,
    /// The dots of points series.
    Points,
}

impl SeriesPipe {
    /// Pipelines that draw `kind`, in order (fills under lines, as in the CPU
    /// backend).
    fn passes(kind: SeriesKind) -> &'static [Self] {
        match kind {
            SeriesKind::Candlestick | SeriesKind::HeikinAshi => &[Self::Candles],
            SeriesKind::OhlcBar => &[Self::OhlcBars],
            SeriesKind::Volume => &[Self::Histogram],
            SeriesKind::Histogram => &[Self::ValueHistogram],
            SeriesKind::Line => &[Self::Line],
            SeriesKind::Area | SeriesKind::Baseline => &[Self::Fill, Self::Line],
            SeriesKind::Points => &[Self::Points],
            // Bands have their own pipeline (`Op::Band`). Custom series
            // arrive as primitives or `DrawCmd::Custom`, Kagi and Point &
            // Figure as primitives.
            SeriesKind::Band | SeriesKind::Custom | SeriesKind::Kagi | SeriesKind::PointFigure => {
                &[]
            }
        }
    }
}

/// One recorded draw operation; replayed into a render pass.
#[derive(Clone, Copy, Debug)]
enum Op {
    Scissor(IBox),
    Rects {
        start: u32,
        count: u32,
    },
    Lines {
        start: u32,
        count: u32,
    },
    Glyphs {
        start: u32,
        count: u32,
    },
    /// Glyphs from the color atlas (same instance buffer as `Glyphs`).
    ColorGlyphs {
        start: u32,
        count: u32,
    },
    Icons {
        start: u32,
        count: u32,
    },
    RichIcons {
        start: u32,
        count: u32,
    },
    Triangles {
        start: u32,
        count: u32,
    },
    Series {
        pipe: SeriesPipe,
        series: u32,
        level: u8,
        chunk: i64,
        /// Element offset within the chunk.
        elem: i64,
        count: u32,
        uniform: u32,
        log: bool,
    },
    /// A band between series `upper` and `lower` (same level and chunk).
    Band {
        upper: u32,
        lower: u32,
        level: u8,
        chunk: i64,
        /// Element offset within the chunk.
        elem: i64,
        count: u32,
        uniform: u32,
        log: bool,
    },
    /// `gpu_series[plugin]` draws `custom_draws[draw]`.
    Custom {
        plugin: u32,
        draw: u32,
    },
}

/// Where frames go.
enum Target {
    /// Not yet sized.
    None,
    Offscreen {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        size: (u32, u32),
    },
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        view_format: wgpu::TextureFormat,
    },
}

/// The cached background + main layers.
struct LayerCache {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    size: (u32, u32),
    /// Revisions of the background and main layers it holds.
    revisions: Option<(u64, u64)>,
}

/// A growable GPU buffer.
struct GrowBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl GrowBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let capacity = 4096;
        Self {
            buffer: Self::make(device, label, usage, capacity),
            capacity,
            usage,
            label,
        }
    }

    fn make(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        size: u64,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Writes `bytes`, growing first if needed. Returns whether the buffer was
    /// replaced (bind groups must be recreated).
    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> bool {
        let need = bytes.len() as u64;
        let mut grown = false;
        if need > self.capacity {
            self.capacity = need.next_power_of_two();
            self.buffer = Self::make(device, self.label, self.usage, self.capacity);
            grown = true;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
        grown
    }
}

/// The wgpu renderer.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    target: Target,
    pipelines: Pipelines,
    atlas: Atlas,
    atlas_bind: wgpu::BindGroup,
    color_atlas_bind: wgpu::BindGroup,
    icon_atlas: IconAtlas,
    icon_atlas_bind: wgpu::BindGroup,
    text: TextEngine,
    series: HashMap<u32, SeriesGpu>,
    scratch: Scratch,
    /// Bound as the color buffer of value histograms without per-point
    /// colors (all zero: series color).
    zero_colors: wgpu::Buffer,
    uniform_align: usize,
    uniform_buf: GrowBuffer,
    uniform_bind: wgpu::BindGroup,
    rect_buf: GrowBuffer,
    line_buf: GrowBuffer,
    icon_buf: GrowBuffer,
    rich_icon_buf: GrowBuffer,
    tri_buf: GrowBuffer,
    glyph_buf: GrowBuffer,
    cache: Option<LayerCache>,
    // Per-frame scratch, reused.
    uniforms: Vec<u8>,
    rects: Vec<RectInstance>,
    lines: Vec<LineInstance>,
    icons: Vec<IconInstance>,
    rich_icons: Vec<RichIconInstance>,
    tris: Vec<TriangleInstance>,
    glyphs: Vec<GlyphInstance>,
    main_ops: Vec<Op>,
    overlay_ops: Vec<Op>,
    clips: Vec<IBox>,
    gpu_error: Arc<Mutex<Option<String>>>,
    /// Native custom series and their kinds, in the same order.
    gpu_series: Vec<Box<dyn GpuSeries>>,
    native_kinds: Vec<&'static str>,
    /// The `DrawCmd::Custom`s of this frame (scratch).
    custom_draws: Vec<CustomDraw>,
}

impl std::fmt::Debug for WgpuRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuRenderer")
            .field("format", &self.format)
            .field("series", &self.series.len())
            .finish_non_exhaustive()
    }
}

const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MAX_SIZE: f32 = 16_384.0;

impl WgpuRenderer {
    fn with_format(ctx: &GpuContext, format: wgpu::TextureFormat, target: Target) -> Self {
        let device = ctx.device.clone();
        // wgpu's default handler panics; keep the error and fail the next
        // `render` instead (public API must not panic).
        let gpu_error = Arc::new(Mutex::new(None::<String>));
        let sink = Arc::clone(&gpu_error);
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            if let Ok(mut slot) = sink.lock() {
                slot.get_or_insert_with(|| e.to_string());
            }
        }));
        let pipelines = Pipelines::new(&device, format);
        let atlas = Atlas::new(&device);
        let atlas_bind = Self::texture_bind(&device, &pipelines, &atlas.mono.view);
        let color_atlas_bind = Self::texture_bind(&device, &pipelines, &atlas.color.view);
        let icon_atlas = IconAtlas::new(&device);
        let icon_atlas_bind = Self::texture_bind(&device, &pipelines, &icon_atlas.view);
        let uniform_buf =
            GrowBuffer::new(&device, "ferrochart uniforms", wgpu::BufferUsages::UNIFORM);
        let uniform_bind = Self::uniform_bind(&device, &pipelines, &uniform_buf.buffer);
        let align = device.limits().min_uniform_buffer_offset_alignment.max(256) as usize;
        Self {
            rect_buf: GrowBuffer::new(&device, "ferrochart rects", wgpu::BufferUsages::VERTEX),
            line_buf: GrowBuffer::new(&device, "ferrochart lines", wgpu::BufferUsages::VERTEX),
            icon_buf: GrowBuffer::new(&device, "ferrochart icons", wgpu::BufferUsages::VERTEX),
            rich_icon_buf: GrowBuffer::new(
                &device,
                "ferrochart icons with images",
                wgpu::BufferUsages::VERTEX,
            ),
            tri_buf: GrowBuffer::new(&device, "ferrochart triangles", wgpu::BufferUsages::VERTEX),
            glyph_buf: GrowBuffer::new(&device, "ferrochart glyphs", wgpu::BufferUsages::VERTEX),
            queue: ctx.queue.clone(),
            format,
            target,
            pipelines,
            atlas,
            atlas_bind,
            color_atlas_bind,
            icon_atlas,
            icon_atlas_bind,
            text: TextEngine::new(),
            series: HashMap::new(),
            scratch: Scratch::default(),
            zero_colors: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ferrochart zero colors"),
                size: COLOR_BYTES,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            uniform_align: align,
            uniform_buf,
            uniform_bind,
            cache: None,
            uniforms: Vec::new(),
            rects: Vec::new(),
            lines: Vec::new(),
            icons: Vec::new(),
            rich_icons: Vec::new(),
            tris: Vec::new(),
            glyphs: Vec::new(),
            main_ops: Vec::new(),
            overlay_ops: Vec::new(),
            clips: Vec::new(),
            gpu_error,
            gpu_series: Vec::new(),
            native_kinds: Vec::new(),
            custom_draws: Vec::new(),
            device,
        }
    }

    /// A renderer that draws into its own texture; read it back with
    /// [`read_pixels`](Self::read_pixels). The texture is sized by each frame.
    pub fn offscreen(ctx: &GpuContext) -> Self {
        Self::with_format(ctx, OFFSCREEN_FORMAT, Target::None)
    }

    /// A renderer that presents to `surface` (created from `ctx.instance`).
    ///
    /// # Errors
    ///
    /// [`RenderError::Backend`] if the surface is not supported by the adapter,
    /// [`RenderError::InvalidSize`] for a zero or too large size.
    pub fn with_surface(
        ctx: &GpuContext,
        surface: wgpu::Surface<'static>,
        size: SizePx,
    ) -> Result<Self, RenderError> {
        let (w, h) = target_size(size)?;
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or(RenderError::Backend("surface not supported by the adapter"))?;
        // Colors are blended in sRGB-encoded space like the CPU backend, so
        // render through a non-sRGB view.
        let view_format = format.remove_srgb_suffix();
        let mut config = surface
            .get_default_config(&ctx.adapter, w, h)
            .ok_or(RenderError::Backend("surface not supported by the adapter"))?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        if view_format != format {
            config.view_formats = vec![view_format];
        }
        surface.configure(&ctx.device, &config);
        Ok(Self::with_format(
            ctx,
            view_format,
            Target::Surface {
                surface,
                config,
                view_format,
            },
        ))
    }

    /// The first GPU error (validation, out of memory) reported since the
    /// last call, with wgpu's message. A frame during which such an error
    /// occurred makes [`Renderer::render`] return [`RenderError::Backend`].
    ///
    /// The renderer installs the device's uncaptured-error handler; with
    /// several renderers on one device, the last one created receives them.
    pub fn take_gpu_error(&mut self) -> Option<String> {
        self.gpu_error.lock().ok().and_then(|mut e| e.take())
    }

    fn has_gpu_error(&self) -> bool {
        self.gpu_error.lock().is_ok_and(|e| e.is_some())
    }

    /// Registers the native implementation of a custom series kind,
    /// replacing an earlier one of the same kind. The kind is then in
    /// [`Renderer::native_kinds`]; pass those to `Chart::set_native_kinds`
    /// (the host helpers do it before every frame). Do not register an
    /// implementation that needs compute shaders on WebGL2.
    pub fn register_series(&mut self, series: Box<dyn GpuSeries>) {
        let kind = series.kind();
        if let Some(n) = self.native_kinds.iter().position(|&k| k == kind) {
            self.gpu_series[n] = series;
        } else {
            self.native_kinds.push(kind);
            self.gpu_series.push(series);
        }
    }

    /// The text measurer to pass to `Chart::build_frame`.
    pub fn measurer(&self) -> EngineMeasurer<'_> {
        EngineMeasurer(&self.text)
    }

    /// The text engine (to add fonts).
    pub fn text_engine_mut(&mut self) -> &mut TextEngine {
        &mut self.text
    }

    /// Bytes of GPU vertex buffers held for series data.
    pub fn series_chunks(&self) -> usize {
        self.series.values().map(SeriesGpu::chunk_count).sum()
    }

    fn uniform_bind(
        device: &wgpu::Device,
        p: &Pipelines,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ferrochart uniforms"),
            layout: &p.uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(core::mem::size_of::<DrawUniform>() as u64),
                }),
            }],
        })
    }

    fn texture_bind(
        device: &wgpu::Device,
        p: &Pipelines,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ferrochart texture"),
            layout: &p.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        })
    }

    fn color_texture(
        &self,
        label: &str,
        size: (u32, u32),
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage,
            view_formats: &[],
        })
    }

    fn ensure_cache(&mut self, size: (u32, u32)) {
        if self.cache.as_ref().is_some_and(|c| c.size == size) {
            return;
        }
        let texture = self.color_texture(
            "ferrochart layer cache",
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = Self::texture_bind(&self.device, &self.pipelines, &view);
        self.cache = Some(LayerCache {
            view,
            bind,
            size,
            revisions: None,
        });
    }

    fn ensure_offscreen(&mut self, size: (u32, u32)) {
        if matches!(self.target, Target::Offscreen { size: s, .. } if s == size) {
            return;
        }
        if matches!(self.target, Target::Surface { .. }) {
            return;
        }
        let texture = self.color_texture(
            "ferrochart offscreen target",
            size,
            // Sampled by hosts that compose the chart themselves (egui).
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.target = Target::Offscreen {
            texture,
            view,
            size,
        };
    }

    fn reconfigure(&mut self, size: (u32, u32)) {
        if let Target::Surface {
            surface, config, ..
        } = &mut self.target
            && (config.width, config.height) != size
        {
            config.width = size.0;
            config.height = size.1;
            surface.configure(&self.device, config);
        }
    }

    /// The texture an offscreen renderer drew the last frame into, for hosts
    /// that compose the chart into their own rendering (a UI toolkit's paint
    /// callback, a game engine). `None` for a renderer created with
    /// [`with_surface`](Self::with_surface) and before the first frame.
    ///
    /// The format is `Rgba8Unorm` holding sRGB-encoded colors (the CPU
    /// backend's pixels): sample it as is and write the values to a
    /// non-sRGB target, or convert them to linear before writing to an sRGB
    /// one. The usages are `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING`.
    /// The texture is replaced when the frame size changes, so look it up
    /// after each [`Renderer::render`] rather than keeping it.
    ///
    /// ```no_run
    /// use ferrochart_core::geom::SizeCss;
    /// use ferrochart_core::render::Renderer;
    /// use ferrochart_core::{Chart, ChartOptions};
    /// use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};
    ///
    /// let ctx = GpuContext::headless()?;
    /// let mut renderer = WgpuRenderer::offscreen(&ctx);
    /// let mut chart = Chart::new(ChartOptions::default());
    /// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 2.0);
    /// chart.build_frame(0.0, &renderer.measurer());
    /// renderer.render(chart.frame(), &chart.data_view())?;
    /// let texture = renderer.offscreen_texture().ok_or("no frame yet")?;
    /// assert_eq!((texture.width(), texture.height()), (1600, 900));
    /// // Bind `renderer.offscreen_view()` in the host's own pipeline.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn offscreen_texture(&self) -> Option<&wgpu::Texture> {
        match &self.target {
            Target::Offscreen { texture, .. } => Some(texture),
            _ => None,
        }
    }

    /// A view of the whole [`offscreen_texture`](Self::offscreen_texture)
    /// (same lifetime and conditions), ready to bind for sampling.
    pub fn offscreen_view(&self) -> Option<&wgpu::TextureView> {
        match &self.target {
            Target::Offscreen { view, .. } => Some(view),
            _ => None,
        }
    }

    /// Reads the offscreen target back as tightly packed RGBA8 rows.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidSize`] if this is not an offscreen renderer or
    /// nothing was rendered, [`RenderError::Backend`] if mapping fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_pixels(&self) -> Result<Vec<u8>, RenderError> {
        let Target::Offscreen {
            texture,
            size: (w, h),
            ..
        } = &self.target
        else {
            return Err(RenderError::InvalidSize);
        };
        let (w, h) = (*w, *h);
        let row = w * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ferrochart read-back"),
            size: u64::from(padded) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| RenderError::Backend("GPU poll failed"))?;
        rx.recv()
            .ok()
            .and_then(Result::ok)
            .ok_or(RenderError::Backend("buffer mapping failed"))?;
        let mapped = buffer
            .get_mapped_range(..)
            .map_err(|_| RenderError::Backend("buffer mapping failed"))?;
        let mut out = Vec::with_capacity((row * h) as usize);
        for y in 0..h as usize {
            let start = y * padded as usize;
            out.extend_from_slice(mapped.get(start..start + row as usize).unwrap_or(&[]));
        }
        Ok(out)
    }

    // ---- frame recording -------------------------------------------------

    fn push_uniform(&mut self, u: &DrawUniform) -> u32 {
        let offset = self.uniforms.len();
        self.uniforms.extend_from_slice(bytemuck::bytes_of(u));
        self.uniforms.resize(offset + self.uniform_align, 0);
        u32::try_from(offset).unwrap_or(0)
    }

    fn push_op(ops: &mut Vec<Op>, op: Op) {
        // Extend the previous run of the same primitive when contiguous.
        if let Some(last) = ops.last_mut() {
            match (last, op) {
                (Op::Rects { start, count }, Op::Rects { start: s, count: n })
                | (Op::Lines { start, count }, Op::Lines { start: s, count: n })
                | (Op::Glyphs { start, count }, Op::Glyphs { start: s, count: n })
                | (Op::ColorGlyphs { start, count }, Op::ColorGlyphs { start: s, count: n })
                | (Op::Icons { start, count }, Op::Icons { start: s, count: n })
                | (Op::RichIcons { start, count }, Op::RichIcons { start: s, count: n })
                | (Op::Triangles { start, count }, Op::Triangles { start: s, count: n })
                    if *start + *count == s =>
                {
                    *count += n;
                    return;
                }
                _ => {}
            }
        }
        ops.push(op);
    }

    fn rect(&mut self, ops: &mut Vec<Op>, r: RectPx, color: Rgba) {
        let b = IBox::of_rect(r);
        if b.is_empty() || color.a == 0 {
            return;
        }
        let start = u32::try_from(self.rects.len()).unwrap_or(u32::MAX);
        self.rects.push(RectInstance {
            rect: [
                b.x0 as f32,
                b.y0 as f32,
                (b.x1 - b.x0) as f32,
                (b.y1 - b.y0) as f32,
            ],
            color: rgba(color),
        });
        Self::push_op(ops, Op::Rects { start, count: 1 });
    }

    /// A filled polygon as a triangle fan around its first vertex: exact for
    /// convex polygons (all built-in drawings), like the CPU backend's
    /// non-anti-aliased fill.
    fn polygon(&mut self, ops: &mut Vec<Op>, pts: &[PointPx], fill: Rgba) {
        let [first, rest @ ..] = pts else {
            return;
        };
        if fill.a == 0 || rest.len() < 2 {
            return;
        }
        let start = u32::try_from(self.tris.len()).unwrap_or(u32::MAX);
        let color = rgba(fill);
        for w in rest.windows(2) {
            self.tris.push(TriangleInstance {
                ab: [first.x, first.y, w[0].x, w[0].y],
                c: [w[1].x, w[1].y],
                color,
            });
        }
        let count = u32::try_from(rest.len() - 1).unwrap_or(0);
        Self::push_op(ops, Op::Triangles { start, count });
    }

    /// Mirrors the CPU backend: axis-aligned solid lines become pixel rects,
    /// everything else is an anti-aliased line.
    fn line(&mut self, ops: &mut Vec<Op>, l: LineInst) {
        let width = l.stroke.width.0.max(1.0).round();
        let vertical = (l.from.x - l.to.x).abs() < 1e-3;
        let horizontal = (l.from.y - l.to.y).abs() < 1e-3;
        if l.stroke.dash == LineDash::Solid && (vertical || horizontal) {
            let half = width * 0.5;
            let r = if vertical {
                RectPx {
                    x: l.from.x - half,
                    y: l.from.y.min(l.to.y),
                    w: width,
                    h: (l.to.y - l.from.y).abs().max(1.0),
                }
            } else {
                RectPx {
                    x: l.from.x.min(l.to.x),
                    y: l.from.y - half,
                    w: (l.to.x - l.from.x).abs().max(1.0),
                    h: width,
                }
            };
            self.rect(ops, r, l.stroke.color);
            return;
        }
        let start = u32::try_from(self.lines.len()).unwrap_or(u32::MAX);
        self.lines.push(LineInstance {
            ends: [l.from.x, l.from.y, l.to.x, l.to.y],
            width,
            dash: match l.stroke.dash {
                LineDash::Solid => 0,
                LineDash::Dashed => 1,
                LineDash::Dotted => 2,
            },
            color: rgba(l.stroke.color),
        });
        Self::push_op(ops, Op::Lines { start, count: 1 });
    }

    /// Mirrors `CpuRenderer::draw_text`: same alignment, baseline and pen
    /// rounding, same glyph bitmaps.
    fn text_run(&mut self, ops: &mut Vec<Op>, run: &TextRun, text: &str) {
        let font = run.style.font.0;
        let size = run.style.size;
        let width = self.text.width(font, text, size);
        let m = self.text.line_metrics(font, size);
        let x0 = match run.align {
            TextAlign::Left => run.pos.x,
            TextAlign::Center => run.pos.x - width * 0.5,
            TextAlign::Right => run.pos.x - width,
        }
        .round();
        let baseline = (run.pos.y + (m.ascent - m.descent) * 0.5).round();
        if let Some(bg) = run.bg {
            let pad = bg.padding.0;
            self.rect(
                ops,
                RectPx {
                    x: x0 - pad,
                    y: baseline - m.ascent - pad,
                    w: width + 2.0 * pad,
                    h: m.ascent + m.descent + 2.0 * pad,
                },
                bg.color,
            );
        }
        let mut pen = 0.0_f32;
        for ch in text.chars() {
            let gx = (x0 + pen).round() as i32;
            pen += self.text.advance(font, ch, size);
            let Some(slot) = self.atlas.get(&self.queue, &self.text, font, ch, size) else {
                continue;
            };
            let start = u32::try_from(self.glyphs.len()).unwrap_or(u32::MAX);
            self.glyphs.push(GlyphInstance {
                dst: [
                    (gx + slot.left) as f32,
                    (baseline as i32 + slot.top) as f32,
                    slot.w as f32,
                    slot.h as f32,
                ],
                src: [slot.x as f32, slot.y as f32],
                color: rgba(run.color),
            });
            Self::push_op(
                ops,
                if slot.color {
                    Op::ColorGlyphs { start, count: 1 }
                } else {
                    Op::Glyphs { start, count: 1 }
                },
            );
        }
    }

    fn series_draw(
        &mut self,
        ops: &mut Vec<Op>,
        frame: &Frame,
        data: ChartDataView<'_>,
        s: &SeriesDraw,
        viewport: [f32; 2],
    ) {
        if s.kind == SeriesKind::Band {
            self.band_draw(ops, frame, data, s, viewport);
            return;
        }
        let passes = SeriesPipe::passes(s.kind);
        let Some(view) = data.ohlc(s.series, s.lod_level) else {
            return;
        };
        if s.kind == SeriesKind::Volume && view.volume.is_none() {
            return;
        }
        let Some(style) = frame.styles.get(s.style.0 as usize).copied() else {
            return;
        };
        let span = s.index_range;
        if span.is_empty() {
            return;
        }
        let top = self.clips.last().copied().unwrap_or(IBox {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
        });
        let clip = top.intersect(IBox::of_rect(s.clip));
        ops.push(Op::Scissor(clip));
        let yt = s.y_transform;
        // Volumes are drawn from their own buffer; the OHLC buffer only gives
        // the direction, which the linear values decide.
        let log = s.kind != SeriesKind::Volume && yt.mapping == PriceMapping::Log;
        let m = bar_metrics(s.x_transform.step);
        let dash = match style.line_dash {
            LineDash::Solid => 0,
            LineDash::Dashed => 1,
            LineDash::Dotted => 2,
        };
        let flags = u32::from(m.show_body)
            | (u32::from(m.show_border) << 1)
            | (u32::from(style.step) << 2)
            | (u32::from(s.kind == SeriesKind::Baseline) << 3)
            | (u32::from(s.kind == SeriesKind::Area) << 4)
            | (dash << 5);
        let level = view.level;
        let colors = (s.kind == SeriesKind::Histogram && level == 0)
            .then(|| data.point_colors(s.series))
            .flatten();
        let line_base = y_of_price(&yt, style.base_value);
        let clip_y = [s.clip.y, s.clip.y + s.clip.h];
        let id = s.series.0;
        for &pipe in passes {
            for c in chunk_of(level, span.start)..=chunk_of(level, span.end - 1) {
                let cs = chunk_start(level, c);
                let e0 = span.start.max(cs);
                let e1 = span.end.min(cs + CHUNK_LEN);
                if e0 >= e1 {
                    continue;
                }
                let gpu = self.series.entry(id).or_default();
                let Some(chunk) = gpu.prepare(
                    &self.device,
                    &self.queue,
                    &mut self.scratch,
                    &view,
                    colors,
                    c,
                    log,
                ) else {
                    continue;
                };
                let (bias, base_y) = match pipe {
                    SeriesPipe::Candles | SeriesPipe::OhlcBars => {
                        (y_bias(&yt, chunk.reference), 0.0)
                    }
                    SeriesPipe::Histogram => (y_bias(&yt, 0.0), y_of_price(&yt, 0.0)),
                    SeriesPipe::Fill if s.kind == SeriesKind::Area => {
                        (y_bias(&yt, chunk.reference), clip_y[1])
                    }
                    SeriesPipe::ValueHistogram
                    | SeriesPipe::Line
                    | SeriesPipe::Fill
                    | SeriesPipe::Points => (y_bias(&yt, chunk.reference), line_base),
                };
                let SeriesStyle {
                    up,
                    down,
                    up_wick,
                    down_wick,
                    up_border,
                    down_border,
                    fill_top,
                    fill_bottom,
                    line_width,
                    ..
                } = style;
                let (body, colors) = match pipe {
                    SeriesPipe::Line | SeriesPipe::Fill => (
                        line_width.0 * 0.5,
                        [up, down, fill_top, fill_bottom, up_border, down_border],
                    ),
                    // The dot's diameter.
                    SeriesPipe::Points => (
                        line_width.0,
                        [up, down, fill_top, fill_bottom, up_border, down_border],
                    ),
                    _ => (
                        m.body,
                        [up, down, up_wick, down_wick, up_border, down_border],
                    ),
                };
                let uniform = self.push_uniform(&DrawUniform {
                    viewport,
                    x_step: s.x_transform.step,
                    x_shift: x_shift(&s.x_transform),
                    a: yt.a,
                    bias,
                    index_offset: i32::try_from(e0 - s.x_transform.base).unwrap_or(0),
                    body,
                    flags,
                    base_y,
                    clip_y,
                    colors: colors.map(rgba_f),
                });
                ops.push(Op::Series {
                    pipe,
                    series: id,
                    level,
                    chunk: c,
                    elem: e0 - cs,
                    count: u32::try_from(e1 - e0).unwrap_or(0),
                    uniform,
                    log,
                });
            }
        }
        let top = self.clips.last().copied().unwrap_or(clip);
        ops.push(Op::Scissor(top));
    }

    /// A band: the upper and lower series' chunks drawn together,
    /// each with its own reference price (`bias` and `base_y` of the uniform).
    fn band_draw(
        &mut self,
        ops: &mut Vec<Op>,
        frame: &Frame,
        data: ChartDataView<'_>,
        s: &SeriesDraw,
        viewport: [f32; 2],
    ) {
        let Some(pair) = s.pair else { return };
        let (Some(upper), Some(lower)) = (
            data.ohlc(s.series, s.lod_level),
            data.ohlc(pair, s.lod_level),
        ) else {
            return;
        };
        let Some(style) = frame.styles.get(s.style.0 as usize).copied() else {
            return;
        };
        let span = s.index_range;
        if span.is_empty() {
            return;
        }
        let top = self.clips.last().copied().unwrap_or(IBox {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
        });
        let clip = top.intersect(IBox::of_rect(s.clip));
        ops.push(Op::Scissor(clip));
        let yt = s.y_transform;
        let log = yt.mapping == PriceMapping::Log;
        let level = upper.level;
        let clip_y = [s.clip.y, s.clip.y + s.clip.h];
        let (id, pair) = (s.series.0, pair.0);
        for c in chunk_of(level, span.start)..=chunk_of(level, span.end - 1) {
            let cs = chunk_start(level, c);
            let e0 = span.start.max(cs);
            let e1 = span.end.min(cs + CHUNK_LEN);
            if e0 >= e1 {
                continue;
            }
            let mut reference = |series: u32, view: &OhlcLevelView<'_>| {
                self.series
                    .entry(series)
                    .or_default()
                    .prepare(
                        &self.device,
                        &self.queue,
                        &mut self.scratch,
                        view,
                        None,
                        c,
                        log,
                    )
                    .map(|chunk| chunk.reference)
            };
            let (Some(ru), Some(rl)) = (reference(id, &upper), reference(pair, &lower)) else {
                continue;
            };
            let st = style;
            let colors = [
                st.up,
                st.down,
                st.fill_top,
                st.fill_bottom,
                st.up_border,
                st.down_border,
            ];
            let uniform = self.push_uniform(&DrawUniform {
                viewport,
                x_step: s.x_transform.step,
                x_shift: x_shift(&s.x_transform),
                a: yt.a,
                bias: y_bias(&yt, ru),
                index_offset: i32::try_from(e0 - s.x_transform.base).unwrap_or(0),
                body: 0.0,
                flags: 0,
                // The lower series' bias (`y_lower` in the shader).
                base_y: y_bias(&yt, rl),
                clip_y,
                colors: colors.map(rgba_f),
            });
            ops.push(Op::Band {
                upper: id,
                lower: pair,
                level,
                chunk: c,
                elem: e0 - cs,
                count: u32::try_from(e1 - e0).unwrap_or(0),
                uniform,
                log,
            });
        }
        let top = self.clips.last().copied().unwrap_or(clip);
        ops.push(Op::Scissor(top));
    }

    fn record_layer(
        &mut self,
        ops: &mut Vec<Op>,
        frame: &Frame,
        layer: &Layer,
        data: ChartDataView<'_>,
        full: IBox,
        viewport: [f32; 2],
    ) {
        self.clips.clear();
        self.clips.push(full);
        ops.push(Op::Scissor(full));
        let pools = &frame.pools;
        for cmd in &layer.cmds {
            match *cmd {
                DrawCmd::PushClip(r) => {
                    let top = self.clips.last().copied().unwrap_or(full);
                    let b = top.intersect(IBox::of_rect(r));
                    self.clips.push(b);
                    ops.push(Op::Scissor(b));
                }
                DrawCmd::PopClip if self.clips.len() > 1 => {
                    self.clips.pop();
                    ops.push(Op::Scissor(self.clips.last().copied().unwrap_or(full)));
                }
                DrawCmd::Rects(span) => {
                    for r in pools.rects.get(span.range()).unwrap_or(&[]) {
                        self.rect(ops, r.rect, r.color);
                    }
                }
                DrawCmd::Lines(span) => {
                    for l in pools.lines.get(span.range()).unwrap_or(&[]) {
                        self.line(ops, *l);
                    }
                }
                DrawCmd::Polyline { points, style } => {
                    let pts = pools.points.get(points.range()).unwrap_or(&[]);
                    for w in pts.windows(2) {
                        self.line(
                            ops,
                            LineInst {
                                from: w[0],
                                to: w[1],
                                stroke: style,
                            },
                        );
                    }
                }
                DrawCmd::Text(span) => {
                    for run in pools.text_runs.get(span.range()).unwrap_or(&[]) {
                        let text = pools.text.get(run.text.range()).unwrap_or("");
                        self.text_run(ops, run, text);
                    }
                }
                DrawCmd::Series(s) => self.series_draw(ops, frame, data, &s, viewport),
                DrawCmd::Icon(span) => {
                    for icon in pools.icons.get(span.range()).unwrap_or(&[]) {
                        let tile = icon.image.and_then(|id| self.icon_atlas.tile_of(id));
                        let bordered = icon.border.a > 0 && icon.border_width.0 > 0.0;
                        if (tile.is_some() || bordered) && icon.size.0 > 0.0 {
                            let start = u32::try_from(self.rich_icons.len()).unwrap_or(u32::MAX);
                            self.rich_icons.push(RichIconInstance {
                                pos: [icon.pos.x, icon.pos.y],
                                size: icon.size.0,
                                icon: u32::from(icon.icon),
                                color: rgba(icon.color),
                                border: rgba(icon.border),
                                border_width: icon.border_width.0,
                                tile: tile.unwrap_or([u32::MAX; 2]),
                            });
                            Self::push_op(ops, Op::RichIcons { start, count: 1 });
                            continue;
                        }
                        if icon.color.a == 0 || icon.size.0 <= 0.0 {
                            continue;
                        }
                        let start = u32::try_from(self.icons.len()).unwrap_or(u32::MAX);
                        self.icons.push(IconInstance {
                            pos: [icon.pos.x, icon.pos.y],
                            size: icon.size.0,
                            icon: u32::from(icon.icon),
                            color: rgba(icon.color),
                        });
                        Self::push_op(ops, Op::Icons { start, count: 1 });
                    }
                }
                DrawCmd::Polygon { points, fill } => {
                    let pts = pools.points.get(points.range()).unwrap_or(&[]);
                    self.polygon(ops, pts, fill);
                }
                DrawCmd::Custom(c) => {
                    let Some(n) = self.native_kinds.iter().position(|&k| k == c.kind) else {
                        continue;
                    };
                    let ctx = GpuSeriesCtx {
                        device: &self.device,
                        queue: &self.queue,
                        format: self.format,
                        viewport,
                        frame,
                    };
                    self.gpu_series[n].prepare(&ctx, &c, &data);
                    let draw = u32::try_from(self.custom_draws.len()).unwrap_or(u32::MAX);
                    self.custom_draws.push(c);
                    ops.push(Op::Custom {
                        plugin: u32::try_from(n).unwrap_or(u32::MAX),
                        draw,
                    });
                }
                // Unbalanced pops are ignored.
                DrawCmd::PopClip => {}
            }
        }
    }

    fn replay(&self, pass: &mut wgpu::RenderPass<'_>, ops: &[Op], size: (u32, u32)) {
        let mut skip = false;
        for op in ops {
            match *op {
                Op::Scissor(b) => {
                    let b = b.intersect(IBox {
                        x0: 0,
                        y0: 0,
                        x1: size.0 as i32,
                        y1: size.1 as i32,
                    });
                    skip = b.is_empty();
                    if !skip {
                        pass.set_scissor_rect(
                            b.x0 as u32,
                            b.y0 as u32,
                            (b.x1 - b.x0) as u32,
                            (b.y1 - b.y0) as u32,
                        );
                    }
                }
                _ if skip => {}
                Op::Custom { plugin, draw } => {
                    if let (Some(p), Some(d)) = (
                        self.gpu_series.get(plugin as usize),
                        self.custom_draws.get(draw as usize),
                    ) {
                        p.draw(pass, d);
                    }
                }
                Op::Rects { start, count } => {
                    pass.set_pipeline(&self.pipelines.rects);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    let stride = core::mem::size_of::<RectInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.rect_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..6, 0..count);
                }
                Op::Lines { start, count } => {
                    pass.set_pipeline(&self.pipelines.lines);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    let stride = core::mem::size_of::<LineInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.line_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..6, 0..count);
                }
                Op::Icons { start, count } => {
                    pass.set_pipeline(&self.pipelines.icons);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    let stride = core::mem::size_of::<IconInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.icon_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..6, 0..count);
                }
                Op::RichIcons { start, count } => {
                    pass.set_pipeline(&self.pipelines.icons_rich);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    pass.set_bind_group(1, &self.icon_atlas_bind, &[]);
                    let stride = core::mem::size_of::<RichIconInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.rich_icon_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..6, 0..count);
                }
                Op::Triangles { start, count } => {
                    pass.set_pipeline(&self.pipelines.triangles);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    let stride = core::mem::size_of::<TriangleInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.tri_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..3, 0..count);
                }
                Op::Glyphs { start, count } | Op::ColorGlyphs { start, count } => {
                    let (pipeline, bind) = if let Op::ColorGlyphs { .. } = op {
                        (&self.pipelines.glyphs_color, &self.color_atlas_bind)
                    } else {
                        (&self.pipelines.glyphs, &self.atlas_bind)
                    };
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &self.uniform_bind, &[0]);
                    pass.set_bind_group(1, bind, &[]);
                    let stride = core::mem::size_of::<GlyphInstance>() as u64;
                    pass.set_vertex_buffer(
                        0,
                        self.glyph_buf.buffer.slice(u64::from(start) * stride..),
                    );
                    pass.draw(0..6, 0..count);
                }
                Op::Band {
                    upper,
                    lower,
                    level,
                    chunk,
                    elem,
                    count,
                    uniform,
                    log,
                } => {
                    let buffers = |series: u32| {
                        let c = self.series.get(&series)?.chunk(level, chunk)?;
                        if log { c.ln.as_ref() } else { Some(&c.ohlc) }
                    };
                    let (Some(u), Some(l)) = (buffers(upper), buffers(lower)) else {
                        continue;
                    };
                    pass.set_bind_group(0, &self.uniform_bind, &[uniform]);
                    pass.set_pipeline(&self.pipelines.band);
                    // Elements j and j + 1 of both series.
                    pass.set_vertex_buffer(0, u.slice(ohlc_offset(0, elem)..));
                    pass.set_vertex_buffer(1, u.slice(ohlc_offset(0, elem + 1)..));
                    pass.set_vertex_buffer(2, l.slice(ohlc_offset(0, elem)..));
                    pass.set_vertex_buffer(3, l.slice(ohlc_offset(0, elem + 1)..));
                    pass.draw(0..6, 0..count);
                }
                Op::Series {
                    pipe,
                    series,
                    level,
                    chunk,
                    elem,
                    count,
                    uniform,
                    log,
                } => {
                    let Some(c) = self.series.get(&series).and_then(|s| s.chunk(level, chunk))
                    else {
                        continue;
                    };
                    let prices = if log { c.ln.as_ref() } else { Some(&c.ohlc) };
                    let Some(prices) = prices else {
                        continue;
                    };
                    pass.set_bind_group(0, &self.uniform_bind, &[uniform]);
                    pass.set_vertex_buffer(0, prices.slice(ohlc_offset(0, elem)..));
                    match pipe {
                        SeriesPipe::Candles => {
                            pass.set_pipeline(&self.pipelines.candles);
                            pass.draw(0..12, 0..count);
                        }
                        SeriesPipe::OhlcBars => {
                            pass.set_pipeline(&self.pipelines.ohlc_bars);
                            pass.draw(0..18, 0..count);
                        }
                        SeriesPipe::ValueHistogram => {
                            let colors = c.colors.as_ref().unwrap_or(&self.zero_colors);
                            pass.set_pipeline(&self.pipelines.value_histogram);
                            pass.set_vertex_buffer(1, colors.slice(value_offset(0, elem)..));
                            pass.draw(0..6, 0..count);
                        }
                        SeriesPipe::Points => {
                            pass.set_pipeline(&self.pipelines.points);
                            pass.draw(0..6, 0..count);
                        }
                        SeriesPipe::Line | SeriesPipe::Fill => {
                            // Element j + 1 (the junction element for the last).
                            pass.set_vertex_buffer(1, prices.slice(ohlc_offset(0, elem + 1)..));
                            if pipe == SeriesPipe::Line {
                                pass.set_pipeline(&self.pipelines.line_series);
                                pass.draw(0..18, 0..count);
                            } else {
                                pass.set_pipeline(&self.pipelines.fill_series);
                                pass.draw(0..6, 0..count);
                            }
                        }
                        SeriesPipe::Histogram => {
                            let Some(vol) = &c.volume else {
                                continue;
                            };
                            pass.set_pipeline(&self.pipelines.histogram);
                            pass.set_vertex_buffer(1, vol.slice(value_offset(0, elem)..));
                            pass.draw(0..6, 0..count);
                        }
                    }
                }
            }
        }
    }

    /// Draws `frame` into `target`.
    fn render_into(
        &mut self,
        frame: &Frame,
        data: ChartDataView<'_>,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        // Pull data changes for every series drawn in this frame.
        for layer in &frame.layers {
            for cmd in &layer.cmds {
                if let DrawCmd::Series(s) = cmd {
                    for id in core::iter::once(s.series).chain(s.pair) {
                        self.series.entry(id.0).or_default().sync(data, id);
                    }
                }
            }
        }
        if self
            .icon_atlas
            .sync(&self.device, &self.queue, data.icons())
        {
            self.icon_atlas_bind =
                Self::texture_bind(&self.device, &self.pipelines, &self.icon_atlas.view);
        }
        self.ensure_cache(size);
        let revisions = (frame.layers[0].revision, frame.layers[1].revision);
        let cache_valid = self
            .cache
            .as_ref()
            .is_some_and(|c| c.revisions == Some(revisions));

        self.uniforms.clear();
        self.custom_draws.clear();
        self.rects.clear();
        self.lines.clear();
        self.icons.clear();
        self.rich_icons.clear();
        self.tris.clear();
        self.glyphs.clear();
        let viewport = [size.0 as f32, size.1 as f32];
        self.push_uniform(&DrawUniform {
            viewport,
            ..DrawUniform::default()
        });
        let full = IBox {
            x0: 0,
            y0: 0,
            x1: size.0 as i32,
            y1: size.1 as i32,
        };
        let mut main_ops = std::mem::take(&mut self.main_ops);
        let mut overlay_ops = std::mem::take(&mut self.overlay_ops);
        main_ops.clear();
        overlay_ops.clear();
        if !cache_valid {
            for layer in &frame.layers[..2] {
                self.record_layer(&mut main_ops, frame, layer, data, full, viewport);
            }
        }
        self.record_layer(
            &mut overlay_ops,
            frame,
            &frame.layers[2],
            data,
            full,
            viewport,
        );

        if self
            .uniform_buf
            .write(&self.device, &self.queue, &self.uniforms)
        {
            self.uniform_bind =
                Self::uniform_bind(&self.device, &self.pipelines, &self.uniform_buf.buffer);
        }
        self.rect_buf
            .write(&self.device, &self.queue, bytemuck::cast_slice(&self.rects));
        self.line_buf
            .write(&self.device, &self.queue, bytemuck::cast_slice(&self.lines));
        self.icon_buf
            .write(&self.device, &self.queue, bytemuck::cast_slice(&self.icons));
        self.rich_icon_buf.write(
            &self.device,
            &self.queue,
            bytemuck::cast_slice(&self.rich_icons),
        );
        self.tri_buf
            .write(&self.device, &self.queue, bytemuck::cast_slice(&self.tris));
        self.glyph_buf.write(
            &self.device,
            &self.queue,
            bytemuck::cast_slice(&self.glyphs),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ferrochart frame"),
            });
        if let Some(cache) = &self.cache {
            if !cache_valid {
                let c = frame.clear_color;
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ferrochart main layers"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &cache.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(c.r) / 255.0,
                                g: f64::from(c.g) / 255.0,
                                b: f64::from(c.b) / 255.0,
                                a: f64::from(c.a) / 255.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                self.replay(&mut pass, &main_ops, size);
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ferrochart composite + overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.composite);
            pass.set_bind_group(0, &self.uniform_bind, &[0]);
            pass.set_bind_group(1, &cache.bind, &[]);
            pass.draw(0..3, 0..1);
            self.replay(&mut pass, &overlay_ops, size);
        }
        self.queue.submit([encoder.finish()]);
        if let Some(cache) = &mut self.cache {
            cache.revisions = Some(revisions);
        }
        self.main_ops = main_ops;
        self.overlay_ops = overlay_ops;
    }
}

impl WgpuRenderer {
    fn result(&self) -> Result<(), RenderError> {
        if self.has_gpu_error() {
            return Err(RenderError::Backend("GPU error (see take_gpu_error)"));
        }
        Ok(())
    }
}

fn target_size(size: SizePx) -> Result<(u32, u32), RenderError> {
    let (w, h) = (size.w.round(), size.h.round());
    if !(w >= 1.0 && h >= 1.0 && w <= MAX_SIZE && h <= MAX_SIZE) {
        return Err(RenderError::InvalidSize);
    }
    Ok((w as u32, h as u32))
}

impl Renderer for WgpuRenderer {
    fn render(&mut self, frame: &Frame, data: &ChartDataView<'_>) -> Result<(), RenderError> {
        let size = target_size(frame.size_px)?;
        if let Target::Surface { .. } = self.target {
            self.reconfigure(size);
            let Target::Surface {
                surface,
                view_format,
                ..
            } = &self.target
            else {
                return Err(RenderError::InvalidSize);
            };
            let texture = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t)
                | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    if let Target::Surface {
                        surface, config, ..
                    } = &self.target
                    {
                        surface.configure(&self.device, config);
                    }
                    return Err(RenderError::Backend("surface outdated; reconfigured"));
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Err(RenderError::Backend("surface not available"));
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::Backend("surface validation error"));
                }
            };
            let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(*view_format),
                ..wgpu::TextureViewDescriptor::default()
            });
            self.render_into(frame, *data, &view, size);
            self.queue.present(texture);
            return self.result();
        }
        self.ensure_offscreen(size);
        let Target::Offscreen { view, .. } = &self.target else {
            return Err(RenderError::InvalidSize);
        };
        let view = view.clone();
        self.render_into(frame, *data, &view, size);
        self.result()
    }

    fn resize(&mut self, size: SizePx) {
        if let Ok(size) = target_size(size) {
            match self.target {
                Target::Surface { .. } => self.reconfigure(size),
                _ => self.ensure_offscreen(size),
            }
        }
    }

    fn native_kinds(&self) -> &[&'static str] {
        &self.native_kinds
    }
}
