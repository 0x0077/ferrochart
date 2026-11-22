//! Font loading, text measurement and glyph rasterization shared by the FerroChart
//! backends.
//!
//! The crate does not depend on `ferrochart-core`: backends adapt [`TextEngine`]
//! to the core's `TextMeasurer` trait. All metrics are reported both
//! em-normalized (per 1 px of font size, as the core's ASCII fast path expects)
//! and in pixels for a given size.
//!
//! A subset of JetBrains Mono Regular (SIL Open Font License 1.1, ASCII plus a few
//! symbols, about 11 KB) is bundled and always available as font 0.
//!
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

use ab_glyph::{Font, FontArc, FontVec, GlyphId, PxScale, ScaleFont, point};

/// The bundled font: JetBrains Mono Regular, subset to ASCII and
/// `\u{a0} ° ± · × ÷ – — … ₀-₉ ← ↑ → ↓ − ≈ ≠ ≤ ≥ ▲ ▼`.
/// License: `assets/fonts/OFL.txt`.
pub const BUNDLED_FONT: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular-subset.ttf");

/// Error from loading a font.
///
/// # Examples
///
/// ```
/// use ferrochart_text::{FontError, TextEngine};
///
/// let mut engine = TextEngine::new();
/// assert_eq!(engine.add_font(vec![0, 1, 2]), Err(FontError::Invalid));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontError {
    /// The data is not a font `ab_glyph` can read.
    Invalid,
    /// More than `u16::MAX` fonts.
    TooManyFonts,
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid font data"),
            Self::TooManyFonts => f.write_str("too many fonts"),
        }
    }
}

impl std::error::Error for FontError {}

/// Vertical metrics of a font at some size (pixels, y down: `descent` is
/// positive below the baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    /// Distance from the top of the line box to the baseline.
    pub ascent: f32,
    /// Distance from the baseline to the bottom of the line box.
    pub descent: f32,
    /// Extra space between lines.
    pub line_gap: f32,
}

impl LineMetrics {
    /// Height of a line box: ascent + descent + line gap.
    pub fn height(self) -> f32 {
        self.ascent + self.descent + self.line_gap
    }

    fn scaled(self, size: f32) -> Self {
        Self {
            ascent: self.ascent * size,
            descent: self.descent * size,
            line_gap: self.line_gap * size,
        }
    }
}

struct LoadedFont {
    font: FontArc,
    /// Advances of ASCII characters at a font size of 1 px.
    advances: [f32; 128],
    /// Advances of the subscript digits U+2080..=U+2089 at 1 px.
    subscripts: [f32; 10],
    /// Line metrics at a font size of 1 px.
    metrics: LineMetrics,
}

impl LoadedFont {
    fn new(font: FontArc) -> Self {
        // `PxScale` is the line height (ascent - descent), not the em size. Scaling
        // by `units_per_em / height` gives an em size of exactly 1 px.
        let em = em_scale(&font, 1.0);
        let scaled = font.as_scaled(em);
        let mut advances = [0.0; 128];
        for (c, adv) in (0_u8..128).zip(advances.iter_mut()) {
            *adv = scaled.h_advance(font.glyph_id(char::from(c)));
        }
        let mut subscripts = [0.0; 10];
        for (c, adv) in ('\u{2080}'..='\u{2089}').zip(subscripts.iter_mut()) {
            *adv = scaled.h_advance(font.glyph_id(c));
        }
        let metrics = LineMetrics {
            ascent: scaled.ascent(),
            descent: -scaled.descent(),
            line_gap: scaled.line_gap(),
        };
        Self {
            font,
            advances,
            subscripts,
            metrics,
        }
    }
}

/// `0..=9` for the subscript digits `₀`..=`₉`.
fn subscript_digit(c: char) -> Option<usize> {
    ('\u{2080}'..='\u{2089}')
        .contains(&c)
        .then(|| c as usize - 0x2080)
}

/// The `PxScale` that gives an em size of `size` pixels.
fn em_scale(font: &FontArc, size: f32) -> PxScale {
    let upem = font.units_per_em().unwrap_or(1000.0);
    let height = font.height_unscaled();
    if height > 0.0 {
        PxScale::from(size * height / upem)
    } else {
        PxScale::from(size)
    }
}

/// A rasterized glyph: 8-bit coverage, row-major, `width × height`.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphBitmap {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Offset of the bitmap's left edge from the pen position.
    pub left: i32,
    /// Offset of the bitmap's top edge from the baseline (negative = above).
    pub top: i32,
    /// Coverage, 0..=255.
    pub coverage: Vec<u8>,
}

/// Fonts and metrics.
///
/// # Examples
///
/// ```
/// use ferrochart_text::TextEngine;
///
/// let engine = TextEngine::new();
/// // The bundled font is monospaced: every digit has the same advance.
/// let adv = engine.ascii_advances(0);
/// assert!(adv[b'0' as usize] > 0.0);
/// assert_eq!(adv[b'1' as usize], adv[b'8' as usize]);
/// assert!(engine.line_metrics(0, 12.0).height() > 12.0);
/// ```
pub struct TextEngine {
    fonts: Vec<LoadedFont>,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TextEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextEngine")
            .field("fonts", &self.fonts.len())
            .finish()
    }
}

impl TextEngine {
    /// An engine with the bundled font as font 0.
    pub fn new() -> Self {
        let fonts = ab_glyph::FontRef::try_from_slice(BUNDLED_FONT)
            .map(|f| vec![LoadedFont::new(FontArc::new(f))])
            .unwrap_or_default();
        Self { fonts }
    }

    /// Adds a font (TTF/OTF data) and returns its index.
    ///
    /// # Errors
    ///
    /// [`FontError::Invalid`] if the data cannot be parsed,
    /// [`FontError::TooManyFonts`] beyond `u16::MAX` fonts.
    pub fn add_font(&mut self, data: Vec<u8>) -> Result<u16, FontError> {
        let id = u16::try_from(self.fonts.len()).map_err(|_| FontError::TooManyFonts)?;
        let font = FontVec::try_from_vec(data).map_err(|_| FontError::Invalid)?;
        self.fonts.push(LoadedFont::new(FontArc::new(font)));
        Ok(id)
    }

    /// Number of loaded fonts.
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Font `id`, falling back to font 0 for unknown ids.
    fn font(&self, id: u16) -> Option<&LoadedFont> {
        self.fonts
            .get(usize::from(id))
            .or_else(|| self.fonts.first())
    }

    /// ASCII advances of font `id` at a font size of 1 px. Unknown ids use font 0.
    pub fn ascii_advances(&self, id: u16) -> &[f32; 128] {
        static ZERO: [f32; 128] = [0.0; 128];
        self.font(id).map_or(&ZERO, |f| &f.advances)
    }

    /// Advances of the subscript digits `₀`..=`₉` (U+2080..=U+2089) of font
    /// `id` at a font size of 1 px. Unknown ids use font 0.
    pub fn subscript_advances(&self, id: u16) -> &[f32; 10] {
        static ZERO: [f32; 10] = [0.0; 10];
        self.font(id).map_or(&ZERO, |f| &f.subscripts)
    }

    /// Line metrics of font `id` at `size` px.
    pub fn line_metrics(&self, id: u16, size: f32) -> LineMetrics {
        self.font(id).map_or(
            LineMetrics {
                ascent: size,
                descent: 0.0,
                line_gap: 0.0,
            },
            |f| f.metrics.scaled(size),
        )
    }

    /// Advance of `c` at `size` px.
    pub fn advance(&self, id: u16, c: char, size: f32) -> f32 {
        let Some(f) = self.font(id) else {
            return 0.0;
        };
        if c.is_ascii() {
            return f.advances[usize::from(c as u8)] * size;
        }
        if let Some(d) = subscript_digit(c) {
            return f.subscripts[d] * size;
        }
        let scaled = f.font.as_scaled(em_scale(&f.font, size));
        scaled.h_advance(f.font.glyph_id(c))
    }

    /// Width of `text` at `size` px: the sum of the advances, without kerning,
    /// exactly as the core's ASCII fast path measures it.
    pub fn width(&self, id: u16, text: &str, size: f32) -> f32 {
        if text.is_ascii() {
            let table = self.ascii_advances(id);
            return text.bytes().map(|b| table[usize::from(b)]).sum::<f32>() * size;
        }
        text.chars().map(|c| self.advance(id, c, size)).sum()
    }

    /// Rasterizes `c` at `size` px with the pen on an integer pixel. `None` for
    /// glyphs without an outline (space).
    pub fn rasterize(&self, id: u16, c: char, size: f32) -> Option<GlyphBitmap> {
        let f = self.font(id)?;
        let glyph = f
            .font
            .glyph_id(c)
            .with_scale_and_position(em_scale(&f.font, size), point(0.0, 0.0));
        let outlined = f.font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();
        let width = bounds.width() as u32;
        let height = bounds.height() as u32;
        let mut coverage = vec![0_u8; (width as usize) * (height as usize)];
        outlined.draw(|x, y, v| {
            let i = (y as usize) * (width as usize) + x as usize;
            if let Some(px) = coverage.get_mut(i) {
                *px = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        });
        Some(GlyphBitmap {
            width,
            height,
            left: bounds.min.x as i32,
            top: bounds.min.y as i32,
            coverage,
        })
    }

    /// The glyph id of `c` (for cache keys).
    pub fn glyph_id(&self, id: u16, c: char) -> u16 {
        self.font(id).map_or(0, |f| {
            let GlyphId(g) = f.font.glyph_id(c);
            g
        })
    }
}

/// Cache of rasterized glyphs, keyed by font, glyph and size.
///
/// # Examples
///
/// ```
/// use ferrochart_text::{GlyphCache, TextEngine};
///
/// let engine = TextEngine::new();
/// let mut cache = GlyphCache::default();
/// let w = cache.get(&engine, 0, 'A', 12.0).map(|g| g.width);
/// assert!(w.is_some_and(|w| w > 0));
/// assert!(cache.get(&engine, 0, ' ', 12.0).is_none());
/// assert_eq!(cache.len(), 2);
/// ```
#[derive(Debug, Default)]
pub struct GlyphCache {
    map: HashMap<(u16, u16, u32), Option<GlyphBitmap>>,
}

impl GlyphCache {
    /// The bitmap of `c`, rasterizing it on first use.
    pub fn get(
        &mut self,
        engine: &TextEngine,
        id: u16,
        c: char,
        size: f32,
    ) -> Option<&GlyphBitmap> {
        let key = (id, engine.glyph_id(id, c), size.to_bits());
        self.map
            .entry(key)
            .or_insert_with(|| engine.rasterize(id, c, size))
            .as_ref()
    }

    /// Number of cached entries (including empty glyphs).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Drops all cached glyphs.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_font_loads() {
        let e = TextEngine::new();
        assert_eq!(e.font_count(), 1);
        // JetBrains Mono advances are 600/1000 em.
        assert!((e.ascii_advances(0)[usize::from(b'5')] - 0.6).abs() < 1e-6);
        assert!(BUNDLED_FONT.len() < 100 * 1024);
    }

    #[test]
    fn bundled_font_has_the_listed_symbols() {
        let e = TextEngine::new();
        for c in "\u{a0}°±·×÷–—…₀₁₂₃₄₅₆₇₈₉←↑→↓−≈≠≤≥▲▼".chars()
        {
            assert_ne!(e.glyph_id(0, c), 0, "no glyph for {c:?}");
            if c != '\u{a0}' {
                assert!(e.rasterize(0, c, 16.0).is_some(), "{c:?} draws nothing");
            }
        }
        assert_eq!(e.glyph_id(0, '€'), 0);
    }

    #[test]
    fn width_matches_ascii_sum() {
        let e = TextEngine::new();
        let w = e.width(0, "123.45", 11.0);
        assert!((w - 6.0 * 0.6 * 11.0).abs() < 1e-3);
        // Unknown font ids fall back to font 0.
        assert!((e.width(7, "123.45", 11.0) - w).abs() < 1e-6);
    }

    #[test]
    fn glyphs_sit_on_the_baseline() {
        let e = TextEngine::new();
        let g = e.rasterize(0, 'H', 20.0).unwrap();
        let m = e.line_metrics(0, 20.0);
        assert!(g.top < 0 && -g.top as f32 <= m.ascent + 1.0);
        assert!(g.coverage.contains(&255));
    }
}
