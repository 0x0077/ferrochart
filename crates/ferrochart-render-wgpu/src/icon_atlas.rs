//! Icon atlas (r21): an RGBA8 texture of 64×64 tiles holding the images
//! registered with `Chart::add_icon`, read with `textureLoad` by
//! `fs_icon_rich`. Starts at 512² (64 tiles) and doubles up to 2048² (1024,
//! the core's limit). Synced from `ChartDataView::icons` when its generation
//! changes, like series data.

use std::collections::HashMap;

use ferrochart_core::icon::{ICON_TEXELS, IconId, IconsView};

const TILE: u32 = ICON_TEXELS as u32;
const MIN_SIDE: u32 = 512;
const MAX_SIDE: u32 = 2048;

pub(crate) struct IconAtlas {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    side: u32,
    /// Image id → tile number.
    tiles: HashMap<u32, u32>,
    free: Vec<u32>,
    next: u32,
    generation: Option<u64>,
}

impl IconAtlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let (texture, view) = Self::texture(device, MIN_SIDE);
        Self {
            texture,
            view,
            side: MIN_SIDE,
            tiles: HashMap::new(),
            free: Vec::new(),
            next: 0,
            generation: None,
        }
    }

    fn texture(device: &wgpu::Device, side: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ferrochart icon atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn capacity(&self) -> u32 {
        (self.side / TILE) * (self.side / TILE)
    }

    /// Texel origin of tile `n`.
    fn origin(&self, n: u32) -> [u32; 2] {
        let per_row = self.side / TILE;
        [(n % per_row) * TILE, (n / per_row) * TILE]
    }

    /// Texel origin of image `id`'s tile, if uploaded.
    pub(crate) fn tile_of(&self, id: IconId) -> Option<[u32; 2]> {
        self.tiles.get(&id.0).map(|&n| self.origin(n))
    }

    /// Uploads new images and frees removed ones when `icons` changed.
    /// Returns `true` when the texture was replaced (rebind it).
    pub(crate) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        icons: IconsView<'_>,
    ) -> bool {
        if self.generation == Some(icons.generation()) {
            return false;
        }
        self.generation = Some(icons.generation());
        let mut replaced = false;
        let need = u32::try_from(icons.len()).unwrap_or(u32::MAX);
        if need > self.capacity() {
            let mut side = self.side;
            while side < MAX_SIDE && (side / TILE) * (side / TILE) < need {
                side *= 2;
            }
            (self.texture, self.view) = Self::texture(device, side);
            self.side = side;
            self.tiles.clear();
            self.free.clear();
            self.next = 0;
            replaced = true;
        }
        // Free the tiles of removed images.
        let removed: Vec<u32> = self
            .tiles
            .keys()
            .copied()
            .filter(|&id| icons.get(IconId(id)).is_none())
            .collect();
        for id in removed {
            if let Some(n) = self.tiles.remove(&id) {
                self.free.push(n);
            }
        }
        for (id, tile) in icons.iter() {
            if self.tiles.contains_key(&id.0) {
                continue;
            }
            let n = match self.free.pop() {
                Some(n) => n,
                None if self.next < self.capacity() => {
                    self.next += 1;
                    self.next - 1
                }
                None => break,
            };
            self.tiles.insert(id.0, n);
            let [x, y] = self.origin(n);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                tile,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TILE * 4),
                    rows_per_image: Some(TILE),
                },
                wgpu::Extent3d {
                    width: TILE,
                    height: TILE,
                    depth_or_array_layers: 1,
                },
            );
        }
        replaced
    }
}
