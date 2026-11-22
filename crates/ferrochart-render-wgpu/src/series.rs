//! GPU-resident series data, organized in chunks.
//!
//! Each (series, LOD level, chunk) owns vertex buffers of `CHUNK_LEN + 1`
//! elements (the extra one is the junction element: the next chunk's first
//! element in this chunk's reference). Chunks are created lazily when a draw
//! first needs them, so only what has been on screen lives on the GPU. A
//! chunk's reference price is frozen at creation; data changes reported by
//! `changes_since` only mark element ranges dirty, which are rewritten before
//! the chunk is drawn next. `DataDelta::Full` (e.g. `set_data`) drops all
//! chunks, which also resets their references.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use ferrochart_core::chart::ChartDataView;
use ferrochart_core::data::{ANCHOR, DataDelta, IdxSpan, OhlcLevelView, SeriesId};
use ferrochart_core::frame::{PriceMapping, YTransform};
use ferrochart_core::transform_helpers::{
    CHUNK_LEN, chunk_of, chunk_start, initial_chunk_ref, ohlc_rel, v_rel,
};

/// Elements per chunk buffer, including the junction element.
pub(crate) const SLOTS: i64 = CHUNK_LEN + 1;
const OHLC_STRIDE: u64 = 16;
const VALUE_STRIDE: u64 = 4;
/// Bytes of a per-point color buffer (`[r, g, b, a]` per element).
pub(crate) const COLOR_BYTES: u64 = VALUE_STRIDE * SLOTS as u64;

/// The vertex buffers of one chunk.
pub(crate) struct Chunk {
    /// `[open, high, low, close]` relative to `chunk_ref` (linear).
    pub(crate) ohlc: wgpu::Buffer,
    /// The same in `ln` space; created when first drawn in log mode.
    pub(crate) ln: Option<wgpu::Buffer>,
    /// Volumes relative to 0.
    pub(crate) volume: Option<wgpu::Buffer>,
    /// Per-point colors `[r, g, b, a]` (level 0 of value series that have them).
    pub(crate) colors: Option<wgpu::Buffer>,
    /// Frozen reference price.
    pub(crate) reference: f64,
    /// Elements (level space) that must be rewritten before drawing.
    dirty: Option<(i64, i64)>,
}

/// Per-series GPU state.
#[derive(Default)]
pub(crate) struct SeriesGpu {
    synced: Option<u64>,
    /// Bit `k` set when level `k` has chunks.
    levels: u64,
    chunks: HashMap<(u8, i64), Chunk>,
}

/// Element range of level `level` covering logical span `s`.
fn level_range(level: u8, s: IdxSpan) -> (i64, i64) {
    if level == 0 {
        (s.start, s.end)
    } else {
        let k = u32::from(level);
        ((s.start - ANCHOR) >> k, ((s.end - 1 - ANCHOR) >> k) + 1)
    }
}

fn linear() -> YTransform {
    YTransform {
        a: 0.0,
        b: 0.0,
        view_ref: 0.0,
        mapping: PriceMapping::Linear,
    }
}

fn log() -> YTransform {
    YTransform {
        mapping: PriceMapping::Log,
        ..linear()
    }
}

/// Scratch buffers reused across uploads.
#[derive(Default)]
pub(crate) struct Scratch {
    ohlc: Vec<[f32; 4]>,
    ln: Vec<[f32; 4]>,
    volume: Vec<f32>,
    colors: Vec<[u8; 4]>,
}

/// Level-0 per-point colors of a series: first logical index and values.
pub(crate) type PointColors<'a> = Option<(i64, &'a [u32])>;

impl SeriesGpu {
    /// Pulls the changes since the last sync.
    pub(crate) fn sync(&mut self, data: ChartDataView<'_>, id: SeriesId) {
        let Some(generation) = data.generation(id) else {
            self.chunks.clear();
            self.levels = 0;
            self.synced = None;
            return;
        };
        if self.synced == Some(generation) {
            return;
        }
        let delta = match self.synced {
            None => DataDelta::Full,
            Some(g) => data.changes_since(id, g),
        };
        match delta {
            DataDelta::Full => {
                self.chunks.clear();
                self.levels = 0;
            }
            DataDelta::Range(span) if !span.is_empty() => self.invalidate(span),
            DataDelta::None | DataDelta::Range(_) => {}
        }
        self.synced = Some(generation);
    }

    fn invalidate(&mut self, span: IdxSpan) {
        for level in 0..64_u8 {
            if self.levels & (1 << level) == 0 {
                continue;
            }
            let (lo, hi) = level_range(level, span);
            // `lo - 1`: the previous chunk's junction element may change too.
            for c in chunk_of(level, lo - 1)..=chunk_of(level, hi - 1) {
                let Some(chunk) = self.chunks.get_mut(&(level, c)) else {
                    continue;
                };
                let cs = chunk_start(level, c);
                let (a, b) = (lo.max(cs), hi.min(cs + SLOTS));
                if a < b {
                    chunk.dirty = Some(chunk.dirty.map_or((a, b), |(x, y)| (x.min(a), y.max(b))));
                }
            }
        }
    }

    /// Makes chunk `c` of `view`'s level ready for drawing (creating it, adding
    /// the ln buffer when `need_log`, and rewriting dirty elements). `colors`
    /// (level 0 only) adds a per-point color buffer. `None` if the chunk has no
    /// usable reference price.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scratch: &mut Scratch,
        view: &OhlcLevelView<'_>,
        colors: PointColors<'_>,
        c: i64,
        need_log: bool,
    ) -> Option<&Chunk> {
        let level = view.level;
        let cs = chunk_start(level, c);
        let key = (level, c);
        let chunk = match self.chunks.entry(key) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(slot) => {
                let reference = initial_chunk_ref(view, c)?;
                let make = |label, stride| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        size: stride * SLOTS as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                let chunk = Chunk {
                    ohlc: make("ferrochart ohlc chunk", OHLC_STRIDE),
                    ln: None,
                    volume: view
                        .volume
                        .map(|_| make("ferrochart volume chunk", VALUE_STRIDE)),
                    colors: (level == 0 && colors.is_some())
                        .then(|| make("ferrochart color chunk", VALUE_STRIDE)),
                    reference,
                    dirty: Some((cs, cs + SLOTS)),
                };
                self.levels |= 1 << level;
                slot.insert(chunk)
            }
        };
        if need_log && chunk.ln.is_none() {
            chunk.ln = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ferrochart ln chunk"),
                size: OHLC_STRIDE * SLOTS as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            chunk.dirty = Some((cs, cs + SLOTS));
        }
        if let Some((a, b)) = chunk.dirty.take() {
            write_range(queue, scratch, chunk, view, colors, cs, a, b);
        }
        Some(chunk)
    }

    /// Number of chunks on the GPU.
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// A prepared chunk.
    pub(crate) fn chunk(&self, level: u8, c: i64) -> Option<&Chunk> {
        self.chunks.get(&(level, c))
    }
}

/// Rewrites elements `a..b` (level space) of a chunk starting at `cs`.
#[allow(clippy::too_many_arguments)]
fn write_range(
    queue: &wgpu::Queue,
    scratch: &mut Scratch,
    chunk: &Chunk,
    view: &OhlcLevelView<'_>,
    colors: PointColors<'_>,
    cs: i64,
    a: i64,
    b: i64,
) {
    scratch.ohlc.clear();
    scratch.ln.clear();
    scratch.volume.clear();
    scratch.colors.clear();
    let (lin, lg) = (linear(), log());
    for j in a..b {
        let ohlc = (|| {
            Some([
                view.open.at(view.first, j)?,
                view.high.at(view.first, j)?,
                view.low.at(view.first, j)?,
                view.close.at(view.first, j)?,
            ])
        })();
        // Elements outside the data are gaps (a line's last element reads the
        // next one); `ohlc_rel` stores non-finite values as `GAP`.
        let o = ohlc.unwrap_or([f64::NAN; 4]);
        scratch.ohlc.push(ohlc_rel(&lin, o, chunk.reference));
        if chunk.ln.is_some() {
            scratch.ln.push(ohlc_rel(&lg, o, chunk.reference));
        }
        if chunk.colors.is_some() {
            let c = colors
                .and_then(|(first, all)| all.get(usize::try_from(j - first).ok()?))
                .copied()
                .unwrap_or(0);
            // 0xRRGGBBAA -> [r, g, b, a]; alpha 0 selects the series color.
            scratch.colors.push(c.to_be_bytes());
        }
        if chunk.volume.is_some() {
            let v = view
                .volume
                .and_then(|col| col.at(view.first, j))
                .unwrap_or(0.0);
            scratch.volume.push(v_rel(&lin, v, 0.0));
        }
    }
    let local = u64::try_from(a - cs).unwrap_or(0);
    queue.write_buffer(
        &chunk.ohlc,
        local * OHLC_STRIDE,
        bytemuck::cast_slice(&scratch.ohlc),
    );
    if let Some(ln) = &chunk.ln {
        queue.write_buffer(ln, local * OHLC_STRIDE, bytemuck::cast_slice(&scratch.ln));
    }
    if let Some(col) = &chunk.colors {
        queue.write_buffer(
            col,
            local * VALUE_STRIDE,
            bytemuck::cast_slice(&scratch.colors),
        );
    }
    if let Some(vol) = &chunk.volume {
        queue.write_buffer(
            vol,
            local * VALUE_STRIDE,
            bytemuck::cast_slice(&scratch.volume),
        );
    }
}

/// Byte offset of element `e` in a chunk starting at `cs`.
pub(crate) fn ohlc_offset(cs: i64, e: i64) -> u64 {
    u64::try_from(e - cs).unwrap_or(0) * OHLC_STRIDE
}

/// Byte offset of element `e` in a chunk's volume buffer.
pub(crate) fn value_offset(cs: i64, e: i64) -> u64 {
    u64::try_from(e - cs).unwrap_or(0) * VALUE_STRIDE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ranges_cover_buckets() {
        let s = IdxSpan { start: 10, end: 11 };
        assert_eq!(level_range(0, s), (10, 11));
        let (lo, hi) = level_range(2, s);
        assert_eq!(hi - lo, 1);
        assert_eq!(lo, (10 - ANCHOR) >> 2);
    }
}
