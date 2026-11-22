//! `to_png` of charts and plots: offscreen rendering, wgpu first, CPU if no
//! GPU adapter.
//!
//! The headless GPU device is created once per process. A chart or plot keeps
//! its CPU renderer; the wgpu renderer is made for each image, because it is
//! not `Send` (registered `GpuSeries` need not be) and the chart is shared
//! between threads.

use std::sync::OnceLock;

use ferrochart_core::render::Renderer;
use ferrochart_render_cpu::CpuRenderer;
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

use crate::shared::Draw;

/// Which renderer `to_png` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    /// wgpu, CPU if there is no GPU adapter.
    Auto,
    /// wgpu only.
    Gpu,
    /// tiny-skia only.
    Cpu,
}

impl Backend {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" => Self::Auto,
            "gpu" => Self::Gpu,
            "cpu" => Self::Cpu,
            _ => return None,
        })
    }
}

/// The process' headless GPU device; the error if there is none.
fn gpu() -> Result<&'static GpuContext, String> {
    static CTX: OnceLock<Result<GpuContext, String>> = OnceLock::new();
    CTX.get_or_init(|| GpuContext::headless().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(Clone::clone)
}

/// The CPU renderer of one chart or plot, created on first use.
#[derive(Default)]
pub(crate) struct Renderers {
    cpu: Option<CpuRenderer>,
}

/// An image: PNG bytes and the backend that drew it (`"gpu"` or `"cpu"`).
pub(crate) type Image = (Vec<u8>, &'static str);

impl Renderers {
    /// Builds a frame of `chart` (a chart or plot, already sized) and
    /// encodes it as PNG.
    pub(crate) fn render<D: Draw>(
        &mut self,
        chart: &mut D,
        backend: Backend,
    ) -> Result<Image, String> {
        if backend != Backend::Cpu {
            match gpu() {
                Ok(ctx) => {
                    let mut r = WgpuRenderer::offscreen(ctx);
                    return gpu_png(&mut r, chart).map(|png| (png, "gpu"));
                }
                Err(e) if backend == Backend::Gpu => return Err(format!("no GPU: {e}")),
                Err(_) => {}
            }
        }
        let r = self.cpu.get_or_insert_with(CpuRenderer::new);
        chart.build_frame(0.0, &r.measurer());
        r.render(chart.frame(), &chart.data_view())
            .map_err(|e| e.to_string())?;
        let png = r.encode_png().map_err(|e| e.to_string())?;
        Ok((png, "cpu"))
    }
}

fn gpu_png<D: Draw>(r: &mut WgpuRenderer, chart: &mut D) -> Result<Vec<u8>, String> {
    chart.build_frame(0.0, &r.measurer());
    r.render(chart.frame(), &chart.data_view())
        .map_err(|e| e.to_string())?;
    if let Some(e) = r.take_gpu_error() {
        return Err(e);
    }
    let size = chart.frame().size_px;
    let (w, h) = (size.w.round() as u32, size.h.round() as u32);
    let pixels = r.read_pixels().map_err(|e| e.to_string())?;
    let size = tiny_skia::IntSize::from_wh(w, h).ok_or("invalid image size")?;
    // Opaque pixels: premultiplied and straight alpha are the same.
    let pixmap = tiny_skia::Pixmap::from_vec(pixels, size).ok_or("unexpected pixel buffer size")?;
    pixmap.encode_png().map_err(|e| e.to_string())
}
