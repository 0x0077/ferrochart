//! Browser glyphs (`text-browser`, r21): the last link of the text fallback
//! chain on the web. Characters that neither the bundled font nor a fallback
//! font has are drawn by the browser on an `OffscreenCanvas`, with the
//! system's fonts: CJK, other scripts, and color emoji. No font is shipped,
//! so the package size does not change.
//!
//! Glyphs are drawn in white and read back: pixels that are not gray mark a
//! color glyph (emoji), drawn as is; the others become coverage drawn in the
//! text color. Advances come from `measureText`. The output depends on the
//! system's fonts, so it is checked by browser tests, not goldens.
//!

use std::cell::RefCell;
use std::collections::HashMap;

use ferrochart_text::{GlyphBitmap, GlyphSource};
use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};

/// The fonts tried, in order: the system UI font, common CJK fonts, color
/// emoji fonts.
const FONT_STACK: &str = "system-ui, \"PingFang SC\", \"Hiragino Sans\", \"Microsoft YaHei\", \
     \"Noto Sans CJK SC\", \"Apple Color Emoji\", \"Segoe UI Emoji\", \"Noto Color Emoji\", \
     sans-serif";
/// Largest glyph side read back, in pixels.
const MAX_SIDE: u32 = 512;
/// A pixel at least this opaque whose channels differ by more than
/// `COLOR_SPREAD` makes a glyph a color glyph (gray text drawn in white has
/// equal channels, up to rounding at low alpha).
const COLOR_ALPHA: u8 = 64;
const COLOR_SPREAD: u8 = 24;

struct Canvas {
    surface: OffscreenCanvas,
    ctx: OffscreenCanvasRenderingContext2d,
    /// Bits of the font size set on `ctx`, in pixels (0 after a resize,
    /// which resets the context).
    font_px: u32,
    advances: HashMap<(char, u32), f32>,
}

thread_local! {
    // JS objects are not `Send`; the source itself is a unit struct (the
    // engine requires `Send + Sync`) and the canvas lives here. wasm32 runs
    // the chart on one thread.
    static CANVAS: RefCell<Option<Canvas>> = const { RefCell::new(None) };
}

impl Canvas {
    fn new() -> Option<Self> {
        let surface = OffscreenCanvas::new(64, 64).ok()?;
        let options = Object::new();
        Reflect::set(
            &options,
            &JsValue::from_str("willReadFrequently"),
            &JsValue::TRUE,
        )
        .ok()?;
        let ctx = surface
            .get_context_with_context_options("2d", &options)
            .ok()??
            .dyn_into::<OffscreenCanvasRenderingContext2d>()
            .ok()?;
        Some(Self {
            surface,
            ctx,
            font_px: 0,
            advances: HashMap::new(),
        })
    }

    fn set_font(&mut self, size: f32) {
        if self.font_px != size.to_bits() {
            self.ctx.set_font(&format!("{size}px {FONT_STACK}"));
            self.font_px = size.to_bits();
        }
    }

    fn advance(&mut self, c: char, size: f32) -> f32 {
        if let Some(&a) = self.advances.get(&(c, size.to_bits())) {
            return a;
        }
        self.set_font(size);
        let a = self
            .ctx
            .measure_text(c.encode_utf8(&mut [0; 4]))
            .map_or(0.0, |m| m.width() as f32);
        self.advances.insert((c, size.to_bits()), a);
        a
    }

    fn rasterize(&mut self, c: char, size: f32) -> Option<GlyphBitmap> {
        let mut buf = [0; 4];
        let s = c.encode_utf8(&mut buf);
        self.set_font(size);
        let m = self.ctx.measure_text(s).ok()?;
        // The ink box around the pen at (0, baseline), one pixel of margin
        // for antialiasing.
        let x0 = (-m.actual_bounding_box_left()).floor() as i32 - 1;
        let x1 = m.actual_bounding_box_right().ceil() as i32 + 1;
        let y0 = (-m.actual_bounding_box_ascent()).floor() as i32 - 1;
        let y1 = m.actual_bounding_box_descent().ceil() as i32 + 1;
        let w = u32::try_from(x1 - x0).ok()?;
        let h = u32::try_from(y1 - y0).ok()?;
        if w <= 2 || h <= 2 || w > MAX_SIDE || h > MAX_SIDE {
            return None;
        }
        if self.surface.width() < w || self.surface.height() < h {
            self.surface.set_width(self.surface.width().max(w));
            self.surface.set_height(self.surface.height().max(h));
            self.font_px = 0;
            self.set_font(size);
        }
        let (wf, hf) = (f64::from(w), f64::from(h));
        self.ctx.clear_rect(0.0, 0.0, wf, hf);
        self.ctx.set_fill_style_str("#fff");
        self.ctx.set_text_baseline("alphabetic");
        self.ctx.fill_text(s, f64::from(-x0), f64::from(-y0)).ok()?;
        let rgba = self.ctx.get_image_data(0.0, 0.0, wf, hf).ok()?.data().0;
        let coverage: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
        if coverage.iter().all(|&a| a == 0) {
            return None;
        }
        let color = rgba.chunks_exact(4).any(|p| {
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            p[3] >= COLOR_ALPHA && hi - lo > COLOR_SPREAD
        });
        Some(GlyphBitmap {
            width: w,
            height: h,
            left: x0,
            top: y0,
            coverage,
            rgba: color.then_some(rgba),
        })
    }
}

/// The browser as a [`GlyphSource`].
pub(crate) struct BrowserGlyphs;

impl BrowserGlyphs {
    /// `None` when the browser has no `OffscreenCanvas` 2D context; text
    /// then falls back to `□`.
    pub(crate) fn new() -> Option<Self> {
        CANVAS
            .with(|cell| {
                let mut canvas = cell.try_borrow_mut().ok()?;
                if canvas.is_none() {
                    *canvas = Canvas::new();
                }
                Some(canvas.is_some())
            })
            .unwrap_or(false)
            .then_some(Self)
    }
}

fn with_canvas<R>(f: impl FnOnce(&mut Canvas) -> Option<R>) -> Option<R> {
    CANVAS.with(|cell| f(cell.try_borrow_mut().ok()?.as_mut()?))
}

impl GlyphSource for BrowserGlyphs {
    fn advance(&self, c: char, size: f32) -> f32 {
        with_canvas(|canvas| Some(canvas.advance(c, size))).unwrap_or(0.0)
    }

    fn rasterize(&self, c: char, size: f32) -> Option<GlyphBitmap> {
        with_canvas(|canvas| canvas.rasterize(c, size))
    }
}
