//! The attribution logo: a DOM link next to
//! the canvas, at the bottom-left corner of the lowest plot area, just above
//! the time axis. It is not in the `Frame`: backends, goldens, `to_png` and
//! native hosts never draw it. Attribution courtesy, not a license term.

use std::cell::Cell;

use ferrochart_core::geom::PointPx;
use ferrochart_core::theme::Theme;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlElement};

use crate::host::Listener;

/// Logo box and inset from the plot corner, CSS px: the size and place of
/// the lightweight-charts logo (35×19, 10 px in), whose glyph is as tall as
/// ours and has about the same ink.
const SIZE: f64 = 19.0;
const INSET: f64 = 10.0;

/// The FerroChart mark (site/app/icon.svg): its 512-unit square plus 32 units
/// around it, so the glyph is 17 px in the 19 px box and the 1 px margin
/// holds an outline in the background colour (61 units ≈ 2 px wide, half of
/// it outside), which keeps it readable over candles.
///
/// Under the pointer (or keyboard focus) the link opens to the right into a
/// pill with the product name, like the logo on `TradingView`'s own charts; at
/// rest it is the 19 px mark alone.
const PATHS: &str = r#"<path d="M64 64H422L357 287H285L64 230Z"/><path d="M466 64H576V400L368 352V304Z"/><path d="M64 280L285 309V359L177 576H64Z"/><path d="M304 369H367L576 447V576H231Z"/>"#;

pub(crate) struct Logo {
    el: HtmlElement,
    /// Last position written (left, top), CSS px.
    at: Cell<(f64, f64)>,
    _hover: [Listener; 4],
}

/// Style of the link closed (the mark) and open (the mark and the name).
const CLOSED: [(&str, &str); 3] = [
    ("max-width", "19px"),
    ("background-color", "transparent"),
    ("box-shadow", "none"),
];
const OPEN: [(&str, &str); 2] = [
    ("max-width", "160px"),
    ("box-shadow", "0 1px 6px rgba(0, 0, 0, 0.25)"),
];

impl Logo {
    /// Inserts the logo right after `canvas`; `None` when the canvas is not
    /// in a document.
    pub(crate) fn new(canvas: &HtmlCanvasElement, theme: &Theme) -> Option<Self> {
        let parent = canvas.parent_node()?;
        let doc = canvas.owner_document()?;
        let el: HtmlElement = doc.create_element("a").ok()?.dyn_into().ok()?;
        for (k, v) in [
            ("href", "https://ferrochart.com"),
            ("target", "_blank"),
            ("rel", "noopener"),
            ("aria-label", "FerroChart"),
        ] {
            let _ = el.set_attribute(k, v);
        }
        let bg = theme.background;
        let luma = 0.2126 * f64::from(bg.r) + 0.7152 * f64::from(bg.g) + 0.0722 * f64::from(bg.b);
        let color = if luma < 128.0 { "#d1d4dc" } else { "#131722" };
        let bg_css = format!("rgb({},{},{})", bg.r, bg.g, bg.b);
        el.set_inner_html(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="32 32 576 576" width="19" height="19" aria-hidden="true" focusable="false" style="flex:none;display:block"><g fill="{bg_css}" stroke="{bg_css}" stroke-width="61" stroke-linejoin="round">{PATHS}</g><g fill="currentColor">{PATHS}</g></svg><span aria-hidden="true" style="padding:0 8px 0 5px">FerroChart</span>"#,
        ));
        let style = el.style();
        for (k, v) in [
            ("position", "absolute"),
            ("display", "flex"),
            ("align-items", "center"),
            ("height", "19px"),
            ("left", "0px"),
            ("top", "0px"),
            ("overflow", "hidden"),
            ("white-space", "nowrap"),
            ("border-radius", "4px"),
            (
                "font",
                "600 12px/19px system-ui, -apple-system, 'Segoe UI', sans-serif",
            ),
            ("letter-spacing", "-0.01em"),
            ("text-decoration", "none"),
            ("color", color),
            (
                "transition",
                "max-width 220ms ease, background-color 220ms ease, box-shadow 220ms ease",
            ),
            ("pointer-events", "auto"),
            ("z-index", "1"),
        ]
        .into_iter()
        .chain(CLOSED)
        {
            let _ = style.set_property(k, v);
        }
        let _ = parent.insert_before(&el, canvas.next_sibling().as_ref());
        let toggle = |open: bool| {
            let el = el.clone();
            let bg_css = bg_css.clone();
            move |_| {
                let style = el.style();
                if open {
                    for (k, v) in OPEN {
                        let _ = style.set_property(k, v);
                    }
                    let _ = style.set_property("background-color", &bg_css);
                } else {
                    for (k, v) in CLOSED {
                        let _ = style.set_property(k, v);
                    }
                }
            }
        };
        let hover = [
            Listener::new(&el, "pointerenter", true, toggle(true)).ok()?,
            Listener::new(&el, "pointerleave", true, toggle(false)).ok()?,
            Listener::new(&el, "focus", true, toggle(true)).ok()?,
            Listener::new(&el, "blur", true, toggle(false)).ok()?,
        ];
        Some(Self {
            el,
            at: Cell::new((f64::NAN, f64::NAN)),
            _hover: hover,
        })
    }

    /// Moves the logo to `anchor` (device px of the canvas): `INSET` px in
    /// from the bottom-left corner of the plot area. Positioned in the
    /// canvas's offset parent, like the canvas itself.
    pub(crate) fn place(&self, canvas: &HtmlCanvasElement, anchor: PointPx, dpr: f64) {
        let dpr = if dpr > 0.0 { dpr } else { 1.0 };
        let left = f64::from(canvas.offset_left())
            + f64::from(canvas.client_left())
            + f64::from(anchor.x) / dpr
            + INSET;
        let top = f64::from(canvas.offset_top())
            + f64::from(canvas.client_top())
            + f64::from(anchor.y) / dpr
            - INSET
            - SIZE;
        let (left, top) = (left.round(), top.round());
        #[allow(clippy::float_cmp)] // exact: unchanged rounded values
        if self.at.get() == (left, top) {
            return;
        }
        self.at.set((left, top));
        let style = self.el.style();
        let _ = style.set_property("left", &format!("{left}px"));
        let _ = style.set_property("top", &format!("{top}px"));
    }

    /// Takes the logo out of the page.
    pub(crate) fn remove(self) {
        self.el.remove();
    }
}
