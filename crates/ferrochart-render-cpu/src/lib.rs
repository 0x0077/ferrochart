//! CPU rendering backend for FerroChart, built on `tiny-skia`.
//!
//! Executes the `DrawCmd`s of a `Frame` one by one. `DrawCmd::Series` is expanded
//! on the CPU with the core's `transform_helpers`, the same f32 arithmetic the
//! wgpu shaders use, so both backends produce the same pixels. Rectangles,
//! axis-aligned lines and text are drawn without anti-aliasing by direct
//! source-over blending into the pixmap; only slanted or dashed lines go through
//! `tiny-skia`'s anti-aliased stroker.
//!
//! Uses: golden-image tests, server-side rendering, fallback without a GPU. No
//! performance targets.
//!
//! # Examples
//!
//! ```
//! use ferrochart_core::chart::{SeriesData, SeriesOptions};
//! use ferrochart_core::frame::SeriesKind;
//! use ferrochart_core::geom::SizeCss;
//! use ferrochart_core::render::Renderer;
//! use ferrochart_core::testing::random_walk;
//! use ferrochart_core::{Chart, ChartOptions};
//! use ferrochart_render_cpu::CpuRenderer;
//!
//! let mut chart = Chart::new(ChartOptions::default());
//! chart.resize(SizeCss { w: 320.0, h: 200.0 }, 1.0);
//! let id = chart.add_series(chart.main_pane(), SeriesKind::Candlestick, SeriesOptions::default())?;
//! chart.set_data(id, SeriesData::Ohlc(random_walk(7, 200, 0, 60_000)?))?;
//!
//! let mut renderer = CpuRenderer::new();
//! chart.build_frame(0.0, &renderer.measurer());
//! renderer.render(chart.frame(), &chart.data_view())?;
//! assert_eq!(renderer.pixmap().map(|p| p.width()), Some(320));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
#![forbid(unsafe_code)]

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::data::OhlcLevelView;
use ferrochart_core::frame::{
    DrawCmd, FontStyleRef, Frame, IconInst, LineDash, LineInst, PriceMapping, SeriesDraw,
    SeriesKind, SeriesStyle, TextAlign, TextRun, YTransform,
};
use ferrochart_core::geom::{PointPx, Px, RectPx, Rgba, SizePx};
use ferrochart_core::render::{RenderError, Renderer};
use ferrochart_core::text::TextMeasurer;
use ferrochart_core::transform_helpers::{
    ICON_CIRCLE, band_coverage, bar_metrics, candle, chunk_of, coverage_8, dash_on, fill_coverage,
    fill_edge, gradient_t, histogram, histogram_value_y, icon_coverage, initial_chunk_ref, is_gap,
    is_up_rel, line_axis, line_coverage, line_segments, ohlc_bar, ohlc_rel, segment_coverage,
    v_rel, x_center, y, y_bias, y_of_price,
};
use ferrochart_text::{GlyphCache, TextEngine};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Transform};

/// Adapts a [`TextEngine`] to the core's [`TextMeasurer`].
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{FontId, FontStyleRef};
/// use ferrochart_core::text::text_width;
/// use ferrochart_render_cpu::EngineMeasurer;
/// use ferrochart_text::TextEngine;
///
/// let engine = TextEngine::new();
/// let style = FontStyleRef { font: FontId(0), size: 10.0 };
/// let w = text_width(&EngineMeasurer(&engine), "100.00", style);
/// assert!((w - engine.width(0, "100.00", 10.0)).abs() < 1e-4);
/// ```
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

/// Integer pixel box `[x0, x1) × [y0, y1)`.
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

/// The CPU renderer.
#[derive(Debug)]
pub struct CpuRenderer {
    pixmap: Option<Pixmap>,
    text: TextEngine,
    glyphs: GlyphCache,
    clips: Vec<IBox>,
}

impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuRenderer {
    /// A renderer with the bundled font. The target is sized by the first frame.
    pub fn new() -> Self {
        Self::with_text_engine(TextEngine::new())
    }

    /// A renderer with a custom text engine (extra fonts).
    pub fn with_text_engine(text: TextEngine) -> Self {
        Self {
            pixmap: None,
            text,
            glyphs: GlyphCache::default(),
            clips: Vec::new(),
        }
    }

    /// The text measurer to pass to `Chart::build_frame`.
    pub fn measurer(&self) -> EngineMeasurer<'_> {
        EngineMeasurer(&self.text)
    }

    /// The text engine.
    pub fn text_engine(&self) -> &TextEngine {
        &self.text
    }

    /// The rendered image (premultiplied RGBA), once something was rendered.
    pub fn pixmap(&self) -> Option<&Pixmap> {
        self.pixmap.as_ref()
    }

    /// The rendered image as PNG.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidSize`] if nothing was rendered yet,
    /// [`RenderError::Backend`] if encoding fails.
    pub fn encode_png(&self) -> Result<Vec<u8>, RenderError> {
        self.pixmap
            .as_ref()
            .ok_or(RenderError::InvalidSize)?
            .encode_png()
            .map_err(|_| RenderError::Backend("PNG encoding failed"))
    }

    fn ensure_target(&mut self, size: SizePx) -> Result<(), RenderError> {
        let (w, h) = (size.w.round(), size.h.round());
        if !(w >= 1.0 && h >= 1.0 && w <= 16_384.0 && h <= 16_384.0) {
            return Err(RenderError::InvalidSize);
        }
        let (w, h) = (w as u32, h as u32);
        if self
            .pixmap
            .as_ref()
            .is_none_or(|p| p.width() != w || p.height() != h)
        {
            self.pixmap = Some(Pixmap::new(w, h).ok_or(RenderError::InvalidSize)?);
        }
        Ok(())
    }

    fn clip(&self) -> Option<IBox> {
        self.clips.last().copied()
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, frame: &Frame, data: &ChartDataView<'_>) -> Result<(), RenderError> {
        self.ensure_target(frame.size_px)?;
        let Some(mut pixmap) = self.pixmap.take() else {
            return Err(RenderError::InvalidSize);
        };
        let c = frame.clear_color;
        pixmap.fill(tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a));
        let full = IBox {
            x0: 0,
            y0: 0,
            x1: pixmap.width() as i32,
            y1: pixmap.height() as i32,
        };
        for layer in &frame.layers {
            self.clips.clear();
            self.clips.push(full);
            for cmd in &layer.cmds {
                self.execute(&mut pixmap, frame, *data, cmd);
            }
        }
        self.pixmap = Some(pixmap);
        Ok(())
    }

    fn resize(&mut self, size: SizePx) {
        // An invalid size keeps the old target; `render` reports it.
        let _ = self.ensure_target(size);
    }
}

impl CpuRenderer {
    fn execute(&mut self, pm: &mut Pixmap, frame: &Frame, data: ChartDataView<'_>, cmd: &DrawCmd) {
        let pools = &frame.pools;
        match *cmd {
            DrawCmd::PushClip(r) => {
                let top = self
                    .clip()
                    .map_or(IBox::of_rect(r), |c| c.intersect(IBox::of_rect(r)));
                self.clips.push(top);
            }
            // Never pop the full-target clip.
            DrawCmd::PopClip if self.clips.len() > 1 => {
                self.clips.pop();
            }
            DrawCmd::Rects(span) => {
                for r in pools.rects.get(span.range()).unwrap_or(&[]) {
                    self.fill_rect(pm, r.rect, r.color);
                }
            }
            DrawCmd::Lines(span) => {
                for l in pools.lines.get(span.range()).unwrap_or(&[]) {
                    self.draw_line(pm, *l);
                }
            }
            DrawCmd::Polyline { points, style } => {
                let pts = pools.points.get(points.range()).unwrap_or(&[]);
                for w in pts.windows(2) {
                    self.draw_line(
                        pm,
                        LineInst {
                            from: w[0],
                            to: w[1],
                            stroke: style,
                        },
                    );
                }
            }
            DrawCmd::Polygon { points, fill } => {
                let pts = pools.points.get(points.range()).unwrap_or(&[]);
                self.fill_polygon(pm, pts, fill);
            }
            DrawCmd::Text(span) => {
                for run in pools.text_runs.get(span.range()).unwrap_or(&[]) {
                    let text = pools.text.get(run.text.range()).unwrap_or("");
                    self.draw_text(pm, run, text);
                }
            }
            DrawCmd::Series(s) => self.draw_series(pm, frame, data, &s),
            DrawCmd::Icon(span) => {
                for icon in pools.icons.get(span.range()).unwrap_or(&[]) {
                    self.draw_icon(pm, icon);
                }
            }
            // Unbalanced pops are ignored. No native kinds are declared here,
            // so core never sends a custom draw.
            DrawCmd::PopClip | DrawCmd::Custom(_) => {}
        }
    }

    /// Source-over blend of `color × coverage/255` into pixel `(x, y)`.
    fn blend(pm: &mut Pixmap, x: i32, y: i32, color: Rgba, coverage: u8) {
        let w = pm.width() as i32;
        let i = (y * w + x) as usize * 4;
        let a = u32::from(color.a) * u32::from(coverage) / 255;
        if a == 0 {
            return;
        }
        let data = pm.data_mut();
        let Some(px) = data.get_mut(i..i + 4) else {
            return;
        };
        let inv = 255 - a;
        let src = [color.r, color.g, color.b];
        for k in 0..3 {
            let s = u32::from(src[k]) * a / 255;
            px[k] = (s + (u32::from(px[k]) * inv + 127) / 255).min(255) as u8;
        }
        px[3] = (a + (u32::from(px[3]) * inv + 127) / 255).min(255) as u8;
    }

    fn fill_box(&self, pm: &mut Pixmap, b: IBox, color: Rgba) {
        let Some(clip) = self.clip() else {
            return;
        };
        let b = b.intersect(clip);
        if b.is_empty() || color.a == 0 {
            return;
        }
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                Self::blend(pm, x, y, color, 255);
            }
        }
    }

    /// Built-in icon by its coverage function; shader `fs_icon`.
    fn draw_icon(&self, pm: &mut Pixmap, icon: &IconInst) {
        let Some(clip) = self.clip() else {
            return;
        };
        let reach = icon.size.0 * 0.5 + 1.0;
        let b = IBox {
            x0: (icon.pos.x - reach).floor() as i32,
            y0: (icon.pos.y - reach).floor() as i32,
            x1: (icon.pos.x + reach).ceil() as i32,
            y1: (icon.pos.y + reach).ceil() as i32,
        }
        .intersect(clip);
        if b.is_empty() || icon.color.a == 0 {
            return;
        }
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                let dx = x as f32 + 0.5 - icon.pos.x;
                let dy = y as f32 + 0.5 - icon.pos.y;
                let c = icon_coverage(icon.icon, dx, dy, icon.size.0);
                Self::blend(pm, x, y, icon.color, coverage_8(c));
            }
        }
    }

    fn fill_rect(&self, pm: &mut Pixmap, r: RectPx, color: Rgba) {
        self.fill_box(pm, IBox::of_rect(r), color);
    }

    fn draw_line(&self, pm: &mut Pixmap, l: LineInst) {
        let width = l.stroke.width.0.max(1.0).round();
        let solid = l.stroke.dash == LineDash::Solid;
        // Axis-aligned solid lines: exact pixel columns/rows, no anti-aliasing.
        let vertical = (l.from.x - l.to.x).abs() < 1e-3;
        let horizontal = (l.from.y - l.to.y).abs() < 1e-3;
        if solid && (vertical || horizontal) {
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
            self.fill_rect(pm, r, l.stroke.color);
            return;
        }
        // Slanted or dashed: per-pixel coverage, as the wgpu line shader.
        let Some(clip) = self.clip() else {
            return;
        };
        let (a, b) = (l.from, l.to);
        let (dir, len) = line_axis(a, b);
        let reach = width * 0.5 + 1.0;
        let bx = IBox {
            x0: (a.x.min(b.x) - reach).floor() as i32,
            y0: (a.y.min(b.y) - reach).floor() as i32,
            x1: (a.x.max(b.x) + reach).ceil() as i32,
            y1: (a.y.max(b.y) + reach).ceil() as i32,
        }
        .intersect(clip);
        if bx.is_empty() || l.stroke.color.a == 0 {
            return;
        }
        for y in bx.y0..bx.y1 {
            for x in bx.x0..bx.x1 {
                let px = x as f32 + 0.5 - a.x;
                let py = y as f32 + 0.5 - a.y;
                let along = px * dir.x + py * dir.y;
                let across = py * dir.x - px * dir.y;
                let c = line_coverage(along, across, len, Px(width), l.stroke.dash);
                if c > 0.0 {
                    Self::blend(pm, x, y, l.stroke.color, coverage_8(c));
                }
            }
        }
    }

    fn fill_polygon(&self, pm: &mut Pixmap, pts: &[PointPx], fill: Rgba) {
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(p.x, p.y);
            } else {
                pb.line_to(p.x, p.y);
            }
        }
        pb.close();
        let Some(path) = pb.finish() else {
            return;
        };
        let mask = self.mask(pm);
        // Not anti-aliased: pixels whose centers are inside, as the wgpu
        // backend's triangles. Drawing outlines cover the edges.
        let mut paint = Self::paint(fill);
        paint.anti_alias = false;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            mask.as_ref(),
        );
    }

    fn paint(c: Rgba) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.r, c.g, c.b, c.a);
        paint.anti_alias = true;
        paint
    }

    /// A mask for the current clip, `None` if it covers the whole target.
    fn mask(&self, pm: &Pixmap) -> Option<Mask> {
        let clip = self.clip()?;
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        if clip
            == (IBox {
                x0: 0,
                y0: 0,
                x1: w,
                y1: h,
            })
        {
            return None;
        }
        let mut mask = Mask::new(pm.width(), pm.height())?;
        let rect = tiny_skia::Rect::from_ltrb(
            clip.x0 as f32,
            clip.y0 as f32,
            clip.x1 as f32,
            clip.y1 as f32,
        )?;
        let path = PathBuilder::from_rect(rect);
        mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
        Some(mask)
    }

    fn draw_text(&mut self, pm: &mut Pixmap, run: &TextRun, text: &str) {
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
        // `pos.y` is the center of the ascent..descent box.
        let baseline = (run.pos.y + (m.ascent - m.descent) * 0.5).round();
        if let Some(bg) = run.bg {
            let pad = bg.padding.0;
            self.fill_rect(
                pm,
                RectPx {
                    x: x0 - pad,
                    y: baseline - m.ascent - pad,
                    w: width + 2.0 * pad,
                    h: m.ascent + m.descent + 2.0 * pad,
                },
                bg.color,
            );
        }
        let Some(clip) = self.clip() else {
            return;
        };
        let mut pen = 0.0_f32;
        for ch in text.chars() {
            let gx = (x0 + pen).round() as i32;
            pen += self.text.advance(font, ch, size);
            let Some(g) = self.glyphs.get(&self.text, font, ch, size) else {
                continue;
            };
            let (left, top) = (gx + g.left, baseline as i32 + g.top);
            for gy in 0..g.height as i32 {
                let y = top + gy;
                if y < clip.y0 || y >= clip.y1 {
                    continue;
                }
                for gxi in 0..g.width as i32 {
                    let x = left + gxi;
                    if x < clip.x0 || x >= clip.x1 {
                        continue;
                    }
                    let cov = g.coverage[(gy * g.width as i32 + gxi) as usize];
                    if cov > 0 {
                        Self::blend(pm, x, y, run.color, cov);
                    }
                }
            }
        }
    }

    fn draw_series(
        &mut self,
        pm: &mut Pixmap,
        frame: &Frame,
        data: ChartDataView<'_>,
        s: &SeriesDraw,
    ) {
        let Some(view) = data.ohlc(s.series, s.lod_level) else {
            return;
        };
        let Some(style) = frame.styles.get(s.style.0 as usize).copied() else {
            return;
        };
        self.clips
            .push(self.clip().map_or(IBox::of_rect(s.clip), |c| {
                c.intersect(IBox::of_rect(s.clip))
            }));
        let m = bar_metrics(s.x_transform.step);
        let yt = &s.y_transform;
        let range = s.index_range;
        let level = view.level;
        let bar = |j: i64| {
            Some([
                view.open.at(view.first, j)?,
                view.high.at(view.first, j)?,
                view.low.at(view.first, j)?,
                view.close.at(view.first, j)?,
            ])
        };
        // Per-chunk reference price, exactly as the GPU backend chooses it.
        let mut chunk = (i64::MIN, None::<f64>);
        let mut chunk_ref = |j: i64| {
            let c = chunk_of(level, j);
            if chunk.0 != c {
                chunk = (c, initial_chunk_ref(&view, c));
            }
            chunk.1
        };
        match s.kind {
            SeriesKind::Candlestick | SeriesKind::HeikinAshi | SeriesKind::OhlcBar => {
                for j in range.start..range.end {
                    let (Some(ohlc), Some(r)) = (bar(j), chunk_ref(j)) else {
                        continue;
                    };
                    let rel = ohlc_rel(yt, ohlc, r);
                    if rel.iter().any(|&v| is_gap(v)) {
                        continue;
                    }
                    let bias = y_bias(yt, r);
                    let xc = x_center(&s.x_transform, j);
                    let ys = rel.map(|v| y(yt, v, bias));
                    let up = is_up_rel(rel[0], rel[3]);
                    if s.kind == SeriesKind::OhlcBar {
                        let color = if up { style.up } else { style.down };
                        for r in ohlc_bar(xc, m, ys).into_iter().flatten() {
                            self.fill_rect(pm, r, color);
                        }
                        continue;
                    }
                    let g = candle(xc, m, ys);
                    let wick = if up { style.up_wick } else { style.down_wick };
                    self.fill_rect(pm, g.wick, wick);
                    if let Some(body) = g.body {
                        let border = if up {
                            style.up_border
                        } else {
                            style.down_border
                        };
                        let fill = if up { style.up } else { style.down };
                        if m.show_border && border != fill {
                            self.fill_rect(pm, body, border);
                            let inner = RectPx {
                                x: body.x + 1.0,
                                y: body.y + 1.0,
                                w: body.w - 2.0,
                                h: body.h - 2.0,
                            };
                            if inner.w > 0.0 && inner.h > 0.0 {
                                self.fill_rect(pm, inner, fill);
                            }
                        } else {
                            self.fill_rect(pm, body, fill);
                        }
                    }
                }
            }
            SeriesKind::Volume => {
                let Some(volume) = view.volume else {
                    self.clips.pop();
                    return;
                };
                // Volumes are stored relative to 0; up/down uses the linear
                // OHLC buffer.
                let linear = YTransform {
                    mapping: PriceMapping::Linear,
                    ..*yt
                };
                let base = y_of_price(yt, 0.0);
                let bias = y_bias(yt, 0.0);
                for j in range.start..range.end {
                    let (Some(ohlc), Some(v), Some(r)) =
                        (bar(j), volume.at(view.first, j), chunk_ref(j))
                    else {
                        continue;
                    };
                    let rel = ohlc_rel(&linear, ohlc, r);
                    // A gap bar of the source series has no volume bar.
                    if rel.iter().any(|&v| is_gap(v)) {
                        continue;
                    }
                    let xc = x_center(&s.x_transform, j);
                    let r = histogram(xc, m, y(yt, v_rel(yt, v, 0.0), bias), base);
                    let up = is_up_rel(rel[0], rel[3]);
                    self.fill_rect(pm, r, if up { style.up } else { style.down });
                }
            }
            SeriesKind::Histogram => {
                let base = y_of_price(yt, style.base_value);
                let colors = (level == 0).then(|| data.point_colors(s.series)).flatten();
                for j in range.start..range.end {
                    let (Some(v), Some(r)) = (bar(j), chunk_ref(j)) else {
                        continue;
                    };
                    let rel = ohlc_rel(yt, v, r);
                    let Some(yv) = histogram_value_y(yt, y_bias(yt, r), rel, base) else {
                        continue;
                    };
                    let xc = x_center(&s.x_transform, j);
                    let color = point_color(colors, j).unwrap_or(style.up);
                    self.fill_rect(pm, histogram(xc, m, yv, base), color);
                }
            }
            SeriesKind::Line | SeriesKind::Area | SeriesKind::Baseline => {
                self.line_series(pm, s, &view, style);
            }
            SeriesKind::Band => self.band_series(pm, s, &view, data, style),
            SeriesKind::Points => {
                // A dot at the (last) value of each element, with the
                // coverage of a circle icon (shader: `fs_points`).
                let size = style.line_width.0;
                let reach = size * 0.5 + 1.0;
                for j in range.start..range.end {
                    let (Some(v), Some(r)) = (bar(j), chunk_ref(j)) else {
                        continue;
                    };
                    let last = ohlc_rel(yt, v, r)[3];
                    if is_gap(last) {
                        continue;
                    }
                    let xc = x_center(&s.x_transform, j);
                    let yc = y(yt, last, y_bias(yt, r));
                    let bbox = (xc - reach, yc - reach, xc + reach, yc + reach);
                    self.shade_box(pm, bbox, |p| {
                        let c = icon_coverage(ICON_CIRCLE, p.x - xc, p.y - yc, size);
                        Some((style.up, c))
                    });
                }
            }
            // Custom, Kagi and Point & Figure series arrive as primitives,
            // never as a series draw.
            SeriesKind::Custom | SeriesKind::Kagi | SeriesKind::PointFigure => {}
        }
        self.clips.pop();
    }

    /// Blends `shade(p)` (color and coverage) into every pixel whose center
    /// lies in `[x0, x1] × [y0, y1]`, within the clip, in row-major order.
    fn shade_box(
        &self,
        pm: &mut Pixmap,
        (x0, y0, x1, y1): (f32, f32, f32, f32),
        shade: impl Fn(PointPx) -> Option<(Rgba, f32)>,
    ) {
        let Some(clip) = self.clip() else {
            return;
        };
        let b = IBox {
            x0: x0.floor() as i32,
            y0: y0.floor() as i32,
            x1: x1.ceil() as i32,
            y1: y1.ceil() as i32,
        }
        .intersect(clip);
        for py in b.y0..b.y1 {
            for px in b.x0..b.x1 {
                let p = PointPx {
                    x: px as f32 + 0.5,
                    y: py as f32 + 0.5,
                };
                if p.x < x0 || p.x > x1 || p.y < y0 || p.y > y1 {
                    continue;
                }
                if let Some((color, cov)) = shade(p) {
                    let c8 = coverage_8(cov);
                    if c8 > 0 {
                        Self::blend(pm, px, py, color, c8);
                    }
                }
            }
        }
    }

    /// A band between `upper` (the draw's series) and its pair: per element,
    /// the area between the two edges `last(j)` → `first(j + 1)`, with the
    /// coverage the shader uses (`fs_band`).
    fn band_series(
        &self,
        pm: &mut Pixmap,
        s: &SeriesDraw,
        upper: &OhlcLevelView<'_>,
        data: ChartDataView<'_>,
        style: SeriesStyle,
    ) {
        let Some(lower) = s.pair.and_then(|p| data.ohlc(p, s.lod_level)) else {
            return;
        };
        let yt = &s.y_transform;
        let level = upper.level;
        let bar = |v: &OhlcLevelView<'_>, j: i64| {
            let ohlc = || {
                Some([
                    v.open.at(v.first, j)?,
                    v.high.at(v.first, j)?,
                    v.low.at(v.first, j)?,
                    v.close.at(v.first, j)?,
                ])
            };
            ohlc().unwrap_or([f64::NAN; 4])
        };
        let (mut chunk_u, mut chunk_l) = ((i64::MIN, None::<f64>), (i64::MIN, None::<f64>));
        for j in s.index_range.start..s.index_range.end {
            let c = chunk_of(level, j);
            if chunk_u.0 != c {
                chunk_u = (c, initial_chunk_ref(upper, c));
                chunk_l = (c, initial_chunk_ref(&lower, c));
            }
            let (Some(ru), Some(rl)) = (chunk_u.1, chunk_l.1) else {
                continue;
            };
            let u0 = ohlc_rel(yt, bar(upper, j), ru)[3];
            let u1 = ohlc_rel(yt, bar(upper, j + 1), ru)[0];
            let l0 = ohlc_rel(yt, bar(&lower, j), rl)[3];
            let l1 = ohlc_rel(yt, bar(&lower, j + 1), rl)[0];
            if is_gap(u0) || is_gap(u1) || is_gap(l0) || is_gap(l1) {
                continue;
            }
            let (bu, bl) = (y_bias(yt, ru), y_bias(yt, rl));
            let xj = x_center(&s.x_transform, j);
            let xn = x_center(&s.x_transform, j + 1);
            let a = PointPx {
                x: xj,
                y: y(yt, u0, bu),
            };
            let b = PointPx {
                x: xn,
                y: y(yt, u1, bu),
            };
            let (la, lb) = (y(yt, l0, bl), y(yt, l1, bl));
            let bbox = (
                xj - 1.0,
                a.y.min(b.y).min(la).min(lb) - 1.0,
                xn + 1.0,
                a.y.max(b.y).max(la).max(lb) + 1.0,
            );
            self.shade_box(pm, bbox, |p| {
                let (coverage, crossed) = band_coverage(p, a, b, la, lb, yt.a);
                let color = if crossed {
                    style.fill_bottom
                } else {
                    style.fill_top
                };
                Some((color, coverage))
            });
        }
    }

    /// Line, area and baseline series: fills first (area, baseline), then the
    /// line, element by element, with the coverage functions the shaders use.
    fn line_series(
        &self,
        pm: &mut Pixmap,
        s: &SeriesDraw,
        view: &OhlcLevelView<'_>,
        style: SeriesStyle,
    ) {
        let yt = &s.y_transform;
        let level = view.level;
        let bar = |j: i64| {
            Some([
                view.open.at(view.first, j)?,
                view.high.at(view.first, j)?,
                view.low.at(view.first, j)?,
                view.close.at(view.first, j)?,
            ])
        };
        let mut chunk = (i64::MIN, None::<f64>);
        // (bias, xj, xn, cur, next) of element j.
        let mut elements = Vec::new();
        for j in s.index_range.start..s.index_range.end {
            let c = chunk_of(level, j);
            if chunk.0 != c {
                chunk = (c, initial_chunk_ref(view, c));
            }
            let Some(r) = chunk.1 else { continue };
            let gap = [f64::NAN; 4];
            let cur = ohlc_rel(yt, bar(j).unwrap_or(gap), r);
            let next = ohlc_rel(yt, bar(j + 1).unwrap_or(gap), r);
            let xj = x_center(&s.x_transform, j);
            let xn = x_center(&s.x_transform, j + 1);
            elements.push((y_bias(yt, r), xj, xn, cur, next));
        }
        let clip_top = s.clip.y;
        let clip_bottom = s.clip.y + s.clip.h;
        if s.kind != SeriesKind::Line {
            let base_y = if s.kind == SeriesKind::Area {
                clip_bottom
            } else {
                y_of_price(yt, style.base_value)
            };
            for &(bias, xj, xn, cur, next) in &elements {
                let Some((a, b)) = fill_edge(yt, bias, xj, xn, cur, next, style.step) else {
                    continue;
                };
                let bbox = (
                    a.x - 1.0,
                    a.y.min(b.y).min(base_y) - 1.0,
                    b.x + 1.0,
                    a.y.max(b.y).max(base_y) + 1.0,
                );
                self.shade_box(pm, bbox, |p| {
                    let color = if s.kind == SeriesKind::Area {
                        mix(
                            style.fill_top,
                            style.fill_bottom,
                            gradient_t(Px(p.y), Px(clip_top), Px(clip_bottom)),
                        )
                    } else if p.y < base_y {
                        style.fill_top
                    } else {
                        style.fill_bottom
                    };
                    Some((color, fill_coverage(p, a, b, Px(base_y))))
                });
            }
        }
        let base_y = y_of_price(yt, style.base_value);
        let hw = style.line_width.0 * 0.5;
        for &(bias, xj, xn, cur, next) in &elements {
            for (a, b) in line_segments(yt, bias, xj, xn, cur, next, style.step)
                .into_iter()
                .flatten()
            {
                let bbox = (
                    a.x.min(b.x) - hw - 1.0,
                    a.y.min(b.y) - hw - 1.0,
                    a.x.max(b.x) + hw + 1.0,
                    a.y.max(b.y) + hw + 1.0,
                );
                self.shade_box(pm, bbox, |p| {
                    if !dash_on(Px(p.x), style.line_width, style.line_dash) {
                        return None;
                    }
                    let color = if s.kind == SeriesKind::Baseline && p.y >= base_y {
                        style.down
                    } else {
                        style.up
                    };
                    Some((color, segment_coverage(p, a, b, Px(hw))))
                });
            }
        }
    }
}

/// Per-point color of element `j`; `None` for "use the series color".
fn point_color(colors: Option<(i64, &[u32])>, j: i64) -> Option<Rgba> {
    let (first, colors) = colors?;
    let c = *colors.get(usize::try_from(j - first).ok()?)?;
    (c & 0xFF != 0).then(|| Rgba::from_u32(c))
}

/// Linear mix of two colors, rounded half up to 8 bits. Shader: `mix8()`.
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |x: u8, y: u8| {
        let (x, y) = (f32::from(x), f32::from(y));
        (x + (y - x) * t + 0.5).floor().clamp(0.0, 255.0) as u8
    };
    Rgba {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: m(a.a, b.a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_is_source_over() {
        let mut pm = Pixmap::new(1, 1).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
        CpuRenderer::blend(&mut pm, 0, 0, Rgba::hex(0x00FF_FFFF).with_alpha(128), 255);
        assert_eq!(pm.data(), &[128, 128, 128, 255]);
        CpuRenderer::blend(&mut pm, 0, 0, Rgba::hex(0x00FF_0000), 255);
        assert_eq!(pm.data(), &[255, 0, 0, 255]);
    }

    #[test]
    fn invalid_size_is_an_error() {
        let mut r = CpuRenderer::new();
        assert_eq!(
            r.ensure_target(SizePx { w: 0.0, h: 10.0 }),
            Err(RenderError::InvalidSize)
        );
        assert_eq!(r.encode_png(), Err(RenderError::InvalidSize));
    }
}
