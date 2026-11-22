//! Coordinate, span and color types.
//!
//! Physical pixels (`Px`) and CSS (logical) pixels (`Css`) are distinct types and
//! are only converted through `to_px(dpr)` / `to_css(dpr)`. Coordinates are never
//! passed as bare `f32`. Everything here is `Copy` so it can be
//! used inside a `Frame`.
//!

/// A length or a single coordinate in physical pixels.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{Css, Px};
///
/// assert_eq!(Css(10.0).to_px(2.0), Px(20.0));
/// assert_eq!(Px(20.0).to_css(2.0), Css(10.0));
/// ```
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Default)]
pub struct Px(pub f32);

/// A length or a single coordinate in CSS (logical) pixels.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{Css, Px};
///
/// assert_eq!(Css(1.5).to_px(2.0), Px(3.0));
/// ```
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Css(pub f32);

impl Css {
    /// Converts to physical pixels at device pixel ratio `dpr`.
    pub fn to_px(self, dpr: f32) -> Px {
        Px(self.0 * dpr)
    }
}

impl Px {
    /// Converts to CSS pixels at device pixel ratio `dpr`.
    pub fn to_css(self, dpr: f32) -> Css {
        Css(self.0 / dpr)
    }
}

/// A point in physical pixels, origin at the top-left corner.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{PointCss, PointPx};
///
/// let p = PointCss { x: 10.0, y: 4.0 }.to_px(2.0);
/// assert_eq!(p, PointPx { x: 20.0, y: 8.0 });
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PointPx {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position (down is positive).
    pub y: f32,
}

/// A point in CSS pixels, origin at the top-left corner.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::PointCss;
///
/// assert_eq!(PointCss { x: 1.0, y: 2.0 }.to_px(1.0).x, 1.0);
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PointCss {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position (down is positive).
    pub y: f32,
}

impl PointCss {
    /// Converts to physical pixels at device pixel ratio `dpr`.
    pub fn to_px(self, dpr: f32) -> PointPx {
        PointPx {
            x: self.x * dpr,
            y: self.y * dpr,
        }
    }
}

impl PointPx {
    /// Converts to CSS pixels at device pixel ratio `dpr`.
    pub fn to_css(self, dpr: f32) -> PointCss {
        PointCss {
            x: self.x / dpr,
            y: self.y / dpr,
        }
    }
}

/// A size in physical pixels.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::{SizeCss, SizePx};
///
/// assert_eq!(SizeCss { w: 400.0, h: 300.0 }.to_px(2.0), SizePx { w: 800.0, h: 600.0 });
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SizePx {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

/// A size in CSS pixels.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
///
/// let s = SizeCss { w: 800.0, h: 450.0 };
/// assert_eq!(s.to_px(1.0).w, 800.0);
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SizeCss {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl SizeCss {
    /// Converts to physical pixels at device pixel ratio `dpr`.
    pub fn to_px(self, dpr: f32) -> SizePx {
        SizePx {
            w: self.w * dpr,
            h: self.h * dpr,
        }
    }
}

/// An axis-aligned rectangle in physical pixels.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::RectPx;
///
/// let r = RectPx { x: 10.0, y: 20.0, w: 30.0, h: 40.0 };
/// assert_eq!((r.right(), r.bottom()), (40.0, 60.0));
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct RectPx {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl RectPx {
    /// Right edge (`x + w`).
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge (`y + h`).
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// Whether `p` lies inside (left/top edges inclusive, right/bottom exclusive).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::geom::{PointPx, RectPx};
    ///
    /// let r = RectPx { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
    /// assert!(r.contains(PointPx { x: 0.0, y: 9.5 }));
    /// assert!(!r.contains(PointPx { x: 10.0, y: 5.0 }));
    /// ```
    pub fn contains(self, p: PointPx) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Intersection with `other`; an empty rectangle (zero size) if they do not overlap.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::geom::RectPx;
    ///
    /// let a = RectPx { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
    /// let b = RectPx { x: 5.0, y: 5.0, w: 10.0, h: 10.0 };
    /// assert_eq!(a.intersect(b), RectPx { x: 5.0, y: 5.0, w: 5.0, h: 5.0 });
    /// ```
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        Self {
            x,
            y,
            w: (r - x).max(0.0),
            h: (b - y).max(0.0),
        }
    }
}

/// A span inside one of the `Frame` pools: `start..start + len`.
///
/// Used instead of `Range<u32>` because `Range` is not `Copy`.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::Span;
///
/// let s = Span { start: 4, len: 3 };
/// assert_eq!(s.range(), 4..7);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Span {
    /// First element.
    pub start: u32,
    /// Number of elements.
    pub len: u32,
}

impl Span {
    /// The span as a `usize` range, for slicing a pool.
    pub fn range(self) -> core::ops::Range<usize> {
        let start = self.start as usize;
        start..start + self.len as usize
    }
}

/// An sRGB color with straight (not premultiplied) alpha.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::Rgba;
///
/// assert_eq!(Rgba::hex(0x26A6_9A), Rgba { r: 0x26, g: 0xA6, b: 0x9A, a: 255 });
/// assert_eq!(Rgba::hex(0x26A6_9A).with_alpha(128).a, 128);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 = opaque).
    pub a: u8,
}

impl Rgba {
    /// Opaque color from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xFF) as u8,
            g: ((rgb >> 8) & 0xFF) as u8,
            b: (rgb & 0xFF) as u8,
            a: 255,
        }
    }

    /// The same color with alpha `a`.
    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Color from `0xRRGGBBAA`, the per-point color format of
    /// [`ValueColumns::color`](crate::data::ValueColumns).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::geom::Rgba;
    ///
    /// assert_eq!(Rgba::from_u32(0x26A6_9A80), Rgba::hex(0x26A6_9A).with_alpha(0x80));
    /// ```
    pub const fn from_u32(rgba: u32) -> Self {
        Self {
            r: (rgba >> 24) as u8,
            g: ((rgba >> 16) & 0xFF) as u8,
            b: ((rgba >> 8) & 0xFF) as u8,
            a: (rgba & 0xFF) as u8,
        }
    }
}
