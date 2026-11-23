//! Images for markers and plugin icons (r21): registered once, resampled to
//! a fixed 64×64 tile, read by the renderers through
//! [`ChartDataView::icons`](crate::chart::ChartDataView::icons).
//!
//! The core decodes nothing: hosts pass RGBA pixels. Resampling happens here,
//! once, so that every backend uploads the same texels (golden images agree).
//!

use crate::error::{Error, Result};

/// Side of the stored tile, in texels.
pub const ICON_TEXELS: usize = 64;
/// Bytes of a stored tile (RGBA8).
pub const ICON_BYTES: usize = ICON_TEXELS * ICON_TEXELS * 4;
/// Most images a chart holds at once.
pub const MAX_ICONS: usize = 1024;
/// Largest accepted image side, in pixels.
pub const MAX_IMAGE_SIDE: u32 = 256;

/// Identifier of an image registered with
/// [`Chart::add_icon`](crate::Chart::add_icon).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct IconId(pub u32);

/// Pixels passed to [`Chart::add_icon`](crate::Chart::add_icon): straight
/// (not premultiplied) sRGB RGBA8, row by row, `width × height × 4` bytes.
#[derive(Clone, Copy, Debug)]
pub struct IconImage<'a> {
    /// Width in pixels, 1..=256.
    pub width: u32,
    /// Height in pixels, 1..=256.
    pub height: u32,
    /// The pixels.
    pub rgba: &'a [u8],
}

/// A stored tile: premultiplied sRGB RGBA8, 64×64, row by row.
pub type IconTile = [u8; ICON_BYTES];

/// The images of a chart.
#[derive(Debug, Default)]
pub(crate) struct IconStore {
    entries: Vec<(IconId, Box<IconTile>)>,
    next: u32,
    generation: u64,
}

impl IconStore {
    pub(crate) fn add(&mut self, image: IconImage<'_>) -> Result<IconId> {
        if self.entries.len() >= MAX_ICONS {
            return Err(Error::CapacityExceeded("at most 1024 icons per chart"));
        }
        let tile = resample(image)?;
        let id = IconId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(Error::CapacityExceeded("icon ids exhausted"))?;
        self.entries
            .try_reserve(1)
            .map_err(|_| Error::OutOfMemory)?;
        self.entries.push((id, tile));
        self.generation += 1;
        Ok(id)
    }

    pub(crate) fn remove(&mut self, id: IconId) -> Result<()> {
        let k = self
            .entries
            .iter()
            .position(|(i, _)| *i == id)
            .ok_or(Error::UnknownId)?;
        self.entries.remove(k);
        self.generation += 1;
        Ok(())
    }

    pub(crate) fn view(&self) -> IconsView<'_> {
        IconsView { store: self }
    }
}

/// Read-only view of a chart's images, for renderers.
#[derive(Clone, Copy, Debug)]
pub struct IconsView<'a> {
    store: &'a IconStore,
}

impl<'a> IconsView<'a> {
    /// A view without images.
    pub fn empty() -> IconsView<'static> {
        static EMPTY: IconStore = IconStore {
            entries: Vec::new(),
            next: 0,
            generation: 0,
        };
        IconsView { store: &EMPTY }
    }

    /// Increases with every `add_icon` and `remove_icon`; a renderer
    /// re-syncs its atlas when it changes.
    pub fn generation(&self) -> u64 {
        self.store.generation
    }

    /// Number of images.
    pub fn len(&self) -> usize {
        self.store.entries.len()
    }

    /// Whether there are no images.
    pub fn is_empty(&self) -> bool {
        self.store.entries.is_empty()
    }

    /// The images in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (IconId, &'a IconTile)> + 'a {
        self.store.entries.iter().map(|(id, t)| (*id, &**t))
    }

    /// The tile of image `id`.
    pub fn get(&self, id: IconId) -> Option<&'a IconTile> {
        self.store
            .entries
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, t)| &**t)
    }
}

/// Resamples an image into a 64×64 premultiplied tile: fitted with its aspect
/// ratio, centered, transparent around it. Shrinking averages the covered
/// source area (box filter); enlarging is bilinear. Both work on
/// premultiplied values, so transparent pixels do not darken the edges.
///
/// # Errors
///
/// [`Error::InvalidArgument`] for a side outside 1..=256 or a buffer that is
/// not `width × height × 4` bytes.
///
/// # Examples
///
/// ```
/// use ferrochart_core::icon::{resample, IconImage, ICON_TEXELS};
///
/// // A 2×1 image (red, blue) fills the full width and the middle half.
/// let tile = resample(IconImage { width: 2, height: 1, rgba: &[255, 0, 0, 255, 0, 0, 255, 255] })?;
/// let at = |x: usize, y: usize| &tile[(y * ICON_TEXELS + x) * 4..][..4];
/// assert_eq!(at(0, 32), &[255, 0, 0, 255]);
/// assert_eq!(at(63, 32), &[0, 0, 255, 255]);
/// assert_eq!(at(0, 0), &[0, 0, 0, 0]);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn resample(image: IconImage<'_>) -> Result<Box<IconTile>> {
    let (w, h) = (image.width, image.height);
    if !(1..=MAX_IMAGE_SIDE).contains(&w) || !(1..=MAX_IMAGE_SIDE).contains(&h) {
        return Err(Error::InvalidArgument(
            "icon width and height must be 1..=256",
        ));
    }
    let (w, h) = (w as usize, h as usize);
    if image.rgba.len() != w * h * 4 {
        return Err(Error::InvalidArgument(
            "icon rgba must be width × height × 4 bytes",
        ));
    }
    // Premultiplied source, f32.
    let mut src = Vec::new();
    src.try_reserve_exact(w * h * 4)
        .map_err(|_| Error::OutOfMemory)?;
    for px in image.rgba.chunks_exact(4) {
        let a = f32::from(px[3]) / 255.0;
        src.extend_from_slice(&[
            f32::from(px[0]) * a,
            f32::from(px[1]) * a,
            f32::from(px[2]) * a,
            f32::from(px[3]),
        ]);
    }
    let n = ICON_TEXELS as f64;
    let scale = n / w.max(h) as f64; // texels per source pixel
    let (tw, th) = (w as f64 * scale, h as f64 * scale);
    let (ox, oy) = ((n - tw) * 0.5, (n - th) * 0.5);
    let mut tile = Box::new([0_u8; ICON_BYTES]);
    let texel = |x: usize, y: usize| -> &[f32] { &src[(y * w + x) * 4..][..4] };
    for ty in 0..ICON_TEXELS {
        for tx in 0..ICON_TEXELS {
            // The texel's footprint in source pixels.
            let x0 = (tx as f64 - ox) / scale;
            let x1 = (tx as f64 + 1.0 - ox) / scale;
            let y0 = (ty as f64 - oy) / scale;
            let y1 = (ty as f64 + 1.0 - oy) / scale;
            let mut acc = [0.0_f64; 4];
            if scale <= 1.0 {
                // Shrink: average over the footprint (clipped to the image;
                // the outside counts as transparent).
                let area = (x1 - x0) * (y1 - y0);
                let (xa, xb) = (x0.max(0.0), x1.min(w as f64));
                let (ya, yb) = (y0.max(0.0), y1.min(h as f64));
                if xa >= xb || ya >= yb {
                    continue;
                }
                let mut sy = ya.floor() as usize;
                while (sy as f64) < yb {
                    let fy = yb.min(sy as f64 + 1.0) - ya.max(sy as f64);
                    let mut sx = xa.floor() as usize;
                    while (sx as f64) < xb {
                        let fx = xb.min(sx as f64 + 1.0) - xa.max(sx as f64);
                        let p = texel(sx, sy);
                        for c in 0..4 {
                            acc[c] += f64::from(p[c]) * fx * fy;
                        }
                        sx += 1;
                    }
                    sy += 1;
                }
                for v in &mut acc {
                    *v /= area;
                }
            } else {
                // Enlarge: bilinear at the texel center, clamped to the
                // image; texels whose center is outside it stay transparent.
                let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
                if cx < 0.0 || cy < 0.0 || cx >= w as f64 || cy >= h as f64 {
                    continue;
                }
                let (fx, fy) = ((cx - 0.5).max(0.0), (cy - 0.5).max(0.0));
                let (ix, iy) = (fx.floor() as usize, fy.floor() as usize);
                let (dx, dy) = (fx - ix as f64, fy - iy as f64);
                let (jx, jy) = ((ix + 1).min(w - 1), (iy + 1).min(h - 1));
                for (c, v) in acc.iter_mut().enumerate() {
                    let a =
                        f64::from(texel(ix, iy)[c]) * (1.0 - dx) + f64::from(texel(jx, iy)[c]) * dx;
                    let b =
                        f64::from(texel(ix, jy)[c]) * (1.0 - dx) + f64::from(texel(jx, jy)[c]) * dx;
                    *v = a * (1.0 - dy) + b * dy;
                }
            }
            let out = &mut tile[(ty * ICON_TEXELS + tx) * 4..][..4];
            for (o, v) in out.iter_mut().zip(acc) {
                *o = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(tile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        px.repeat((w * h) as usize)
    }

    #[test]
    fn exact_size_is_copied() {
        let mut rgba = solid(64, 64, [10, 20, 30, 255]);
        rgba[0..4].copy_from_slice(&[200, 100, 50, 255]);
        let t = resample(IconImage {
            width: 64,
            height: 64,
            rgba: &rgba,
        })
        .unwrap();
        assert_eq!(&t[0..4], &[200, 100, 50, 255]);
        assert_eq!(&t[4..8], &[10, 20, 30, 255]);
    }

    #[test]
    fn shrink_averages_and_premultiplies() {
        // 128×128 checkerboard of opaque white and transparent: half alpha.
        let mut rgba = Vec::new();
        for y in 0..128 {
            for x in 0..128 {
                rgba.extend_from_slice(if (x + y) % 2 == 0 {
                    &[255, 255, 255, 255]
                } else {
                    &[255, 0, 0, 0]
                });
            }
        }
        let t = resample(IconImage {
            width: 128,
            height: 128,
            rgba: &rgba,
        })
        .unwrap();
        // Premultiplied: the transparent red adds nothing.
        assert_eq!(&t[0..4], &[128, 128, 128, 128]);
    }

    #[test]
    fn wide_images_are_centered() {
        let rgba = solid(256, 128, [0, 0, 0, 255]);
        let t = resample(IconImage {
            width: 256,
            height: 128,
            rgba: &rgba,
        })
        .unwrap();
        let a = |x: usize, y: usize| t[(y * ICON_TEXELS + x) * 4 + 3];
        assert_eq!((a(0, 15), a(0, 16), a(63, 47), a(63, 48)), (0, 255, 255, 0));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(
            resample(IconImage {
                width: 0,
                height: 1,
                rgba: &[]
            })
            .is_err()
        );
        assert!(
            resample(IconImage {
                width: 257,
                height: 1,
                rgba: &solid(257, 1, [0; 4])
            })
            .is_err()
        );
        assert!(
            resample(IconImage {
                width: 2,
                height: 2,
                rgba: &[0; 12]
            })
            .is_err()
        );
    }

    #[test]
    fn store_limits_and_generations() {
        let mut s = IconStore::default();
        let px = [1, 2, 3, 255];
        let a = s
            .add(IconImage {
                width: 1,
                height: 1,
                rgba: &px,
            })
            .unwrap();
        let b = s
            .add(IconImage {
                width: 1,
                height: 1,
                rgba: &px,
            })
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(s.view().generation(), 2);
        s.remove(a).unwrap();
        assert_eq!(s.remove(a), Err(Error::UnknownId));
        assert!(s.view().get(a).is_none() && s.view().get(b).is_some());
        assert_eq!(s.view().generation(), 3);
        for _ in 1..MAX_ICONS {
            s.add(IconImage {
                width: 1,
                height: 1,
                rgba: &px,
            })
            .unwrap();
        }
        assert!(matches!(
            s.add(IconImage {
                width: 1,
                height: 1,
                rgba: &px
            }),
            Err(Error::CapacityExceeded(_))
        ));
    }
}
