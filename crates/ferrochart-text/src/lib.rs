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
//! Characters are looked up along a fallback chain (r21): the requested font,
//! the bundled font, then the fonts added with
//! [`TextEngine::add_fallback_font`] in the order they were added. A character
//! none of them has is drawn by the [`GlyphSource`] when one is set (the
//! browser on the web, color emoji included), otherwise as `□` (U+25A1 of the
//! bundled font) instead of being dropped.
//!
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

use ab_glyph::{Font, FontArc, FontVec, GlyphId, PxScale, ScaleFont, point};

/// The bundled font: JetBrains Mono Regular, subset to ASCII and
/// `\u{a0} ° ± · × ÷ – — … ₀-₉ ← ↑ → ↓ − ≈ ≠ ≤ ≥ □ ▲ ▼`.
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

/// A rasterized glyph: 8-bit coverage, row-major, `width × height`; color
/// glyphs (emoji) also carry their pixels.
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
    /// For color glyphs: straight (not premultiplied) RGBA8, row-major,
    /// drawn as is instead of in the text color (the text color's alpha
    /// still applies). `coverage` is then its alpha channel.
    pub rgba: Option<Vec<u8>>,
}

/// Drawn for characters that no font in the chain has.
pub const MISSING_GLYPH: char = '\u{25A1}';

/// [`GlyphKey::font`] of glyphs drawn by the [`GlyphSource`].
pub const SOURCE_FONT: u16 = u16::MAX;

/// A glyph found along the fallback chain: the font it comes from and its
/// glyph id there. Two characters with the same key draw the same bitmap, so
/// it is the key for glyph caches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// Index of the font holding the glyph, or [`SOURCE_FONT`].
    pub font: u16,
    /// Glyph id in that font; the character (`char as u32`) for
    /// [`SOURCE_FONT`].
    pub glyph: u32,
}

/// Draws the characters no loaded font has: the last link of the fallback
/// chain, before `□`. The web build sets one that rasterizes with the
/// browser (`text-browser`), so system CJK fonts and color emoji work
/// without shipping fonts.
///
/// Both methods take the size in pixels. Renderers cache the bitmaps but not
/// the advances, so `advance` should be cheap.
pub trait GlyphSource: Send + Sync {
    /// Advance of `c` at `size` px.
    fn advance(&self, c: char, size: f32) -> f32;

    /// The bitmap of `c` at `size` px, positioned like
    /// [`TextEngine::rasterize`]'s; `None` when it has no pixels. A color
    /// glyph sets [`GlyphBitmap::rgba`].
    fn rasterize(&self, c: char, size: f32) -> Option<GlyphBitmap>;
}

/// Characters drawn as nothing, with no advance: zero-width spaces and
/// joiners, word joiners, variation selectors (emoji presentation) and the
/// byte-order mark. They would otherwise turn into `□` inside emoji
/// sequences.
fn is_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{E0100}'..='\u{E01EF}'
    )
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
    /// Indices into `fonts`, in lookup order after the bundled font.
    fallbacks: Vec<u16>,
    source: Option<Box<dyn GlyphSource>>,
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
            .field("fallbacks", &self.fallbacks)
            .field("source", &self.source.is_some())
            .finish()
    }
}

impl TextEngine {
    /// An engine with the bundled font as font 0.
    pub fn new() -> Self {
        let fonts = ab_glyph::FontRef::try_from_slice(BUNDLED_FONT)
            .map(|f| vec![LoadedFont::new(FontArc::new(f))])
            .unwrap_or_default();
        Self {
            fonts,
            fallbacks: Vec::new(),
            source: None,
        }
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

    /// Adds a fallback font (TTF/OTF data) and returns its index. Characters
    /// missing from the requested font and from the bundled font are looked
    /// up in the fallback fonts, in the order they were added.
    ///
    /// Color emoji fonts (sbix, COLR, CBDT) have no outlines `ab_glyph` can
    /// draw; an outline emoji font such as Noto Emoji draws in the text color.
    ///
    /// # Errors
    ///
    /// As [`TextEngine::add_font`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ferrochart_text::TextEngine;
    ///
    /// let mut engine = TextEngine::new();
    /// let cjk = std::fs::read("/System/Library/Fonts/Hiragino Sans GB.ttc")?;
    /// engine.add_fallback_font(cjk)?;
    /// assert!(engine.has_glyph(0, '中'));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_fallback_font(&mut self, data: Vec<u8>) -> Result<u16, FontError> {
        let id = self.add_font(data)?;
        self.fallbacks.push(id);
        Ok(id)
    }

    /// Sets (or removes) the [`GlyphSource`] that draws characters no loaded
    /// font has. Without one they are drawn as `□`.
    ///
    /// Renderers cache glyph bitmaps; set the source before the first frame.
    pub fn set_glyph_source(&mut self, source: Option<Box<dyn GlyphSource>>) {
        self.source = source;
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

    /// Looks `c` up along the chain: font `id` (unknown ids use font 0), the
    /// bundled font, the fallback fonts. `None` when no font has it.
    fn find(&self, id: u16, c: char) -> Option<GlyphKey> {
        let first = if usize::from(id) < self.fonts.len() {
            id
        } else {
            0
        };
        [first, 0]
            .into_iter()
            .chain(self.fallbacks.iter().copied())
            .find_map(|font| {
                let GlyphId(glyph) = self.fonts.get(usize::from(font))?.font.glyph_id(c);
                (glyph != 0).then_some(GlyphKey {
                    font,
                    glyph: u32::from(glyph),
                })
            })
    }

    /// The glyph drawn for `c` with font `id`: found along the fallback
    /// chain, else from the [`GlyphSource`], else `□` of the bundled font.
    /// `None` for ignorable characters (zero-width joiners, variation
    /// selectors) and without fonts.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_text::{TextEngine, MISSING_GLYPH};
    ///
    /// let engine = TextEngine::new();
    /// // No CJK font loaded: drawn as the missing-glyph box.
    /// assert_eq!(engine.glyph_key(0, '中'), engine.glyph_key(0, MISSING_GLYPH));
    /// assert_ne!(engine.glyph_key(0, 'A'), engine.glyph_key(0, MISSING_GLYPH));
    /// assert_eq!(engine.glyph_key(0, '\u{FE0F}'), None);
    /// ```
    pub fn glyph_key(&self, id: u16, c: char) -> Option<GlyphKey> {
        if is_ignorable(c) {
            return None;
        }
        self.find(id, c).or_else(|| match self.source {
            Some(_) => Some(GlyphKey {
                font: SOURCE_FONT,
                glyph: u32::from(c),
            }),
            None => self.find(0, MISSING_GLYPH),
        })
    }

    /// Whether some loaded font in the chain of font `id` has `c` (it is not
    /// left to the [`GlyphSource`] or drawn as `□`).
    pub fn has_glyph(&self, id: u16, c: char) -> bool {
        self.find(id, c).is_some()
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

    /// Advance of `c` at `size` px, from the font the fallback chain picks.
    pub fn advance(&self, id: u16, c: char, size: f32) -> f32 {
        let Some(f) = self.font(id) else {
            return 0.0;
        };
        if c.is_ascii() {
            return f.advances[usize::from(c as u8)] * size;
        }
        if let Some(d) = subscript_digit(c)
            && f.subscripts[d] > 0.0
        {
            return f.subscripts[d] * size;
        }
        let Some(key) = self.glyph_key(id, c) else {
            return 0.0;
        };
        if key.font == SOURCE_FONT {
            return self.source.as_ref().map_or(0.0, |s| s.advance(c, size));
        }
        let (Some(f), Ok(glyph)) = (
            self.fonts.get(usize::from(key.font)),
            u16::try_from(key.glyph),
        ) else {
            return 0.0;
        };
        f.font
            .as_scaled(em_scale(&f.font, size))
            .h_advance(GlyphId(glyph))
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

    /// Rasterizes `c` at `size` px with the pen on an integer pixel, from the
    /// font the fallback chain picks. `None` for glyphs without an outline
    /// (space) and ignorable characters.
    pub fn rasterize(&self, id: u16, c: char, size: f32) -> Option<GlyphBitmap> {
        let key = self.glyph_key(id, c)?;
        self.rasterize_key(key, size)
    }

    /// Rasterizes the glyph `key` (from [`TextEngine::glyph_key`]) at `size`
    /// px. `None` for glyphs without an outline.
    pub fn rasterize_key(&self, key: GlyphKey, size: f32) -> Option<GlyphBitmap> {
        if key.font == SOURCE_FONT {
            return self
                .source
                .as_ref()?
                .rasterize(char::from_u32(key.glyph)?, size);
        }
        let f = self.fonts.get(usize::from(key.font))?;
        let glyph = GlyphId(u16::try_from(key.glyph).ok()?)
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
            rgba: None,
        })
    }

    /// The glyph id of `c` in font `id` itself, 0 when it lacks it (no
    /// fallback; caches use [`TextEngine::glyph_key`]).
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
    map: HashMap<(GlyphKey, u32), Option<GlyphBitmap>>,
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
        let key = engine.glyph_key(id, c)?;
        self.map
            .entry((key, size.to_bits()))
            .or_insert_with(|| engine.rasterize_key(key, size))
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
        for c in "\u{a0}°±·×÷–—…₀₁₂₃₄₅₆₇₈₉←↑→↓−≈≠≤≥□▲▼".chars()
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

    #[test]
    fn missing_characters_draw_a_box() {
        let e = TextEngine::new();
        let tofu = e.rasterize(0, MISSING_GLYPH, 16.0).unwrap();
        assert_eq!(e.rasterize(0, '中', 16.0).unwrap(), tofu);
        assert_eq!(e.rasterize(0, '🚀', 16.0).unwrap(), tofu);
        assert!(!e.has_glyph(0, '中') && e.has_glyph(0, MISSING_GLYPH));
        // 0.6 em, like every other bundled glyph.
        assert!((e.advance(0, '中', 10.0) - 6.0).abs() < 1e-4);
        assert!((e.width(0, "a中b", 10.0) - 18.0).abs() < 1e-4);
        // Variation selectors and joiners take no room.
        assert!(e.advance(0, '\u{FE0F}', 10.0).abs() < f32::EPSILON);
        assert!(e.rasterize(0, '\u{200D}', 10.0).is_none());
    }

    #[test]
    fn fallback_fonts_are_searched_in_order() {
        let mut e = TextEngine::new();
        // The bundled font again, as a fallback: it has nothing new, so
        // ASCII still comes from font 0 and '中' is still the box.
        let id = e.add_fallback_font(BUNDLED_FONT.to_vec()).unwrap();
        assert_eq!(id, 1);
        assert_eq!(e.glyph_key(0, 'A').map(|k| k.font), Some(0));
        assert_eq!(e.glyph_key(1, 'A').map(|k| k.font), Some(1));
        assert_eq!(e.glyph_key(0, '中').map(|k| k.font), Some(0));
        assert_eq!(e.add_fallback_font(vec![1, 2, 3]), Err(FontError::Invalid));
        assert_eq!(e.font_count(), 2);
    }

    /// Draws every character as a 4×2 block: red on the left, blue on the
    /// right, 5 px wide.
    struct Blocks;

    impl GlyphSource for Blocks {
        fn advance(&self, _: char, _: f32) -> f32 {
            5.0
        }

        fn rasterize(&self, _: char, _: f32) -> Option<GlyphBitmap> {
            let px = [
                [255, 0, 0, 255],
                [255, 0, 0, 255],
                [0, 0, 255, 255],
                [0, 0, 255, 128],
            ];
            let rgba: Vec<u8> = px.iter().chain(px.iter()).flatten().copied().collect();
            Some(GlyphBitmap {
                width: 4,
                height: 2,
                left: 0,
                top: -2,
                coverage: rgba.chunks_exact(4).map(|p| p[3]).collect(),
                rgba: Some(rgba),
            })
        }
    }

    #[test]
    fn the_source_comes_after_the_fonts() {
        let mut e = TextEngine::new();
        e.set_glyph_source(Some(Box::new(Blocks)));
        let key = e.glyph_key(0, '😀').unwrap();
        assert_eq!(
            key,
            GlyphKey {
                font: SOURCE_FONT,
                glyph: 0x1F600
            }
        );
        assert!(!e.has_glyph(0, '😀'));
        assert!((e.advance(0, '😀', 12.0) - 5.0).abs() < 1e-6);
        assert!(e.rasterize(0, '😀', 12.0).unwrap().rgba.is_some());
        // Fonts first: ASCII and the box itself still come from font 0.
        assert_eq!(e.glyph_key(0, 'A').map(|k| k.font), Some(0));
        assert_eq!(e.glyph_key(0, MISSING_GLYPH).map(|k| k.font), Some(0));
        // Ignorable characters stay empty.
        assert_eq!(e.glyph_key(0, '\u{FE0F}'), None);
        e.set_glyph_source(None);
        assert_eq!(e.glyph_key(0, '😀'), e.glyph_key(0, MISSING_GLYPH));
    }

    #[test]
    fn cache_keys_follow_the_chain() {
        let e = TextEngine::new();
        let mut cache = GlyphCache::default();
        cache.get(&e, 0, '中', 12.0);
        cache.get(&e, 0, '文', 12.0);
        cache.get(&e, 0, MISSING_GLYPH, 12.0);
        // All three are the same box.
        assert_eq!(cache.len(), 1);
    }
}
