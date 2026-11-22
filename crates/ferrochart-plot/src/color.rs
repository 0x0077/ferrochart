//! Color scales (values to colors) and the default palette of marks.
//!

use ferrochart_core::geom::Rgba;
use ferrochart_core::theme::Theme;

/// How values spread over a color scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleKind {
    /// Low to high.
    Sequential,
    /// Symmetric around `center`: the middle stop is at `center`, and the
    /// farther end of the domain decides the extent of both sides.
    Diverging {
        /// The neutral value.
        center: f64,
    },
}

/// The value range of a color scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleDomain {
    /// From the mark's values, between the `quantile` and `1 − quantile`
    /// quantiles (outliers saturate).
    Auto {
        /// Fraction cut at each end, 0 to 0.5.
        quantile: f32,
    },
    /// A fixed range; values outside saturate.
    Fixed(f64, f64),
}

/// Maps values to colors by linear interpolation between stops in sRGB.
///
/// ```
/// use ferrochart_core::geom::Rgba;
/// use ferrochart_plot::ColorScale;
///
/// let s = ColorScale::new(vec![(0.0, Rgba::hex(0x000000)), (1.0, Rgba::hex(0xFFFFFF))]);
/// assert_eq!(s.color_at(0.5), Rgba::hex(0x808080));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ColorScale {
    /// Positions 0 to 1 (ascending) and colors.
    pub stops: Vec<(f32, Rgba)>,
    /// Sequential or diverging.
    pub kind: ScaleKind,
    /// Value range.
    pub domain: ScaleDomain,
}

impl ColorScale {
    /// A sequential scale over `stops` with an automatic domain.
    pub fn new(stops: Vec<(f32, Rgba)>) -> Self {
        Self {
            stops,
            kind: ScaleKind::Sequential,
            domain: ScaleDomain::Auto { quantile: 0.01 },
        }
    }

    /// The default sequential scale: dark blue to yellow.
    pub fn sequential() -> Self {
        Self::new(vec![
            (0.0, Rgba::hex(0x0D_1B4C)),
            (0.25, Rgba::hex(0x1F_4E9C)),
            (0.5, Rgba::hex(0x21_9EBC)),
            (0.75, Rgba::hex(0x6B_D08A)),
            (1.0, Rgba::hex(0xF4_E04D)),
        ])
    }

    /// The default diverging scale around `center`: the theme's down color,
    /// a neutral middle and the up color.
    pub fn diverging(center: f64, theme: &Theme) -> Self {
        let mid = mix(theme.background, theme.text, 0.18);
        Self {
            stops: vec![(0.0, theme.down), (0.5, mid), (1.0, theme.up)],
            kind: ScaleKind::Diverging { center },
            domain: ScaleDomain::Auto { quantile: 0.01 },
        }
    }

    /// The color at position `t` (clamped to 0..1).
    pub fn color_at(&self, t: f32) -> Rgba {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let Some(&(p0, c0)) = self.stops.first() else {
            return Rgba::hex(0x80_8080);
        };
        if t <= p0 {
            return c0;
        }
        for w in self.stops.windows(2) {
            let ((a, ca), (b, cb)) = (w[0], w[1]);
            if t <= b {
                let f = if b > a { (t - a) / (b - a) } else { 1.0 };
                return mix(ca, cb, f);
            }
        }
        self.stops.last().map_or(c0, |s| s.1)
    }
}

impl ScaleKind {
    /// Position (0 to 1) of value `v` for the resolved domain `lo..hi`.
    pub(crate) fn position(self, v: f64, (lo, hi): (f64, f64)) -> f32 {
        let t = match self {
            ScaleKind::Sequential => {
                if hi > lo {
                    (v - lo) / (hi - lo)
                } else {
                    0.5
                }
            }
            ScaleKind::Diverging { center } => {
                let m = (hi - center).abs().max((center - lo).abs());
                if m > 0.0 {
                    0.5 + (v - center) / (2.0 * m)
                } else {
                    0.5
                }
            }
        };
        t as f32
    }
}

/// The value range of `domain` for `values`. Called when data or options
/// change (it sorts a copy), not per frame; quantiles are taken over all
/// values, not only the visible ones.
pub(crate) fn resolve_domain(domain: ScaleDomain, values: &[f64]) -> Option<(f64, f64)> {
    match domain {
        ScaleDomain::Fixed(a, b) => Some((a.min(b), a.max(b))),
        ScaleDomain::Auto { quantile } => {
            let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
            if v.is_empty() {
                return None;
            }
            // The two order statistics by selection, O(n) instead of a sort.
            let q = f64::from(quantile.clamp(0.0, 0.5));
            let at = |f: f64| ((v.len() - 1) as f64 * f).round() as usize;
            let (lo_i, hi_i) = (at(q), at(1.0 - q));
            let hi = *v.select_nth_unstable_by(hi_i, f64::total_cmp).1;
            let lo = if lo_i < hi_i {
                *v[..hi_i].select_nth_unstable_by(lo_i, f64::total_cmp).1
            } else {
                hi
            };
            Some((lo, hi))
        }
    }
}

/// `a` mixed with `b`: `t = 0` is `a`, 1 is `b`.
pub(crate) fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba {
        r: l(a.r, b.r),
        g: l(a.g, b.g),
        b: l(a.b, b.b),
        a: l(a.a, b.a),
    }
}

/// Colors given to marks without a color, in order of creation; the first is
/// the theme's line color.
pub(crate) fn palette(theme: &Theme, k: usize) -> Rgba {
    const MORE: [u32; 7] = [
        0xF5_9E0B, 0x10_B981, 0xEF_4444, 0x8B_5CF6, 0x06_B6D4, 0xEC_4899, 0x84_CC16,
    ];
    if k == 0 {
        theme.line
    } else {
        Rgba::hex(MORE[(k - 1) % MORE.len()])
    }
}

/// Black or white, whichever reads better on `bg`.
pub(crate) fn contrast(bg: Rgba) -> Rgba {
    let l = 0.299 * f32::from(bg.r) + 0.587 * f32::from(bg.g) + 0.114 * f32::from(bg.b);
    if l > 150.0 {
        Rgba::hex(0x11_1418)
    } else {
        Rgba::hex(0xFF_FFFF)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_and_clamping() {
        let s = ColorScale::new(vec![
            (0.0, Rgba::hex(0x00_0000)),
            (0.5, Rgba::hex(0xFF_0000)),
            (1.0, Rgba::hex(0xFF_FFFF)),
        ]);
        assert_eq!(s.color_at(-1.0), Rgba::hex(0));
        assert_eq!(s.color_at(0.25), Rgba::hex(0x80_0000));
        assert_eq!(s.color_at(2.0), Rgba::hex(0xFF_FFFF));
        assert_eq!(s.color_at(f32::NAN), Rgba::hex(0));
    }

    #[test]
    fn positions_and_domains() {
        let theme = Theme::dark();
        let d = ColorScale::diverging(0.0, &theme);
        assert_eq!(d.kind.position(0.0, (-2.0, 5.0)), 0.5);
        assert_eq!(d.kind.position(5.0, (-2.0, 5.0)), 1.0);
        assert_eq!(d.kind.position(-5.0, (-2.0, 5.0)), 0.0);
        let s = ColorScale::sequential();
        let vals: Vec<f64> = (0..=100).map(f64::from).collect();
        assert_eq!(resolve_domain(s.domain, &vals), Some((1.0, 99.0)));
        assert_eq!(resolve_domain(s.domain, &[f64::NAN]), None);
    }
}
