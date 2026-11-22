//! Glyph atlas: an R8 texture filled with glyph coverage bitmaps by a shelf
//! packer.

use std::collections::HashMap;

use ferrochart_text::TextEngine;

/// Atlas side length in texels.
const SIZE: u32 = 2048;

/// Placement of a glyph in the atlas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
}

pub(crate) struct Atlas {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    map: HashMap<(u16, u16, u32), Option<Slot>>,
    x: u32,
    y: u32,
    row_h: u32,
}

impl Atlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ferrochart glyph atlas"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            map: HashMap::new(),
            x: 0,
            y: 0,
            row_h: 0,
        }
    }

    /// The slot of `c`, rasterizing and uploading it on first use. `None` for
    /// glyphs without pixels. When the atlas is full it starts over; glyphs
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
        let key = (font, engine.glyph_id(font, c), size.to_bits());
        if let Some(slot) = self.map.get(&key) {
            return *slot;
        }
        let slot = engine.rasterize(font, c, size).and_then(|g| {
            if g.width == 0 || g.height == 0 || g.width > SIZE || g.height > SIZE {
                return None;
            }
            if self.x + g.width > SIZE {
                self.x = 0;
                self.y += self.row_h + 1;
                self.row_h = 0;
            }
            if self.y + g.height > SIZE {
                self.map.clear();
                self.x = 0;
                self.y = 0;
                self.row_h = 0;
            }
            let slot = Slot {
                x: self.x,
                y: self.y,
                w: g.width,
                h: g.height,
                left: g.left,
                top: g.top,
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: slot.x,
                        y: slot.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &g.coverage,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(g.width),
                    rows_per_image: Some(g.height),
                },
                wgpu::Extent3d {
                    width: g.width,
                    height: g.height,
                    depth_or_array_layers: 1,
                },
            );
            self.x += g.width + 1;
            self.row_h = self.row_h.max(g.height);
            Some(slot)
        });
        self.map.insert(key, slot);
        slot
    }
}
