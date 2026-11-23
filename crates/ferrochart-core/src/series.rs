//! Per-series storage and the data operations of `set_data`, `append`,
//! `prepend` and `update_last` for every built-in series kind.
//!
//! The global time index is the union of the times of all series. The
//! chart extends or rebuilds it **before** an operation runs here, so every
//! time an operation sees is a slot of the index. Each series is stored
//! densely over its own span of slots: value series fill the slots they skip
//! by interpolation or NaN (see [`data::value`](crate::data::value)), OHLC
//! series with gap bars (all prices NaN). Heikin-Ashi candles are computed
//! here, in the data layer, from the OHLC data the user sets.
//!

use crate::data::lod::Lod;
use crate::data::ohlc::{OhlcStore, check_columns};
use crate::data::value::{ValueStore, check_values, densify};
use crate::data::{
    BarUpdate, ChangeLog, DataChange, OhlcColumns, OhlcLevelView, SeriesMapping, TimeIndex,
    Timestamp, ValidationMode, ValueColumns,
};
use crate::error::{Error, Result};
use crate::frame::SeriesKind;
use crate::plugin::CustomStore;
use crate::transform::store::TransformStore;

/// Data for `set_data` / `append` / `prepend`.
///
/// Custom (plugin) series data arrives with the plugin
/// interface in M5.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SeriesData {
    /// OHLC(V) bars: `Candlestick`, `OhlcBar`, `HeikinAshi`.
    Ohlc(OhlcColumns),
    /// Single values: `Line`, `Area`, `Baseline`, `Histogram`.
    Value(ValueColumns),
}

impl From<OhlcColumns> for SeriesData {
    fn from(c: OhlcColumns) -> Self {
        Self::Ohlc(c)
    }
}

impl From<ValueColumns> for SeriesData {
    fn from(c: ValueColumns) -> Self {
        Self::Value(c)
    }
}

impl SeriesData {
    /// The point times.
    pub(crate) fn times(&self) -> &[Timestamp] {
        match self {
            Self::Ohlc(c) => &c.time,
            Self::Value(c) => &c.time,
        }
    }

    /// Takes the time column out (for a series that becomes the whole time
    /// axis), after checking that it has one time per point.
    pub(crate) fn take_times(mut self) -> Result<(Vec<Timestamp>, Self)> {
        let (time, n, column) = match &mut self {
            Self::Ohlc(c) => (&mut c.time, c.open.len(), "open"),
            Self::Value(c) => (&mut c.time, c.value.len(), "value"),
        };
        if time.len() != n {
            return Err(Error::LengthMismatch { column });
        }
        let times = core::mem::take(time);
        Ok((times, self))
    }

    /// One point at `bar.time` (value series take `bar.close`).
    pub(crate) fn point(kind: SeriesKind, bar: BarUpdate) -> Self {
        if is_value_kind(kind) {
            Self::Value(ValueColumns {
                time: vec![bar.time],
                value: vec![bar.close],
                color: None,
            })
        } else {
            Self::Ohlc(OhlcStore::bar_columns(bar, true))
        }
    }

    /// `self` followed by `later` (same kind; per-point colors are filled
    /// with 0 where one side has none).
    pub(crate) fn concat(self, later: Self) -> Result<Self> {
        match (self, later) {
            (Self::Ohlc(mut a), Self::Ohlc(b)) => {
                if a.volume.is_some() != b.volume.is_some() {
                    return Err(Error::LengthMismatch { column: "volume" });
                }
                a.time.extend_from_slice(&b.time);
                a.open.extend_from_slice(&b.open);
                a.high.extend_from_slice(&b.high);
                a.low.extend_from_slice(&b.low);
                a.close.extend_from_slice(&b.close);
                if let (Some(x), Some(y)) = (a.volume.as_mut(), b.volume.as_ref()) {
                    x.extend_from_slice(y);
                }
                Ok(Self::Ohlc(a))
            }
            (Self::Value(mut a), Self::Value(b)) => {
                let (na, nb) = (a.value.len(), b.value.len());
                a.color = match (a.color.take(), b.color) {
                    (None, None) => None,
                    (x, y) => {
                        let mut c = x.unwrap_or_else(|| vec![0; na]);
                        c.extend(y.unwrap_or_else(|| vec![0; nb]));
                        Some(c)
                    }
                };
                a.time.extend_from_slice(&b.time);
                a.value.extend_from_slice(&b.value);
                Ok(Self::Value(a))
            }
            (Self::Ohlc(_), _) => Err(Error::InvalidArgument("this series needs OHLC data")),
            (Self::Value(_), _) => Err(Error::InvalidArgument("this series needs value data")),
        }
    }
}

const NOT_ON_AXIS: Error =
    Error::InvalidArgument("the times must be bars of the chart's time axis");
const NO_OWN_DATA: Error =
    Error::InvalidArgument("this series has no data of its own; use its source series");

/// What a series stores.
#[derive(Debug)]
// One `Store` per series, never in a hot array: boxing would only add an
// indirection to every data access.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Store {
    /// No data of its own (volume series read their source).
    None,
    Ohlc(OhlcStore),
    /// The OHLC data as set by the user and the Heikin-Ashi candles drawn.
    HeikinAshi {
        raw: OhlcStore,
        ha: OhlcStore,
    },
    Value(ValueStore),
    /// A plugin that owns its data.
    Custom(Box<CustomStore>),
    /// A transform series: its inputs and outputs.
    Transform(Box<TransformStore>),
}

/// A series' data as indicators read it.
#[derive(Clone, Copy)]
pub(crate) enum Source<'a> {
    /// Built-in data: no plugin involved, so it can be read from several
    /// threads (parallel full recomputation).
    Plain(PlainSource<'a>),
    /// A custom series: read through its plugin.
    Custom(&'a Store),
}

/// Built-in data of a series (`Sync`).
#[derive(Clone, Copy)]
pub(crate) enum PlainSource<'a> {
    None,
    /// OHLC data: `span` gives the covered indices, `bars` the values (the
    /// drawn candles: Heikin-Ashi, transform output).
    Ohlc {
        span: &'a OhlcStore,
        bars: &'a OhlcStore,
    },
    Value(&'a ValueStore),
}

impl Source<'_> {
    /// First logical index and end, `None` without data.
    pub(crate) fn span(&self) -> Option<(i64, i64)> {
        match self {
            Self::Custom(s) => s.span(),
            Self::Plain(PlainSource::None) => None,
            Self::Plain(PlainSource::Ohlc { span: s, .. }) => {
                (s.len() > 0).then(|| (s.first(), s.end()))
            }
            Self::Plain(PlainSource::Value(s)) => (s.len() > 0).then(|| (s.first(), s.end())),
        }
    }

    /// As [`Store::bar_at`].
    pub(crate) fn bar_at(&self, j: i64) -> Option<[f64; 4]> {
        match self {
            Self::Custom(s) => s.bar_at(j),
            Self::Plain(PlainSource::None) => None,
            Self::Plain(PlainSource::Ohlc { bars, .. }) => bars.bar_at(j),
            Self::Plain(PlainSource::Value(s)) => {
                s.value_at(j).filter(|v| !v.is_nan()).map(|v| [v; 4])
            }
        }
    }

    /// As [`Store::volume_at`].
    pub(crate) fn volume_at(&self, j: i64) -> Option<f64> {
        match self {
            Self::Plain(PlainSource::Ohlc { bars, .. }) => bars.volume_at(j),
            Self::Custom(s) => s.volume_at(j),
            Self::Plain(_) => None,
        }
    }

    /// As [`Store::has_volume`].
    pub(crate) fn has_volume(&self) -> bool {
        match self {
            Self::Plain(PlainSource::Ohlc { bars, .. }) => bars.has_volume(),
            Self::Custom(s) => s.has_volume(),
            Self::Plain(_) => false,
        }
    }
}

/// Whether a kind takes OHLC data.
pub(crate) fn is_ohlc_kind(kind: SeriesKind) -> bool {
    matches!(
        kind,
        SeriesKind::Candlestick | SeriesKind::OhlcBar | SeriesKind::HeikinAshi
    )
}

/// Whether a kind takes single values.
pub(crate) fn is_value_kind(kind: SeriesKind) -> bool {
    matches!(
        kind,
        SeriesKind::Line
            | SeriesKind::Area
            | SeriesKind::Baseline
            | SeriesKind::Histogram
            | SeriesKind::Points
    )
}

/// Heikin-Ashi bar from the previous Heikin-Ashi bar and a raw bar.
fn ha_bar(prev: Option<[f64; 4]>, raw: [f64; 4]) -> [f64; 4] {
    let [o, h, l, c] = raw;
    let close = (o + h + l + c) * 0.25;
    let open = match prev {
        Some([po, _, _, pc]) => (po + pc) * 0.5,
        None => (o + c) * 0.5,
    };
    [open, h.max(open).max(close), l.min(open).min(close), close]
}

/// Heikin-Ashi bars of raw logical indices `from..to`, continuing from `prev`
/// (the last Heikin-Ashi bar before `from`). Gaps stay gaps and do not break
/// the chain.
fn ha_columns(raw: &OhlcStore, from: i64, to: i64, mut prev: Option<[f64; 4]>) -> OhlcColumns {
    let n = usize::try_from(to - from).unwrap_or(0);
    let mut cols = OhlcColumns {
        time: Vec::new(),
        open: Vec::with_capacity(n),
        high: Vec::with_capacity(n),
        low: Vec::with_capacity(n),
        close: Vec::with_capacity(n),
        volume: raw.has_volume().then(|| Vec::with_capacity(n)),
    };
    for j in from..to {
        let x = match raw.bar_at(j) {
            Some(bar) => {
                let x = ha_bar(prev, bar);
                prev = Some(x);
                x
            }
            None => [f64::NAN; 4],
        };
        cols.open.push(x[0]);
        cols.high.push(x[1]);
        cols.low.push(x[2]);
        cols.close.push(x[3]);
        if let Some(v) = cols.volume.as_mut() {
            v.push(raw.volume_at(j).unwrap_or(0.0));
        }
    }
    cols
}

/// The last bar (not a gap) of `s` before logical index `j`.
fn last_bar_before(s: &OhlcStore, j: i64) -> Option<[f64; 4]> {
    (s.first()..j.min(s.end())).rev().find_map(|k| s.bar_at(k))
}

/// Recomputes all Heikin-Ashi candles (O(n)).
fn rebuild_ha(raw: &OhlcStore, ha: &mut OhlcStore) -> Result<()> {
    ha.replace_at(
        raw.first(),
        ha_columns(raw, raw.first(), raw.end(), None),
        ValidationMode::Repair,
    )?;
    Ok(())
}

/// Adds the Heikin-Ashi candles of raw bars `from..raw.end()`.
fn extend_ha(raw: &OhlcStore, ha: &mut OhlcStore, from: i64) -> Result<()> {
    if ha.len() == 0 || ha.first() != raw.first() || ha.end() != from {
        return rebuild_ha(raw, ha);
    }
    let prev = last_bar_before(ha, from);
    ha.append(
        None,
        ha_columns(raw, from, raw.end(), prev),
        ValidationMode::Repair,
    )?;
    Ok(())
}

/// OHLC columns with point `i` at slot `lead + offsets[i]` (`lead + i`
/// without offsets), gap bars between the points, and `lead` gap bars before
/// and `trail` after them. The time column is dropped.
fn densify_ohlc(
    offsets: Option<&[u32]>,
    cols: &OhlcColumns,
    lead: usize,
    trail: usize,
) -> Result<OhlcColumns> {
    let span = match offsets {
        Some(o) => o.last().map_or(0, |&x| x as usize + 1),
        None => cols.open.len(),
    };
    let n = lead + span + trail;
    let column = |v: f64| -> Result<Vec<f64>> {
        let mut c = Vec::new();
        c.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
        c.resize(n, v);
        Ok(c)
    };
    let mut out = OhlcColumns {
        time: Vec::new(),
        open: column(f64::NAN)?,
        high: column(f64::NAN)?,
        low: column(f64::NAN)?,
        close: column(f64::NAN)?,
        volume: match cols.volume {
            Some(_) => Some(column(0.0)?),
            None => None,
        },
    };
    for i in 0..cols.open.len() {
        let at = lead + offsets.and_then(|o| o.get(i)).map_or(i, |&x| x as usize);
        out.open[at] = cols.open[i];
        out.high[at] = cols.high[i];
        out.low[at] = cols.low[i];
        out.close[at] = cols.close[i];
        if let (Some(dst), Some(src)) = (out.volume.as_mut(), cols.volume.as_ref()) {
            dst[at] = src[i];
        }
    }
    Ok(out)
}

fn bar_update(time: i64, x: [f64; 4], volume: Option<f64>) -> BarUpdate {
    BarUpdate {
        time,
        open: x[0],
        high: x[1],
        low: x[2],
        close: x[3],
        volume,
    }
}

fn check_value_cols(c: &ValueColumns) -> Result<()> {
    if c.value.len() != c.time.len() {
        return Err(Error::LengthMismatch { column: "value" });
    }
    if c.color.as_ref().is_some_and(|k| k.len() != c.time.len()) {
        return Err(Error::LengthMismatch { column: "color" });
    }
    check_values(&c.value)
}

/// `(first, offsets)` of a mapping; `None` offsets for the aligned fast path.
fn split(m: SeriesMapping) -> (i64, Option<Vec<u32>>) {
    match m {
        SeriesMapping::Aligned { first } => (first, None),
        SeriesMapping::Mapped { first, offsets } => (first, Some(offsets)),
    }
}

/// Dense values (and colors) for points at `first + offsets[i]`, with an
/// optional anchor point at relative slot `anchor.0` that is used for
/// interpolation and then removed.
fn dense_with_anchor(
    offsets: Option<&[u32]>,
    cols: &ValueColumns,
    anchor: Option<(i64, f64)>,
    interpolate: bool,
) -> Result<(Vec<f64>, Option<Vec<u32>>)> {
    let n = cols.value.len();
    let mut offs: Vec<i64> = match offsets {
        Some(o) => o.iter().map(|&x| i64::from(x)).collect(),
        None => (0..i64::try_from(n).unwrap_or(0)).collect(),
    };
    let mut values = cols.value.clone();
    let mut colors = cols.color.clone();
    let before = anchor.is_some_and(|(at, _)| at < 0);
    if let Some((at, v)) = anchor {
        if before {
            offs.insert(0, at);
            values.insert(0, v);
            if let Some(c) = colors.as_mut() {
                c.insert(0, 0);
            }
        } else {
            offs.push(at);
            values.push(v);
            if let Some(c) = colors.as_mut() {
                c.push(0);
            }
        }
    }
    let base = offs.first().copied().unwrap_or(0);
    let rel: Vec<u32> = offs
        .iter()
        .map(|&o| u32::try_from(o - base).map_err(|_| NOT_ON_AXIS))
        .collect::<Result<_>>()?;
    let (mut v, mut c) = densify(&rel, &values, colors.as_deref(), interpolate)?;
    if anchor.is_some() {
        if before {
            v.remove(0);
            if let Some(c) = c.as_mut() {
                c.remove(0);
            }
        } else {
            v.pop();
            if let Some(c) = c.as_mut() {
                c.pop();
            }
        }
    }
    Ok((v, c))
}

impl Store {
    pub(crate) fn for_kind(kind: SeriesKind) -> Self {
        match kind {
            SeriesKind::HeikinAshi => Self::HeikinAshi {
                raw: OhlcStore::default(),
                ha: OhlcStore::default(),
            },
            k if is_ohlc_kind(k) => Self::Ohlc(OhlcStore::default()),
            k if is_value_kind(k) => Self::Value(ValueStore::default()),
            _ => Self::None,
        }
    }

    /// The store that is drawn (Heikin-Ashi: the transformed bars).
    /// The data as indicators read it.
    pub(crate) fn source(&self) -> Source<'_> {
        Source::Plain(match self {
            Self::None => PlainSource::None,
            Self::Ohlc(s) => PlainSource::Ohlc { span: s, bars: s },
            Self::HeikinAshi { raw, ha } => PlainSource::Ohlc {
                span: raw,
                bars: ha,
            },
            Self::Transform(t) => PlainSource::Ohlc {
                span: &t.out,
                bars: &t.out,
            },
            Self::Value(s) => PlainSource::Value(s),
            Self::Custom(_) => return Source::Custom(self),
        })
    }

    fn drawn(&self) -> Option<&OhlcStore> {
        match self {
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => Some(s),
            Self::Transform(t) => Some(&t.out),
            _ => None,
        }
    }

    /// The transform series' data.
    pub(crate) fn transform(&self) -> Option<&TransformStore> {
        match self {
            Self::Transform(t) => Some(t),
            _ => None,
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => s.len(),
            Self::Value(s) => s.len(),
            Self::Custom(c) => c.len(),
            Self::Transform(t) => t.out.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Logical indices `first..end` the series covers (gaps included).
    pub(crate) fn span(&self) -> Option<(i64, i64)> {
        match self {
            Self::None => None,
            Self::Ohlc(s) | Self::HeikinAshi { raw: s, .. } => {
                (s.len() > 0).then(|| (s.first(), s.end()))
            }
            Self::Transform(t) => (t.out.len() > 0).then(|| (t.out.first(), t.out.end())),
            Self::Value(s) => (s.len() > 0).then(|| (s.first(), s.end())),
            Self::Custom(c) => c.span(),
        }
    }

    /// Whether the series takes data (volume series read their source).
    pub(crate) fn takes_data(&self) -> bool {
        matches!(
            self,
            Self::Ohlc(_) | Self::HeikinAshi { .. } | Self::Value(_) | Self::Transform(_)
        )
    }

    /// The plugin of a custom series.
    pub(crate) fn custom(&self) -> Option<&CustomStore> {
        match self {
            Self::Custom(c) => Some(c),
            _ => None,
        }
    }

    pub(crate) fn log(&self) -> Option<&ChangeLog> {
        match self {
            Self::None => None,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => Some(s.log()),
            Self::Value(s) => Some(s.log()),
            Self::Custom(c) => Some(&c.log),
            Self::Transform(t) => Some(t.out.log()),
        }
    }

    pub(crate) fn lod(&self) -> Option<&Lod> {
        match self {
            Self::None | Self::Custom(_) => None,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => Some(s.lod()),
            Self::Transform(t) => Some(t.out.lod()),
            Self::Value(s) => Some(s.lod()),
        }
    }

    pub(crate) fn has_volume(&self) -> bool {
        self.drawn().is_some_and(OhlcStore::has_volume)
    }

    pub(crate) fn level_view(&self, level: u8) -> Option<OhlcLevelView<'_>> {
        match self {
            Self::None | Self::Custom(_) => None,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => s.level_view(level),
            Self::Transform(t) => t.out.level_view(level),
            Self::Value(s) => s.level_view(level),
        }
    }

    /// Level-0 per-point colors and the logical index of the first one.
    pub(crate) fn colors(&self) -> Option<(i64, &[u32])> {
        match self {
            Self::Value(s) => s.colors().map(|c| (s.first(), c)),
            _ => None,
        }
    }

    /// `[open, high, low, close]` at logical index `j` (value series: the
    /// value four times); `None` for gaps.
    pub(crate) fn bar_at(&self, j: i64) -> Option<[f64; 4]> {
        match self {
            Self::None => None,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => s.bar_at(j),
            Self::Transform(t) => t.out.bar_at(j),
            Self::Value(s) => s.value_at(j).filter(|v| !v.is_nan()).map(|v| [v; 4]),
            Self::Custom(c) => c.value_at(j).map(|v| [v; 4]),
        }
    }

    pub(crate) fn volume_at(&self, j: i64) -> Option<f64> {
        self.drawn()?.volume_at(j)
    }

    /// Minimum and maximum over logical indices `from..to`.
    pub(crate) fn extremes(&mut self, from: i64, to: i64) -> Option<(f64, f64)> {
        match self {
            // Custom series need the time index (`CustomStore::extremes`).
            Self::None | Self::Custom(_) => None,
            Self::Ohlc(s) | Self::HeikinAshi { ha: s, .. } => s.extremes(from, to),
            Self::Transform(t) => t.out.extremes(from, to),
            Self::Value(s) => s.extremes(from, to),
        }
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Ohlc(s) => s.heap_bytes(),
            Self::HeikinAshi { raw, ha } => raw.heap_bytes() + ha.heap_bytes(),
            Self::Value(s) => s.heap_bytes(),
            Self::Custom(c) => c.heap_bytes(),
            Self::Transform(t) => t.heap_bytes(),
        }
    }

    /// The series' data with times taken from `index`, to re-map it after the
    /// index was rebuilt. Gaps are left out.
    pub(crate) fn snapshot(&self, index: &TimeIndex) -> Option<SeriesData> {
        match self {
            // Custom series are re-mapped from their own times; a transform
            // series is never next to another series with data.
            Self::None | Self::Custom(_) | Self::Transform(_) => None,
            Self::Ohlc(s) | Self::HeikinAshi { raw: s, .. } => {
                let mut cols = OhlcColumns {
                    volume: s.has_volume().then(Vec::new),
                    ..OhlcColumns::default()
                };
                for j in s.first()..s.end() {
                    let (Some(t), Some(b)) = (index.time_at(j), s.bar_at(j)) else {
                        continue;
                    };
                    cols.time.push(t);
                    cols.open.push(b[0]);
                    cols.high.push(b[1]);
                    cols.low.push(b[2]);
                    cols.close.push(b[3]);
                    if let Some(v) = cols.volume.as_mut() {
                        v.push(s.volume_at(j).unwrap_or(0.0));
                    }
                }
                Some(SeriesData::Ohlc(cols))
            }
            Self::Value(s) => {
                let mut cols = ValueColumns {
                    color: s.colors().map(|_| Vec::new()),
                    ..ValueColumns::default()
                };
                for j in s.first()..s.end() {
                    let (Some(t), Some(v)) = (index.time_at(j), s.value_at(j)) else {
                        continue;
                    };
                    if v.is_nan() {
                        continue;
                    }
                    cols.time.push(t);
                    cols.value.push(v);
                    if let (Some(c), Some(src)) = (cols.color.as_mut(), s.colors()) {
                        let p = usize::try_from(j - s.first()).unwrap_or(0);
                        c.push(src.get(p).copied().unwrap_or(0));
                    }
                }
                Some(SeriesData::Value(cols))
            }
        }
    }
}

/// A data operation on one series. Every time it is given must already be
/// a slot of `index` (the chart extends the index first).
pub(crate) struct Op<'a> {
    pub kind: SeriesKind,
    pub index: &'a TimeIndex,
    pub mode: ValidationMode,
}

impl Op<'_> {
    fn ohlc(data: SeriesData) -> Result<OhlcColumns> {
        match data {
            SeriesData::Ohlc(c) => Ok(c),
            SeriesData::Value(_) => Err(Error::InvalidArgument("this series needs OHLC data")),
        }
    }

    fn value(data: SeriesData) -> Result<ValueColumns> {
        match data {
            SeriesData::Value(c) => Ok(c),
            SeriesData::Ohlc(_) => Err(Error::InvalidArgument("this series needs value data")),
        }
    }

    fn interpolate(&self) -> bool {
        // Bars and dots stand alone: a gap has none.
        !matches!(self.kind, SeriesKind::Histogram | SeriesKind::Points)
    }

    fn map(&self, times: &[Timestamp]) -> Result<SeriesMapping> {
        self.index.map_series(times)?.ok_or(NOT_ON_AXIS)
    }

    fn check_times(cols: &OhlcColumns) -> Result<()> {
        if cols.time.len() == cols.open.len() {
            Ok(())
        } else {
            Err(Error::LengthMismatch { column: "open" })
        }
    }

    /// `set_data`. Returns the number of repaired bars.
    pub(crate) fn set(&self, store: &mut Store, data: SeriesData) -> Result<usize> {
        match store {
            // The chart runs transform series' operations itself.
            Store::None | Store::Custom(_) | Store::Transform(_) => Err(NO_OWN_DATA),
            Store::Ohlc(s) => self.set_ohlc(s, Self::ohlc(data)?),
            Store::HeikinAshi { raw, ha } => {
                let repaired = self.set_ohlc(raw, Self::ohlc(data)?)?;
                rebuild_ha(raw, ha)?;
                Ok(repaired)
            }
            Store::Value(s) => {
                let cols = Self::value(data)?;
                check_value_cols(&cols)?;
                let (first, offsets) = split(self.map(&cols.time)?);
                match offsets {
                    None => s.replace(first, cols.value, cols.color)?,
                    Some(o) => {
                        let (v, c) =
                            densify(&o, &cols.value, cols.color.as_deref(), self.interpolate())?;
                        s.replace(first, v, c)?;
                        if self.interpolate()
                            && let [.., prev, _] = o.as_slice()
                        {
                            s.set_fill_from(Some(first + i64::from(*prev)));
                        }
                    }
                }
                Ok(0)
            }
        }
    }

    /// `set_data` of data whose point `p` is logical index `first + p` (the
    /// time column is ignored). Returns the number of repaired bars.
    pub(crate) fn set_aligned(
        &self,
        store: &mut Store,
        first: i64,
        data: SeriesData,
    ) -> Result<usize> {
        match store {
            // The chart runs transform series' operations itself.
            Store::None | Store::Custom(_) | Store::Transform(_) => Err(NO_OWN_DATA),
            Store::Ohlc(s) => s.replace_at(first, Self::ohlc(data)?, self.mode),
            Store::HeikinAshi { raw, ha } => {
                let repaired = raw.replace_at(first, Self::ohlc(data)?, self.mode)?;
                rebuild_ha(raw, ha)?;
                Ok(repaired)
            }
            Store::Value(s) => {
                let cols = Self::value(data)?;
                s.replace(first, cols.value, cols.color)?;
                Ok(0)
            }
        }
    }

    fn set_ohlc(&self, s: &mut OhlcStore, mut cols: OhlcColumns) -> Result<usize> {
        Self::check_times(&cols)?;
        match self.map(&cols.time)? {
            SeriesMapping::Aligned { first } => s.replace_at(first, cols, self.mode),
            SeriesMapping::Mapped { first, offsets } => {
                let repaired = check_columns(&mut cols, self.mode)?;
                s.replace_at(first, densify_ohlc(Some(&offsets), &cols, 0, 0)?, self.mode)?;
                Ok(repaired)
            }
        }
    }

    /// `append`. Returns the number of repaired bars.
    pub(crate) fn append(&self, store: &mut Store, data: SeriesData) -> Result<usize> {
        if store.is_empty() {
            return self.set(store, data);
        }
        match store {
            // The chart runs transform series' operations itself.
            Store::None | Store::Custom(_) | Store::Transform(_) => Err(NO_OWN_DATA),
            Store::Ohlc(s) => self.append_ohlc(s, Self::ohlc(data)?),
            Store::HeikinAshi { raw, ha } => {
                let old_end = raw.end();
                let repaired = self.append_ohlc(raw, Self::ohlc(data)?)?;
                extend_ha(raw, ha, old_end)?;
                Ok(repaired)
            }
            Store::Value(s) => {
                let cols = Self::value(data)?;
                check_value_cols(&cols)?;
                let (first, offsets) = split(self.map(&cols.time)?);
                if first < s.end() {
                    return Err(Error::TimeNotIncreasing { index: 0 });
                }
                if offsets.is_none() && first == s.end() {
                    s.append(&cols.value, cols.color.as_deref())?;
                    return Ok(0);
                }
                // Anchor on the last stored point so the gap is filled.
                let old_last = s.end() - 1;
                let anchor = s.value_at(old_last).map(|v| (old_last - first, v));
                let (v, c) =
                    dense_with_anchor(offsets.as_deref(), &cols, anchor, self.interpolate())?;
                s.append(&v, c.as_deref())?;
                if self.interpolate() {
                    // The given point before the new last one.
                    let n = cols.value.len();
                    let prev = match (n, offsets.as_deref()) {
                        (0 | 1, _) => old_last,
                        (_, Some(o)) => first + i64::from(o[n - 2]),
                        (_, None) => first + i64::try_from(n).unwrap_or(i64::MAX) - 2,
                    };
                    s.set_fill_from(Some(prev));
                }
                Ok(0)
            }
        }
    }

    fn append_ohlc(&self, s: &mut OhlcStore, mut cols: OhlcColumns) -> Result<usize> {
        Self::check_times(&cols)?;
        if cols.open.is_empty() {
            return Ok(0);
        }
        let (first, offsets) = split(self.map(&cols.time)?);
        if first < s.end() {
            return Err(Error::TimeNotIncreasing { index: 0 });
        }
        let lead = usize::try_from(first - s.end()).map_err(|_| NOT_ON_AXIS)?;
        if offsets.is_none() && lead == 0 {
            return s.append(None, cols, self.mode);
        }
        let repaired = check_columns(&mut cols, self.mode)?;
        s.append(
            None,
            densify_ohlc(offsets.as_deref(), &cols, lead, 0)?,
            self.mode,
        )?;
        Ok(repaired)
    }

    /// `prepend`. Returns the number of repaired bars.
    pub(crate) fn prepend(&self, store: &mut Store, data: SeriesData) -> Result<usize> {
        if store.is_empty() {
            return self.set(store, data);
        }
        match store {
            // The chart runs transform series' operations itself.
            Store::None | Store::Custom(_) | Store::Transform(_) => Err(NO_OWN_DATA),
            Store::Ohlc(s) => self.prepend_ohlc(s, Self::ohlc(data)?),
            Store::HeikinAshi { raw, ha } => {
                let repaired = self.prepend_ohlc(raw, Self::ohlc(data)?)?;
                // Every later candle depends on the history: recompute (O(n)).
                rebuild_ha(raw, ha)?;
                Ok(repaired)
            }
            Store::Value(s) => {
                let cols = Self::value(data)?;
                check_value_cols(&cols)?;
                if cols.value.is_empty() {
                    return Ok(0);
                }
                let (first, offsets) = split(self.map(&cols.time)?);
                let last_off = offsets
                    .as_ref()
                    .and_then(|o| o.last().map(|&x| i64::from(x)))
                    .unwrap_or_else(|| i64::try_from(cols.value.len()).unwrap_or(0) - 1);
                if first + last_off >= s.first() {
                    return Err(Error::TimeNotIncreasing {
                        index: cols.value.len().saturating_sub(1),
                    });
                }
                if offsets.is_none() && first + last_off + 1 == s.first() {
                    s.prepend(&cols.value, cols.color.as_deref())?;
                    return Ok(0);
                }
                let anchor = s.value_at(s.first()).map(|v| (s.first() - first, v));
                let (v, c) =
                    dense_with_anchor(offsets.as_deref(), &cols, anchor, self.interpolate())?;
                s.prepend(&v, c.as_deref())?;
                Ok(0)
            }
        }
    }

    fn prepend_ohlc(&self, s: &mut OhlcStore, mut cols: OhlcColumns) -> Result<usize> {
        Self::check_times(&cols)?;
        let n = cols.open.len();
        if n == 0 {
            return Ok(0);
        }
        let (first, offsets) = split(self.map(&cols.time)?);
        let span = offsets
            .as_ref()
            .and_then(|o| o.last())
            .map_or(n, |&x| x as usize + 1);
        let end = first.saturating_add(i64::try_from(span).unwrap_or(i64::MAX));
        if end > s.first() {
            return Err(Error::TimeNotIncreasing { index: n - 1 });
        }
        let trail = usize::try_from(s.first() - end).map_err(|_| NOT_ON_AXIS)?;
        if offsets.is_none() && trail == 0 {
            return s.prepend(None, cols, self.mode);
        }
        let repaired = check_columns(&mut cols, self.mode)?;
        s.prepend(
            None,
            densify_ohlc(offsets.as_deref(), &cols, 0, trail)?,
            self.mode,
        )?;
        Ok(repaired)
    }

    /// `update_last`: replaces the last point if `bar.time` is its time (no
    /// allocation), appends otherwise (with gaps up to it if needed). Value
    /// series use `bar.close`.
    pub(crate) fn update_last(
        &self,
        store: &mut Store,
        bar: BarUpdate,
    ) -> Result<(DataChange, usize)> {
        let j = self.index.index_of(bar.time).ok_or(NOT_ON_AXIS)?;
        match store {
            // The chart runs transform series' operations itself.
            Store::None | Store::Custom(_) | Store::Transform(_) => Err(NO_OWN_DATA),
            Store::Ohlc(s) => self.update_ohlc(s, j, bar),
            Store::HeikinAshi { raw, ha } => {
                let old_end = raw.end();
                let (change, repaired) = self.update_ohlc(raw, j, bar)?;
                let in_place = change == DataChange::UpdateLast
                    && ha.len() > 0
                    && ha.first() == raw.first()
                    && ha.end() == raw.end();
                if in_place {
                    let h = match raw.bar_at(j) {
                        Some(x) => ha_bar(last_bar_before(ha, j), x),
                        None => [f64::NAN; 4],
                    };
                    ha.set_last(
                        bar_update(bar.time, h, raw.volume_at(j)),
                        ValidationMode::Repair,
                    )?;
                } else if change == DataChange::UpdateLast {
                    rebuild_ha(raw, ha)?;
                } else {
                    extend_ha(raw, ha, old_end)?;
                }
                Ok((change, repaired))
            }
            Store::Value(s) => {
                let v = bar.close;
                check_values(&[v])?;
                if s.len() > 0 {
                    let last = s.end() - 1;
                    if j == last {
                        // Interpolated slots before the last point move with
                        // it; more than the last value changed then.
                        let filled = if self.interpolate() {
                            s.set_last_interpolated(v)?
                        } else {
                            s.set_last(v, None)?;
                            false
                        };
                        let change = if filled {
                            DataChange::Replace
                        } else {
                            DataChange::UpdateLast
                        };
                        return Ok((change, 0));
                    }
                    if j < last {
                        return Err(Error::TimeNotIncreasing { index: 0 });
                    }
                }
                let old_end = s.end();
                let fresh = s.len() == 0;
                self.append(store, SeriesData::point(self.kind, bar))?;
                let count = if fresh { 1 } else { appended(old_end, store) };
                Ok((DataChange::Append { count }, 0))
            }
        }
    }

    fn update_ohlc(
        &self,
        s: &mut OhlcStore,
        j: i64,
        bar: BarUpdate,
    ) -> Result<(DataChange, usize)> {
        if s.len() > 0 {
            if j == s.end() - 1 {
                return Ok((DataChange::UpdateLast, s.set_last(bar, self.mode)?));
            }
            if j < s.end() - 1 {
                return Err(Error::TimeNotIncreasing { index: 0 });
            }
            let old_end = s.end();
            let repaired = self.append_ohlc(s, OhlcStore::bar_columns(bar, true))?;
            let count = usize::try_from(s.end() - old_end).unwrap_or(1);
            return Ok((DataChange::Append { count }, repaired));
        }
        let repaired = s.replace_at(j, OhlcStore::bar_columns(bar, false), self.mode)?;
        Ok((DataChange::Append { count: 1 }, repaired))
    }
}

/// Number of points added after `old_end`.
fn appended(old_end: i64, store: &Store) -> usize {
    store
        .span()
        .and_then(|(_, end)| usize::try_from(end - old_end).ok())
        .unwrap_or(1)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact values are the point of these tests
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn bars(times: &[i64], closes: &[f64]) -> OhlcColumns {
        OhlcColumns {
            time: times.to_vec(),
            open: closes.to_vec(),
            high: closes.iter().map(|c| c + 1.0).collect(),
            low: closes.iter().map(|c| c - 1.0).collect(),
            close: closes.to_vec(),
            volume: None,
        }
    }

    fn op(kind: SeriesKind, index: &TimeIndex) -> Op<'_> {
        Op {
            kind,
            index,
            mode: ValidationMode::Reject,
        }
    }

    fn axis(times: &[i64]) -> TimeIndex {
        TimeIndex::from_times(times.to_vec()).unwrap()
    }

    fn v(time: &[i64], value: &[f64]) -> SeriesData {
        SeriesData::Value(ValueColumns {
            time: time.to_vec(),
            value: value.to_vec(),
            color: None,
        })
    }

    #[test]
    fn sparse_line_is_densified_and_extended() {
        let index = axis(&[10, 20, 30, 40, 50]);
        let mut line = Store::for_kind(SeriesKind::Line);
        let o = op(SeriesKind::Line, &index);
        o.set(&mut line, v(&[10, 30], &[1.0, 3.0])).unwrap();
        assert_eq!(line.bar_at(1), Some([2.0; 4])); // interpolated
        o.append(&mut line, v(&[50], &[7.0])).unwrap();
        assert_eq!(line.bar_at(3), Some([5.0; 4]));
        assert_eq!(line.len(), 5);
        assert!(o.set(&mut line, v(&[15], &[1.0])).is_err()); // not on the axis
        // Histograms leave gaps instead.
        let mut hist = Store::for_kind(SeriesKind::Histogram);
        op(SeriesKind::Histogram, &index)
            .set(&mut hist, v(&[10, 30], &[1.0, 3.0]))
            .unwrap();
        assert_eq!(hist.bar_at(1), None);
        assert_eq!(hist.span(), Some((0, 3)));
    }

    /// Level-0 values of a value store, gaps as NaN.
    fn values(store: &Store) -> Vec<f64> {
        let Some((first, end)) = store.span() else {
            return Vec::new();
        };
        (first..end)
            .map(|j| store.bar_at(j).map_or(f64::NAN, |b| b[3]))
            .collect()
    }

    fn same(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()))
    }

    fn tick(time: i64, close: f64) -> BarUpdate {
        BarUpdate {
            time,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close,
            volume: None,
        }
    }

    #[test]
    fn update_last_moves_the_interpolated_slots() {
        // Issue #1: the skipped slot stayed at the old midpoint.
        let index = axis(&[0, 1, 2, 3, 4, 5, 6]);
        for kind in [SeriesKind::Line, SeriesKind::Area, SeriesKind::Baseline] {
            let o = op(kind, &index);
            let mut s = Store::for_kind(kind);
            o.set(&mut s, v(&[0, 2], &[10.0, 30.0])).unwrap();
            assert_eq!(
                o.update_last(&mut s, tick(2, 50.0)).unwrap().0,
                DataChange::Replace
            );
            let mut fresh = Store::for_kind(kind);
            o.set(&mut fresh, v(&[0, 2], &[10.0, 50.0])).unwrap();
            assert!(same(&values(&s), &values(&fresh)), "{kind:?}");
            assert_eq!(values(&s), [10.0, 30.0, 50.0]);

            // After a sparse append of one point, and of several.
            o.append(&mut s, v(&[5], &[20.0])).unwrap();
            o.update_last(&mut s, tick(5, 80.0)).unwrap();
            o.set(&mut fresh, v(&[0, 2, 5], &[10.0, 50.0, 80.0]))
                .unwrap();
            assert!(same(&values(&s), &values(&fresh)), "{kind:?}");
            o.append(&mut s, v(&[6], &[1.0])).unwrap(); // adjacent: nothing filled
            assert_eq!(
                o.update_last(&mut s, tick(6, 2.0)).unwrap().0,
                DataChange::UpdateLast
            );
            assert_eq!(values(&s)[4], fresh.bar_at(4).unwrap()[3]);
        }
        // A new point through update_last fills the gap; replacing it again
        // moves the fill.
        let o = op(SeriesKind::Line, &index);
        let mut s = Store::for_kind(SeriesKind::Line);
        o.set(&mut s, v(&[0], &[0.0])).unwrap();
        o.update_last(&mut s, tick(4, 4.0)).unwrap();
        o.update_last(&mut s, tick(4, 8.0)).unwrap();
        assert_eq!(values(&s), [0.0, 2.0, 4.0, 6.0, 8.0]);
        // Histograms keep their gaps.
        let o = op(SeriesKind::Histogram, &index);
        let mut h = Store::for_kind(SeriesKind::Histogram);
        o.set(&mut h, v(&[0, 2], &[1.0, 3.0])).unwrap();
        assert_eq!(
            o.update_last(&mut h, tick(2, 5.0)).unwrap().0,
            DataChange::UpdateLast
        );
        assert_eq!(h.bar_at(1), None);
    }

    proptest! {
        /// Any sequence of sparse appends and last-point updates gives the
        /// store a fresh `set_data` of the resulting points would give.
        #[test]
        fn sparse_updates_equal_a_fresh_set(
            steps in proptest::collection::vec((1i64..4, -100.0f64..100.0, any::<bool>()), 1..30),
        ) {
            let times: Vec<i64> = (0..200).collect();
            let index = axis(&times);
            let o = op(SeriesKind::Line, &index);
            let mut s = Store::for_kind(SeriesKind::Line);
            let mut points: Vec<(i64, f64)> = vec![(0, 0.0)];
            o.set(&mut s, v(&[0], &[0.0])).unwrap();
            for (dt, value, replace) in steps {
                let last = points.last().copied().unwrap();
                if replace {
                    o.update_last(&mut s, tick(last.0, value)).unwrap();
                    points.last_mut().unwrap().1 = value;
                } else {
                    o.update_last(&mut s, tick(last.0 + dt, value)).unwrap();
                    points.push((last.0 + dt, value));
                }
            }
            let mut fresh = Store::for_kind(SeriesKind::Line);
            let (t, vals): (Vec<i64>, Vec<f64>) = points.into_iter().unzip();
            o.set(&mut fresh, v(&t, &vals)).unwrap();
            prop_assert!(same(&values(&s), &values(&fresh)));
        }
    }

    #[test]
    fn sparse_ohlc_gets_gap_bars() {
        let index = axis(&[1, 2, 3, 4, 5, 6]);
        let mut s = Store::for_kind(SeriesKind::OhlcBar);
        let o = op(SeriesKind::OhlcBar, &index);
        o.set(&mut s, SeriesData::Ohlc(bars(&[1, 3], &[1.0, 3.0])))
            .unwrap();
        assert_eq!(s.span(), Some((0, 3)));
        assert_eq!(s.bar_at(1), None);
        assert_eq!(s.bar_at(2), Some([3.0, 4.0, 2.0, 3.0]));
        let bar = BarUpdate {
            time: 5,
            open: 5.0,
            high: 6.0,
            low: 4.0,
            close: 5.5,
            volume: None,
        };
        // Two slots later: one gap bar, then the bar.
        assert_eq!(
            o.update_last(&mut s, bar).unwrap().0,
            DataChange::Append { count: 2 }
        );
        assert_eq!(
            o.update_last(&mut s, bar).unwrap().0,
            DataChange::UpdateLast
        );
        assert_eq!(s.bar_at(3), None);
        assert_eq!(s.bar_at(4), Some([5.0, 6.0, 4.0, 5.5]));
        o.prepend(&mut s, SeriesData::Ohlc(bars(&[1], &[0.5])))
            .unwrap_err(); // slot 1 is taken already
        // Snapshots leave the gaps out.
        let Some(SeriesData::Ohlc(snap)) = s.snapshot(&index) else {
            panic!("no snapshot");
        };
        assert_eq!(snap.time, [1, 3, 5]);
        // Partial NaN bars are invalid; all-NaN bars are gaps.
        let mut bad = bars(&[6], &[1.0]);
        bad.open[0] = f64::NAN;
        assert!(o.append(&mut s, SeriesData::Ohlc(bad)).is_err());
        assert!(
            o.append(&mut s, SeriesData::Ohlc(bars(&[6], &[f64::NAN])))
                .is_ok()
        );
        assert_eq!(s.bar_at(5), None);
    }

    #[test]
    fn heikin_ashi_follows_the_formula() {
        let index = axis(&[1, 2, 3]);
        let mut s = Store::for_kind(SeriesKind::HeikinAshi);
        let raw = OhlcColumns {
            time: vec![1, 3],
            open: vec![10.0, 12.0],
            high: vec![14.0, 15.0],
            low: vec![9.0, 11.0],
            close: vec![12.0, 13.0],
            volume: None,
        };
        op(SeriesKind::HeikinAshi, &index)
            .set(&mut s, SeriesData::Ohlc(raw))
            .unwrap();
        // First: open = (10 + 12) / 2, close = (10 + 14 + 9 + 12) / 4.
        assert_eq!(s.bar_at(0), Some([11.0, 14.0, 9.0, 11.25]));
        // The gap is skipped: open = (11 + 11.25) / 2.
        assert_eq!(s.bar_at(1), None);
        assert_eq!(s.bar_at(2), Some([11.125, 15.0, 11.0, 12.75]));
    }

    proptest! {
        /// Heikin-Ashi after incremental updates equals Heikin-Ashi of all bars.
        #[test]
        fn heikin_ashi_incremental_equals_full(
            closes in proptest::collection::vec(1.0f64..100.0, 2..60),
            ticks in proptest::collection::vec((0i64..3, 1.0f64..100.0), 1..30),
        ) {
            let times: Vec<i64> = (0..closes.len() as i64).collect();
            // Every time a tick can reach (steps of up to 2 leave gaps).
            let index = axis(&(0..=closes.len() as i64 + 2 * 30).collect::<Vec<_>>());
            let mut s = Store::for_kind(SeriesKind::HeikinAshi);
            let o = op(SeriesKind::HeikinAshi, &index);
            o.set(&mut s, SeriesData::Ohlc(bars(&times, &closes))).unwrap();
            let mut all = bars(&times, &closes);
            for (step, c) in ticks {
                let t = *all.time.last().unwrap() + step;
                let bar = BarUpdate { time: t, open: c, high: c + 1.0, low: c - 1.0, close: c, volume: None };
                o.update_last(&mut s, bar).unwrap();
                if step > 0 {
                    all.time.push(t); all.open.push(c); all.high.push(c + 1.0);
                    all.low.push(c - 1.0); all.close.push(c);
                } else {
                    let p = all.time.len() - 1;
                    all.open[p] = c; all.high[p] = c + 1.0; all.low[p] = c - 1.0; all.close[p] = c;
                }
            }
            let mut full = Store::for_kind(SeriesKind::HeikinAshi);
            o.set(&mut full, SeriesData::Ohlc(all.clone())).unwrap();
            prop_assert_eq!(s.span(), full.span());
            for j in 0..index.len() as i64 {
                prop_assert_eq!(s.bar_at(j), full.bar_at(j));
            }
        }
    }
}
