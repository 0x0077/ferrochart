//! Named images for markers and plugin icons (r21): `registerIcon` decodes a
//! URL, image, bitmap, `ImageData` or canvas with the browser, and the core
//! stores it (`Chart::add_icon`). Names map to `IconId`s per chart.
//!
//! The tables live in a `thread_local` keyed by a per-chart number, because
//! layer plugins emit `icon` commands while the core builds the frame, where
//! the binding's scene is not reachable; `ChartScene::build_frame` marks the
//! chart being built.
//!

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use ferrochart_core::icon::{IconId, MAX_IMAGE_SIDE};
use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsError, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{ImageBitmap, OffscreenCanvas, OffscreenCanvasRenderingContext2d, Response};

use crate::web::js_err;

thread_local! {
    static TABLES: RefCell<HashMap<u32, HashMap<String, IconId>>> = RefCell::new(HashMap::new());
    static CURRENT: Cell<u32> = const { Cell::new(0) };
    static NEXT: Cell<u32> = const { Cell::new(1) };
}

/// A new chart's key (never 0).
pub(crate) fn new_key() -> u32 {
    NEXT.with(|n| {
        let k = n.get();
        n.set(k.wrapping_add(1).max(1));
        k
    })
}

/// Forgets a chart's names.
pub(crate) fn drop_key(key: u32) {
    let _ = TABLES.try_with(|t| t.borrow_mut().remove(&key));
}

/// Marks the chart whose frame is being built (for plugin `icon` commands).
pub(crate) fn set_current(key: u32) {
    CURRENT.with(|c| c.set(key));
}

pub(crate) fn lookup(key: u32, name: &str) -> Option<IconId> {
    TABLES.with(|t| t.borrow().get(&key)?.get(name).copied())
}

/// `lookup` for the chart being built.
pub(crate) fn lookup_current(name: &str) -> Option<IconId> {
    lookup(CURRENT.with(Cell::get), name)
}

pub(crate) fn name_of(key: u32, id: IconId) -> Option<String> {
    TABLES.with(|t| {
        t.borrow()
            .get(&key)?
            .iter()
            .find(|(_, v)| **v == id)
            .map(|(k, _)| k.clone())
    })
}

/// Maps `name` to `id`, returning the id it had.
pub(crate) fn insert(key: u32, name: String, id: IconId) -> Option<IconId> {
    TABLES.with(|t| t.borrow_mut().entry(key).or_default().insert(name, id))
}

pub(crate) fn remove(key: u32, name: &str) -> Option<IconId> {
    TABLES.with(|t| t.borrow_mut().get_mut(&key)?.remove(name))
}

/// The pixels of `source` as straight RGBA, scaled down (keeping the aspect
/// ratio) so no side exceeds 256: `(width, height, rgba)`.
pub(crate) async fn decode(source: JsValue) -> Result<(u32, u32, Vec<u8>), JsError> {
    let global = js_sys::global();
    let call = |name: &str, arg: &JsValue| -> Result<JsValue, JsError> {
        let f: Function = Reflect::get(&global, &JsValue::from_str(name))
            .ok()
            .and_then(|f| f.dyn_into().ok())
            .ok_or_else(|| js_err(format!("registerIcon: {name} is not available")))?;
        f.call1(&global, arg)
            .map_err(|e| js_err(format!("registerIcon: {}", describe(&e))))
    };
    let source = if let Some(url) = source.as_string() {
        let response: Response = JsFuture::from(js_sys::Promise::from(call("fetch", &url.into())?))
            .await
            .map_err(|e| js_err(format!("registerIcon: {}", describe(&e))))?
            .dyn_into()
            .map_err(|_| js_err("registerIcon: fetch did not return a Response"))?;
        if !response.ok() {
            return Err(js_err(format!(
                "registerIcon: fetching the image failed ({})",
                response.status()
            )));
        }
        let blob = response
            .blob()
            .map_err(|e| js_err(format!("registerIcon: {}", describe(&e))))?;
        JsFuture::from(blob)
            .await
            .map_err(|e| js_err(format!("registerIcon: {}", describe(&e))))?
    } else {
        source
    };
    let bitmap: ImageBitmap = JsFuture::from(js_sys::Promise::from(call(
        "createImageBitmap",
        &source,
    )?))
    .await
    .map_err(|e| {
        js_err(format!(
            "registerIcon: not an image (a URL, HTMLImageElement, ImageBitmap, ImageData or canvas): {}",
            describe(&e)
        ))
    })?
    .dyn_into()
    .map_err(|_| js_err("registerIcon: createImageBitmap did not return an ImageBitmap"))?;
    let (w, h) = (bitmap.width(), bitmap.height());
    if w == 0 || h == 0 {
        bitmap.close();
        return Err(js_err("registerIcon: the image is empty"));
    }
    let scale = (f64::from(MAX_IMAGE_SIDE) / f64::from(w.max(h))).min(1.0);
    let tw = ((f64::from(w) * scale).round() as u32).clamp(1, MAX_IMAGE_SIDE);
    let th = ((f64::from(h) * scale).round() as u32).clamp(1, MAX_IMAGE_SIDE);
    let result = (|| {
        let canvas = OffscreenCanvas::new(tw, th).map_err(|_| js_err("registerIcon: no canvas"))?;
        let ctx: OffscreenCanvasRenderingContext2d = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into().ok())
            .ok_or_else(|| js_err("registerIcon: no 2D context"))?;
        ctx.draw_image_with_image_bitmap_and_dw_and_dh(
            &bitmap,
            0.0,
            0.0,
            f64::from(tw),
            f64::from(th),
        )
        .map_err(|e| js_err(format!("registerIcon: {}", describe(&e))))?;
        let data = ctx
            .get_image_data(0.0, 0.0, f64::from(tw), f64::from(th))
            .map_err(|_| {
                js_err(
                    "registerIcon: the image's pixels cannot be read (a cross-origin image \
                     needs CORS headers)",
                )
            })?;
        Ok((tw, th, data.data().0))
    })();
    bitmap.close();
    result
}

fn describe(e: &JsValue) -> String {
    e.dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| e.as_string())
        .unwrap_or_else(|| format!("{e:?}"))
}
