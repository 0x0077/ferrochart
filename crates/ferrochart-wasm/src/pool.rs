//! Pools of charts sharing one GPU device (r25): pages of many small charts
//! (watchlists, tables) would otherwise create one device per chart, and
//! browsers keep only about 16 WebGL contexts alive.
//!
//! WebGPU: one device; every canvas gets its own `webgpu` context configured
//! with it, and the renderers share pipelines and glyph atlases. WebGL2: one
//! canvas outside the document holds the only WebGL context; each chart
//! draws there and copies the result to its own canvas with a 2D context.
//! Charts out of the viewport (`IntersectionObserver`) are not drawn.
//!
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::frame::Frame;
use ferrochart_core::geom::SizePx;
use ferrochart_core::render::RenderError;
use ferrochart_render_wgpu::{GpuContext, SharedResources, SharedSurface, WgpuRenderer};
use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, IntersectionObserver, IntersectionObserverEntry,
};

use crate::host::{self, js_err, string_option};
use crate::web::{FerroChart, Shared};

/// The WebGL2 pool's canvas and surface.
struct GlCanvas {
    canvas: HtmlCanvasElement,
    surface: RefCell<SharedSurface>,
}

/// How a pooled chart on WebGL2 gets its pixels: drawn on the pool's
/// canvas, then copied to its own canvas.
pub(crate) struct Blit {
    gl: Rc<GlCanvas>,
    ctx2d: CanvasRenderingContext2d,
}

impl Blit {
    pub(crate) fn render(
        &self,
        renderer: &mut WgpuRenderer,
        frame: &Frame,
        data: &ChartDataView<'_>,
    ) -> Result<(), RenderError> {
        let (w, h) = (frame.size_px.w.round(), frame.size_px.h.round());
        self.gl
            .surface
            .try_borrow_mut()
            .map_err(|_| RenderError::Backend("pool canvas busy"))?
            .render(renderer, frame, data)?;
        // The drawing buffer is valid until the browser composites: copy it
        // now, in the same task.
        let (w, h) = (f64::from(w), f64::from(h));
        self.ctx2d
            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &self.gl.canvas,
                0.0,
                0.0,
                w,
                h,
                0.0,
                0.0,
                w,
                h,
            )
            .map_err(|_| RenderError::Backend("could not copy the frame to the canvas"))
    }
}

/// The visibility observer and its callback.
type Observer = (IntersectionObserver, Closure<dyn FnMut(Array)>);

pub(crate) struct Pool {
    ctx: GpuContext,
    backend: &'static str,
    /// WebGL2: the shared canvas.
    gl: Option<Rc<GlCanvas>>,
    /// WebGPU: the first chart's resources, shared by the next ones.
    shared: RefCell<Option<SharedResources>>,
    charts: RefCell<Vec<Weak<Shared>>>,
    observer: RefCell<Option<Observer>>,
}

impl Pool {
    /// The GPU side of a new chart on `canvas` (sized `size`).
    pub(crate) fn renderer(
        &self,
        canvas: &HtmlCanvasElement,
        size: SizePx,
    ) -> Result<(GpuContext, WgpuRenderer, &'static str, Option<Blit>), JsError> {
        if let Some(gl) = &self.gl {
            let ctx2d: CanvasRenderingContext2d = canvas
                .get_context("2d")
                .map_err(|_| js_err("the canvas has another context"))?
                .ok_or_else(|| {
                    js_err("the canvas has another context (a pool needs a fresh canvas)")
                })?
                .dyn_into()
                .map_err(|_| js_err("no 2D context"))?;
            let _ = ctx2d.set_global_composite_operation("copy");
            let renderer = {
                let surface = gl.surface.borrow();
                WgpuRenderer::for_shared_surface(&self.ctx, surface.resources())
            };
            let blit = Blit {
                gl: Rc::clone(gl),
                ctx2d,
            };
            return Ok((self.ctx.clone(), renderer, self.backend, Some(blit)));
        }
        let surface = self
            .ctx
            .instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(js_err)?;
        let mut shared = self.shared.borrow_mut();
        let renderer = match shared.as_ref() {
            Some(s) => WgpuRenderer::with_surface_shared(&self.ctx, surface, size, s),
            None => WgpuRenderer::with_surface(&self.ctx, surface, size),
        }
        .map_err(js_err)?;
        shared.get_or_insert_with(|| renderer.shared_resources());
        Ok((self.ctx.clone(), renderer, self.backend, None))
    }

    /// Adds a chart: watched for visibility, destroyed with the pool.
    fn adopt(&self, chart: &Rc<Shared>) {
        let mut charts = self.charts.borrow_mut();
        charts.retain(|c| c.strong_count() > 0);
        charts.push(Rc::downgrade(chart));
        if let Some((observer, _)) = self.observer.borrow().as_ref()
            && let Ok(state) = chart.state.try_borrow()
        {
            observer.observe(&state.canvas);
        }
    }

    /// Visibility changes from the observer.
    fn seen(&self, entries: &Array) {
        let charts: Vec<Rc<Shared>> = self
            .charts
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for entry in entries.iter() {
            let Ok(entry) = entry.dyn_into::<IntersectionObserverEntry>() else {
                continue;
            };
            let target = entry.target();
            for c in &charts {
                let same = c
                    .state
                    .try_borrow()
                    .is_ok_and(|s| AsRef::<web_sys::Element>::as_ref(&s.canvas) == &target);
                if same {
                    let visible = entry.is_intersecting();
                    let was = c.visible.replace(visible);
                    if visible && !was {
                        host::schedule(c);
                    }
                }
            }
        }
    }
}

/// Charts that share one GPU device (r25). Make one with
/// `FerroChart.createPool`, then charts with `pool.create(canvas, options)`.
#[wasm_bindgen]
pub struct ChartPool {
    pool: Rc<Pool>,
}

#[wasm_bindgen]
impl FerroChart {
    /// A pool of charts sharing one GPU device (r25): for pages with many
    /// small charts. `renderer` as in `create` (default `'auto'`).
    ///
    /// @param {{ renderer?: 'auto' | 'webgpu' | 'webgl2' }} [options]
    /// @returns {Promise<ChartPool>}
    #[wasm_bindgen(js_name = createPool)]
    pub async fn create_pool(
        #[wasm_bindgen(
            unchecked_optional_param_type = "{ renderer?: 'auto' | 'webgpu' | 'webgl2' }"
        )]
        options: Option<JsValue>,
    ) -> Result<ChartPool, JsError> {
        console_error_panic_hook::set_once();
        let options = options.unwrap_or(JsValue::UNDEFINED);
        let mode = string_option(&options, "renderer").unwrap_or_else(|| "auto".to_owned());
        let mut errors = Vec::new();
        let mut made = None;
        if mode == "auto" || mode == "webgpu" {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(desc);
            match instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
            {
                Ok(adapter) => {
                    let ctx = GpuContext::from_adapter(instance, adapter)
                        .await
                        .map_err(js_err)?;
                    made = Some((ctx, "webgpu", None));
                }
                Err(_) => errors.push("WebGPU is not available"),
            }
        }
        if made.is_none() && (mode == "auto" || mode == "webgl2") {
            let canvas: HtmlCanvasElement = web_sys::window()
                .and_then(|w| w.document())
                .ok_or_else(|| js_err("no document"))?
                .create_element("canvas")
                .map_err(|_| js_err("could not create a canvas"))?
                .dyn_into()
                .map_err(|_| js_err("could not create a canvas"))?;
            canvas.set_width(1);
            canvas.set_height(1);
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::GL;
            let instance = wgpu::Instance::new(desc);
            match instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone())) {
                Ok(surface) => match GpuContext::request(instance, Some(&surface)).await {
                    Ok(ctx) => {
                        let surface = SharedSurface::new(&ctx, surface, SizePx { w: 1.0, h: 1.0 })
                            .map_err(js_err)?;
                        let gl = Rc::new(GlCanvas {
                            canvas,
                            surface: RefCell::new(surface),
                        });
                        made = Some((ctx, "webgl2", Some(gl)));
                    }
                    Err(_) => errors.push("WebGL2 is not available"),
                },
                Err(_) => errors.push("WebGL2 is not available"),
            }
        }
        let Some((ctx, backend, gl)) = made else {
            if errors.is_empty() {
                return Err(js_err(format!(
                    "unknown renderer '{mode}' (expected 'auto', 'webgpu' or 'webgl2')"
                )));
            }
            return Err(js_err(errors.join("; ")));
        };
        let pool = Rc::new(Pool {
            ctx,
            backend,
            gl,
            shared: RefCell::new(None),
            charts: RefCell::new(Vec::new()),
            observer: RefCell::new(None),
        });
        let weak = Rc::downgrade(&pool);
        let callback = Closure::<dyn FnMut(Array)>::new(move |entries: Array| {
            if let Some(p) = weak.upgrade() {
                p.seen(&entries);
            }
        });
        if let Ok(observer) = IntersectionObserver::new(callback.as_ref().unchecked_ref()) {
            *pool.observer.borrow_mut() = Some((observer, callback));
        }
        Ok(ChartPool { pool })
    }
}

#[wasm_bindgen]
impl ChartPool {
    /// The GPU API the pool uses: `'webgpu'` or `'webgl2'`.
    #[wasm_bindgen(getter, unchecked_return_type = "'webgpu' | 'webgl2'")]
    pub fn renderer(&self) -> String {
        self.pool.backend.to_owned()
    }

    /// A chart in this pool, with the options of `FerroChart.create`
    /// (`renderer` is the pool's). On WebGL2 the canvas gets a 2D context,
    /// so it must not have another one yet.
    ///
    /// @param {HTMLCanvasElement} canvas
    /// @param {ChartCreateOptions} [options]
    /// @returns {Promise<FerroChart>}
    pub async fn create(
        &self,
        canvas: HtmlCanvasElement,
        #[wasm_bindgen(unchecked_optional_param_type = "ChartCreateOptions")] options: Option<
            JsValue,
        >,
    ) -> Result<FerroChart, JsError> {
        let chart = FerroChart::build(canvas, options, Some(&self.pool)).await?;
        self.pool.adopt(&chart.shared);
        Ok(chart)
    }

    /// Number of charts alive in the pool.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        let mut charts = self.pool.charts.borrow_mut();
        charts.retain(|c| c.strong_count() > 0);
        charts.len()
    }

    /// Destroys every chart of the pool and releases the device. Charts
    /// made from it can no longer be used.
    pub fn destroy(self) {
        if let Some((observer, _)) = self.pool.observer.borrow_mut().take() {
            observer.disconnect();
        }
        let charts: Vec<Rc<Shared>> = self
            .pool
            .charts
            .borrow_mut()
            .drain(..)
            .filter_map(|c| c.upgrade())
            .collect();
        for c in charts {
            crate::api::leave_group(&c);
            c.shut_down();
        }
    }
}

/// `toBlob` of charts and plots: a snapshot drawn into an `OffscreenCanvas`
/// and encoded (r25).
pub(crate) async fn blob_of(
    bitmap: js_sys::Promise,
    kind: Option<String>,
    quality: Option<f64>,
) -> Result<JsValue, JsError> {
    use wasm_bindgen_futures::JsFuture;
    let bitmap: web_sys::ImageBitmap = JsFuture::from(bitmap)
        .await
        .map_err(|_| js_err("the browser could not read the canvas"))?
        .dyn_into()
        .map_err(|_| js_err("not an ImageBitmap"))?;
    let kind = kind.unwrap_or_else(|| "image/png".to_owned());
    if !matches!(kind.as_str(), "image/png" | "image/webp" | "image/jpeg") {
        return Err(js_err(format!(
            "unknown image type '{kind}' (expected 'image/png', 'image/webp' or 'image/jpeg')"
        )));
    }
    let canvas = web_sys::OffscreenCanvas::new(bitmap.width(), bitmap.height())
        .map_err(|_| js_err("OffscreenCanvas is not available"))?;
    let g: web_sys::OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into().ok())
        .ok_or_else(|| js_err("no 2D context"))?;
    g.draw_image_with_image_bitmap(&bitmap, 0.0, 0.0)
        .map_err(|_| js_err("could not draw the snapshot"))?;
    bitmap.close();
    let options = web_sys::ImageEncodeOptions::new();
    options.set_type(&kind);
    if let Some(q) = quality {
        options.set_quality(q.clamp(0.0, 1.0));
    }
    let blob = canvas
        .convert_to_blob_with_options(&options)
        .map_err(|_| js_err("could not encode the image"))?;
    JsFuture::from(blob)
        .await
        .map_err(|_| js_err("could not encode the image"))
}
