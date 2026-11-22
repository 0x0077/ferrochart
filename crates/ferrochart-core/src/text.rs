//! Text measurement interface and allocation-free text helpers.
//!
//! The core needs text widths for axis sizing and label layout but must not depend
//! on a font library. Hosts pass a [`TextMeasurer`] (implemented on top of
//! `ferrochart-text` by the backends). ASCII text, which covers almost every axis
//! label, is measured by summing a per-font advance table: no callback per label,
//! no cache, no allocation.
//!

use core::fmt;

use crate::frame::FontStyleRef;
use crate::geom::SizePx;

/// Measures text for layout.
///
/// Advance tables are given **per 1 px of font size** (em-normalized), because the
/// core decides sizes at run time (theme size × device pixel ratio): the core
/// multiplies by `style.size` itself.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{FontId, FontStyleRef};
/// use ferrochart_core::geom::SizePx;
/// use ferrochart_core::text::{text_width, TextMeasurer};
///
/// struct Mono;
/// impl TextMeasurer for Mono {
///     fn ascii_advances(&self, _: FontStyleRef) -> &[f32; 128] { &[0.5; 128] }
///     fn measure(&self, text: &str, style: FontStyleRef) -> SizePx {
///         SizePx { w: text.chars().count() as f32 * 0.5 * style.size, h: style.size }
///     }
///     fn line_height(&self, style: FontStyleRef) -> f32 { style.size * 1.2 }
/// }
///
/// let style = FontStyleRef { font: FontId(0), size: 10.0 };
/// assert_eq!(text_width(&Mono, "1234", style), 20.0);
/// ```
pub trait TextMeasurer {
    /// Advance widths of ASCII characters 0..128 at a font size of 1 px.
    fn ascii_advances(&self, style: FontStyleRef) -> &[f32; 128];
    /// Advance widths of the subscript digits `₀`..=`₉` (U+2080..=U+2089) at
    /// a font size of 1 px. With a table, text made of ASCII and these digits
    /// (small prices with automatic precision) takes the fast path; `None`
    /// sends it to [`measure`](Self::measure).
    fn subscript_advances(&self, style: FontStyleRef) -> Option<&[f32; 10]> {
        let _ = style;
        None
    }
    /// Size of arbitrary (non-ASCII) text in physical pixels. Slow path.
    fn measure(&self, text: &str, style: FontStyleRef) -> SizePx;
    /// Height of a line box (ascent - descent + line gap) in physical pixels.
    fn line_height(&self, style: FontStyleRef) -> f32;
}

/// Width of `text` in physical pixels: the fast path for ASCII (and ASCII
/// with subscript digits, when the measurer has their table),
/// [`TextMeasurer::measure`] otherwise.
pub fn text_width(m: &dyn TextMeasurer, text: &str, style: FontStyleRef) -> f32 {
    let table = m.ascii_advances(style);
    if text.is_ascii() {
        return text
            .bytes()
            .map(|b| table[usize::from(b & 0x7F)])
            .sum::<f32>()
            * style.size;
    }
    if let Some(subs) = m.subscript_advances(style) {
        let mut em = 0.0;
        for c in text.chars() {
            em += match c {
                '\0'..='\x7F' => table[c as usize],
                '\u{2080}'..='\u{2089}' => subs[c as usize - 0x2080],
                _ => return m.measure(text, style).w,
            };
        }
        return em * style.size;
    }
    m.measure(text, style).w
}

/// A fixed-capacity string on the stack, for formatting labels without allocating.
/// Writes beyond the capacity fail with `fmt::Error`.
///
/// # Examples
///
/// ```
/// use core::fmt::Write;
/// use ferrochart_core::text::StackStr;
///
/// let mut s = StackStr::<16>::new();
/// write!(s, "{:.2}", 3.14159).unwrap();
/// assert_eq!(s.as_str(), "3.14");
/// ```
#[derive(Clone, Copy, Debug)]
pub struct StackStr<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> StackStr<N> {
    /// An empty string.
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    /// The contents.
    pub fn as_str(&self) -> &str {
        // Only whole `&str`s are ever copied in, so this is valid UTF-8.
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// Empties the string.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for StackStr<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> fmt::Write for StackStr<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > N {
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
