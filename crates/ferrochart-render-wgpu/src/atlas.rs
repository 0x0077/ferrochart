//! Glyph atlases, filled by shelf packers: an
//! R8 texture of coverage bitmaps (text drawn in its color) and an RGBA8
//! texture of color glyphs (emoji from a `GlyphSource`, drawn as is).

use std::collections::HashMap;

use ferrochart_text::{GlyphKey, TextEngine};

/// Side of the coverage atlas, in texels.
const MONO_SIZE: u32 = 2048;
/// Side of the color atlas, in texels (color glyphs are rare).
const COLOR_SIZE: u32 = 1024;

/// Placement of a glyph in an atlas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
    /// In the color atlas.
    pub color: bool,
}

/// One texture with a shelf packer.
pub(crate) struct Shelf {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    size: u32,
    texel_bytes: u32,
    x: u32,
    y: u32,
    row_h: u32,
}

impl Shelf {
    fn new(device: &wgpu::Device, label: &str, size: u32, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            size,
            texel_bytes: format.block_copy_size(None).unwrap_or(1),
            x: 0,
            y: 0,
            row_h: 0,
        }
    }

    /// Where a `w × h` bitmap goes; `(origin, wrapped)` where `wrapped`
    /// means the shelf started over and earlier slots are invalid.
    fn place(&mut self, w: u32, h: u32) -> Option<([u32; 2], bool)> {
        if w == 0 || h == 0 || w > self.size || h > self.size {
            return None;
        }
        if self.x + w > self.size {
            self.x = 0;
            self.y += self.row_h + 1;
            self.row_h = 0;
        }
        let mut wrapped = false;
        if self.y + h > self.size {
            self.x = 0;
            self.y = 0;
            self.row_h = 0;
            wrapped = true;
        }
        let at = [self.x, self.y];
        self.x += w + 1;
        self.row_h = self.row_h.max(h);
        Some((at, wrapped))
    }

    fn upload(&self, queue: &wgpu::Queue, [x, y]: [u32; 2], w: u32, h: u32, data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * self.texel_bytes),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub(crate) struct Atlas {
    pub(crate) mono: Shelf,
    pub(crate) color: Shelf,
    map: HashMap<(GlyphKey, u32), Option<Slot>>,
}

impl Atlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            mono: Shelf::new(
                device,
                "ferrochart glyph atlas",
                MONO_SIZE,
                wgpu::TextureFormat::R8Unorm,
            ),
            color: Shelf::new(
                device,
                "ferrochart color glyph atlas",
                COLOR_SIZE,
                wgpu::TextureFormat::Rgba8Unorm,
            ),
            map: HashMap::new(),
        }
    }

    /// The slot of `c`, rasterizing and uploading it on first use. `None` for
    /// glyphs without pixels. When an atlas is full it starts over; glyphs
    /// placed earlier in the same frame stay valid until overwritten, which
    /// only happens with more than ~10 000 distinct glyphs in use.
    pub(crate) fn get(
        &mut self,
        queue: &wgpu::Queue,
        engine: &TextEngine,
        font: u16,
        c: char,
        size: f32,
    ) -> Option<Slot> {
        let glyph = engine.glyph_key(font, c)?;
        let key = (glyph, size.to_bits());
        if let Some(slot) = self.map.get(&key) {
            return *slot;
        }
        let slot = engine.rasterize_key(glyph, size).and_then(|g| {
            let color = g.rgba.is_some();
            let shelf = if color {
                &mut self.color
            } else {
                &mut self.mono
            };
            let data = g.rgba.as_deref().unwrap_or(&g.coverage);
            // A malformed bitmap (from a GlyphSource) is skipped, not
            // uploaded: write_texture panics on a short buffer.
            let texels = (g.width as usize) * (g.height as usize);
            if data.len() != texels * shelf.texel_bytes as usize {
                return None;
            }
            let (at, wrapped) = shelf.place(g.width, g.height)?;
            shelf.upload(queue, at, g.width, g.height, data);
            if wrapped {
                self.map.retain(|_, s| s.is_some_and(|s| s.color != color));
            }
            Some(Slot {
                x: at[0],
                y: at[1],
                w: g.width,
                h: g.height,
                left: g.left,
                top: g.top,
                color,
            })
        });
        self.map.insert(key, slot);
        slot
    }
}
