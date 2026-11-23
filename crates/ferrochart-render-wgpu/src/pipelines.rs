//! Render pipelines, created once per target format.

use bytemuck::{Pod, Zeroable};

/// Per-draw uniform block. Layout matches `Draw` in `series.wgsl`; the first
/// field is also `Globals` in `prims.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct DrawUniform {
    pub viewport: [f32; 2],
    pub x_step: f32,
    pub x_shift: f32,
    pub a: f32,
    pub bias: f32,
    pub index_offset: i32,
    pub body: f32,
    pub flags: u32,
    pub base_y: f32,
    /// Top and bottom of the series clip (area gradient).
    pub clip_y: [f32; 2],
    pub colors: [[f32; 4]; 6],
}

/// A filled rectangle instance (whole pixels).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct RectInstance {
    pub rect: [f32; 4],
    pub color: [u8; 4],
}

/// An anti-aliased line instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct LineInstance {
    pub ends: [f32; 4],
    pub width: f32,
    pub dash: u32,
    pub color: [u8; 4],
}

/// A filled triangle (polygons are drawn as fans of them), not
/// anti-aliased like the CPU backend's polygon fills.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct TriangleInstance {
    pub ab: [f32; 4],
    pub c: [f32; 2],
    pub color: [u8; 4],
}

/// A built-in icon instance (center, size, icon number).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct IconInstance {
    pub pos: [f32; 2],
    pub size: f32,
    pub icon: u32,
    pub color: [u8; 4],
}

/// An icon with an image or a border (r21): `tile` is the texel origin of
/// its image in the icon atlas, `u32::MAX` for none.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct RichIconInstance {
    pub pos: [f32; 2],
    pub size: f32,
    pub icon: u32,
    pub color: [u8; 4],
    pub border: [u8; 4],
    pub border_width: f32,
    pub tile: [u32; 2],
}

/// A glyph quad instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GlyphInstance {
    pub dst: [f32; 4],
    pub src: [f32; 2],
    pub color: [u8; 4],
}

pub(crate) struct Pipelines {
    pub uniform_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub candles: wgpu::RenderPipeline,
    pub histogram: wgpu::RenderPipeline,
    pub ohlc_bars: wgpu::RenderPipeline,
    pub value_histogram: wgpu::RenderPipeline,
    pub line_series: wgpu::RenderPipeline,
    pub fill_series: wgpu::RenderPipeline,
    pub points: wgpu::RenderPipeline,
    pub band: wgpu::RenderPipeline,
    pub rects: wgpu::RenderPipeline,
    pub lines: wgpu::RenderPipeline,
    pub glyphs: wgpu::RenderPipeline,
    pub glyphs_color: wgpu::RenderPipeline,
    pub icons: wgpu::RenderPipeline,
    pub icons_rich: wgpu::RenderPipeline,
    pub triangles: wgpu::RenderPipeline,
    pub composite: wgpu::RenderPipeline,
}

const OHLC_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x4];
const VALUE_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![1 => Float32];
const COLOR_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![1 => Unorm8x4];
const NEXT_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![1 => Float32x4];
/// A band's lower series: elements j and j + 1.
const LOWER_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![2 => Float32x4];
const LOWER_NEXT_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![3 => Float32x4];
const RECT_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Unorm8x4];
const LINE_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32, 2 => Uint32, 3 => Unorm8x4];
const ICON_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32, 2 => Uint32, 3 => Unorm8x4];
const RICH_ICON_ATTRS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32, 2 => Uint32, 3 => Unorm8x4, 4 => Unorm8x4, 5 => Float32, 6 => Uint32x2
];
const TRIANGLE_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2, 2 => Unorm8x4];
const GLYPH_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2, 2 => Unorm8x4];

fn instances(stride: u64, attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
    wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes,
    }
}

impl Pipelines {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let series = device.create_shader_module(wgpu::include_wgsl!("shaders/series.wgsl"));
        let prims = device.create_shader_module(wgpu::include_wgsl!("shaders/prims.wgsl"));
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ferrochart uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        core::mem::size_of::<DrawUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ferrochart texture"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let plain = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ferrochart plain"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let textured = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ferrochart textured"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let blend = Some(wgpu::BlendState::ALPHA_BLENDING);
        let make = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    module: &wgpu::ShaderModule,
                    vs: &str,
                    fs: &str,
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                    blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            candles: make(
                "ferrochart candles",
                &plain,
                &series,
                "vs_candle",
                "fs_series",
                &[Some(instances(16, &OHLC_ATTRS))],
                blend,
            ),
            histogram: make(
                "ferrochart histogram",
                &plain,
                &series,
                "vs_histogram",
                "fs_series",
                &[
                    Some(instances(16, &OHLC_ATTRS)),
                    Some(instances(4, &VALUE_ATTRS)),
                ],
                blend,
            ),
            ohlc_bars: make(
                "ferrochart ohlc bars",
                &plain,
                &series,
                "vs_ohlc_bar",
                "fs_series",
                &[Some(instances(16, &OHLC_ATTRS))],
                blend,
            ),
            value_histogram: make(
                "ferrochart value histogram",
                &plain,
                &series,
                "vs_value_histogram",
                "fs_series",
                &[
                    Some(instances(16, &OHLC_ATTRS)),
                    Some(instances(4, &COLOR_ATTRS)),
                ],
                blend,
            ),
            line_series: make(
                "ferrochart line series",
                &plain,
                &series,
                "vs_line_series",
                "fs_line_series",
                &[
                    Some(instances(16, &OHLC_ATTRS)),
                    Some(instances(16, &NEXT_ATTRS)),
                ],
                blend,
            ),
            fill_series: make(
                "ferrochart fill series",
                &plain,
                &series,
                "vs_fill_series",
                "fs_fill_series",
                &[
                    Some(instances(16, &OHLC_ATTRS)),
                    Some(instances(16, &NEXT_ATTRS)),
                ],
                blend,
            ),
            points: make(
                "ferrochart points",
                &plain,
                &series,
                "vs_points",
                "fs_points",
                &[Some(instances(16, &OHLC_ATTRS))],
                blend,
            ),
            band: make(
                "ferrochart band",
                &plain,
                &series,
                "vs_band",
                "fs_band",
                &[
                    Some(instances(16, &OHLC_ATTRS)),
                    Some(instances(16, &NEXT_ATTRS)),
                    Some(instances(16, &LOWER_ATTRS)),
                    Some(instances(16, &LOWER_NEXT_ATTRS)),
                ],
                blend,
            ),
            rects: make(
                "ferrochart rects",
                &plain,
                &prims,
                "vs_rect",
                "fs_rect",
                &[Some(instances(
                    core::mem::size_of::<RectInstance>() as u64,
                    &RECT_ATTRS,
                ))],
                blend,
            ),
            lines: make(
                "ferrochart lines",
                &plain,
                &prims,
                "vs_line",
                "fs_line",
                &[Some(instances(
                    core::mem::size_of::<LineInstance>() as u64,
                    &LINE_ATTRS,
                ))],
                blend,
            ),
            icons: make(
                "ferrochart icons",
                &plain,
                &prims,
                "vs_icon",
                "fs_icon",
                &[Some(instances(
                    core::mem::size_of::<IconInstance>() as u64,
                    &ICON_ATTRS,
                ))],
                blend,
            ),
            icons_rich: make(
                "ferrochart icons with images",
                &textured,
                &prims,
                "vs_icon_rich",
                "fs_icon_rich",
                &[Some(instances(
                    core::mem::size_of::<RichIconInstance>() as u64,
                    &RICH_ICON_ATTRS,
                ))],
                blend,
            ),
            triangles: make(
                "ferrochart triangles",
                &plain,
                &prims,
                "vs_triangle",
                "fs_rect",
                &[Some(instances(
                    core::mem::size_of::<TriangleInstance>() as u64,
                    &TRIANGLE_ATTRS,
                ))],
                blend,
            ),
            glyphs: make(
                "ferrochart text",
                &textured,
                &prims,
                "vs_glyph",
                "fs_glyph",
                &[Some(instances(
                    core::mem::size_of::<GlyphInstance>() as u64,
                    &GLYPH_ATTRS,
                ))],
                blend,
            ),
            glyphs_color: make(
                "ferrochart color text",
                &textured,
                &prims,
                "vs_glyph",
                "fs_glyph_color",
                &[Some(instances(
                    core::mem::size_of::<GlyphInstance>() as u64,
                    &GLYPH_ATTRS,
                ))],
                blend,
            ),
            composite: make(
                "ferrochart composite",
                &textured,
                &prims,
                "vs_full",
                "fs_composite",
                &[],
                None,
            ),
            uniform_layout,
            texture_layout,
        }
    }
}
