//! The offscreen target as a texture for host composition (egui paint callback):
//! available after a frame, sized by it,
//! sampleable, and holding what `read_pixels` returns.
//!
//! Needs a GPU adapter; without one the test prints a notice and passes.
#![allow(clippy::unwrap_used)]

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::geom::SizeCss;
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

#[test]
fn offscreen_texture_follows_the_frame() {
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no GPU adapter ({e}); offscreen texture test skipped");
            return;
        }
    };
    let mut renderer = WgpuRenderer::offscreen(&ctx);
    assert!(renderer.offscreen_texture().is_none());
    assert!(renderer.offscreen_view().is_none());

    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 320.0, h: 200.0 }, 2.0);
    let id = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart
        .set_data(
            id,
            SeriesData::Ohlc(random_walk(3, 300, 0, 60_000).unwrap()),
        )
        .unwrap();
    chart.build_frame(0.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();

    let texture = renderer.offscreen_texture().unwrap();
    assert_eq!((texture.width(), texture.height()), (640, 400));
    assert_eq!(texture.format(), wgpu::TextureFormat::Rgba8Unorm);
    assert!(
        texture
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
    );
    assert!(renderer.offscreen_view().is_some());

    // Sampling the view through a bind group is valid (no GPU error).
    let layout = ctx
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
    let _bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(renderer.offscreen_view().unwrap()),
        }],
    });
    assert_eq!(renderer.take_gpu_error(), None);
    assert_eq!(renderer.read_pixels().unwrap().len(), 640 * 400 * 4);

    // A new size replaces the texture.
    chart.resize(SizeCss { w: 300.0, h: 200.0 }, 1.0);
    chart.build_frame(16.0, &renderer.measurer());
    renderer.render(chart.frame(), &chart.data_view()).unwrap();
    let texture = renderer.offscreen_texture().unwrap();
    assert_eq!((texture.width(), texture.height()), (300, 200));
}
