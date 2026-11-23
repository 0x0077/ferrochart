//! Deterministic test-data generators (feature `testing`).
//!
//! All tests, benchmarks, golden images and demos use these generators instead of
//! committed data files.
//!

use crate::chart::{Chart, SeriesData, SeriesOptions};
use crate::data::{OhlcColumns, Timestamp, ValueColumns};
use crate::error::{Error, Result};
use crate::frame::{LineDash, SeriesKind};
use crate::geom::{Css, Rgba};
use crate::pane::PaneOptions;

/// Starting price and mean-reversion target.
const BASE_PRICE: f64 = 100.0;
/// Per-bar volatility of the close-to-close move.
const SIGMA: f64 = 0.01;
/// Mean-reversion strength towards [`BASE_PRICE`]; keeps 10M-bar series in a sane
/// price range (measured: sd ≈ 23, extremes roughly 40..400 because the walk is
/// multiplicative) instead of drifting by orders of magnitude.
const THETA: f64 = 1e-3;
/// Volatility of the wick beyond the body.
const WICK_SIGMA: f64 = 0.005;
/// Mean volume per bar.
const BASE_VOLUME: f64 = 1000.0;
/// Volume volatility. With `|z| <= 2√3` the volume stays strictly positive.
const VOLUME_SIGMA: f64 = 0.25;
/// `√3`, the scale that turns a sum of 4 uniforms into unit variance.
const SQRT_3: f64 = 1.732_050_807_568_877_2;

/// Generates a deterministic OHLCV random walk with `n` bars.
///
/// Bar `i` has time `start_time + i * interval`. Each bar opens at the previous
/// close; closes follow a mean-reverting multiplicative random walk around 100.
/// The output always satisfies the data constraints (strictly increasing time,
/// `low <= min(open, close) <= max(open, close) <= high`, all prices and volumes
/// strictly positive).
///
/// The generator uses only IEEE-754 basic arithmetic (`+ - * /`, no `exp`/`ln`/`sin`
/// from the platform libm), so the same `seed` produces **bit-identical** data on
/// every platform. Golden images rely on this.
///
/// # Errors
///
/// - [`Error::InvalidArgument`] if `interval <= 0`, or if the last bar's time
///   would overflow `i64`;
/// - [`Error::OutOfMemory`] if the columns cannot be allocated.
///
/// # Examples
///
/// ```
/// use ferrochart_core::testing::random_walk;
///
/// // 1000 one-minute bars starting at 2024-01-01T00:00:00Z.
/// let bars = random_walk(42, 1000, 1_704_067_200_000, 60_000)?;
/// assert_eq!(bars.len(), 1000);
/// assert!(bars.low[0] <= bars.open[0] && bars.open[0] <= bars.high[0]);
/// // Same seed, same data.
/// assert_eq!(bars, random_walk(42, 1000, 1_704_067_200_000, 60_000)?);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn random_walk(
    seed: u64,
    n: usize,
    start_time: Timestamp,
    interval: i64,
) -> Result<OhlcColumns> {
    if interval <= 0 {
        return Err(Error::InvalidArgument("interval must be > 0"));
    }
    if n > 0 {
        let last = i64::try_from(n - 1)
            .ok()
            .and_then(|k| k.checked_mul(interval))
            .and_then(|span| start_time.checked_add(span));
        if last.is_none() {
            return Err(Error::InvalidArgument(
                "start_time + (n - 1) * interval overflows i64",
            ));
        }
    }

    let mut times = alloc(n)?;
    let mut opens = alloc(n)?;
    let mut highs = alloc(n)?;
    let mut lows = alloc(n)?;
    let mut closes = alloc(n)?;
    let mut volumes = alloc(n)?;

    let mut rng = SplitMix64(seed);
    let mut time = start_time;
    let mut prev_close = BASE_PRICE;
    for i in 0..n {
        let open = prev_close;
        let step = SIGMA * rng.normal() + THETA * (BASE_PRICE - open) / open;
        // |SIGMA * z| <= 0.035 and the reversion term is >= 0 when open < BASE_PRICE,
        // so the close stays strictly positive.
        let close = open * (1.0 + step);
        let body_hi = open.max(close);
        let body_lo = open.min(close);
        // Multiplying by a factor >= 1 (<= 1) never rounds below (above) the input,
        // so the OHLC constraints hold exactly.
        let high = body_hi * (1.0 + WICK_SIGMA * rng.normal().abs());
        let low = body_lo * (1.0 - WICK_SIGMA * rng.normal().abs());
        let vol = BASE_VOLUME * (1.0 + VOLUME_SIGMA * rng.normal());

        times.push(time);
        opens.push(open);
        highs.push(high);
        lows.push(low);
        closes.push(close);
        volumes.push(vol);

        prev_close = close;
        if i + 1 < n {
            // Cannot overflow: checked for the last bar above.
            time = time.saturating_add(interval);
        }
    }
    Ok(OhlcColumns {
        time: times,
        open: opens,
        high: highs,
        low: lows,
        close: closes,
        volume: Some(volumes),
    })
}

fn alloc<T>(n: usize) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| Error::OutOfMemory)?;
    Ok(v)
}

/// `SplitMix64`: tiny, fast, well-distributed and fully specified, so the stream is
/// identical on every platform and Rust version.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)` with 53 bits of precision.
    #[allow(clippy::cast_precision_loss)] // the value is < 2^53, so the cast is exact
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Approximately standard normal (Irwin–Hall with 4 terms): mean 0, variance 1,
    /// bounded to `[-2√3, 2√3]`. Uses only basic arithmetic, so it is bit-exact
    /// across platforms.
    fn normal(&mut self) -> f64 {
        let sum = self.uniform() + self.uniform() + self.uniform() + self.uniform();
        (sum - 2.0) * SQRT_3
    }
}

/// A [`TextMeasurer`](crate::text::TextMeasurer) with a fixed advance of 0.6 em per
/// character and a line height of 1.2 em. Lets tests and doctests build frames
/// without a font.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::{FontId, FontStyleRef};
/// use ferrochart_core::testing::MonoMeasurer;
/// use ferrochart_core::text::text_width;
///
/// let style = FontStyleRef { font: FontId(0), size: 10.0 };
/// assert_eq!(text_width(&MonoMeasurer, "12345", style), 30.0);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MonoMeasurer;

/// Advance of every character of [`MonoMeasurer`], in em.
const MONO_ADVANCE: f32 = 0.6;
static MONO_TABLE: [f32; 128] = [MONO_ADVANCE; 128];
static MONO_SUBSCRIPTS: [f32; 10] = [MONO_ADVANCE; 10];

impl crate::text::TextMeasurer for MonoMeasurer {
    fn ascii_advances(&self, _: crate::frame::FontStyleRef) -> &[f32; 128] {
        &MONO_TABLE
    }

    fn subscript_advances(&self, _: crate::frame::FontStyleRef) -> Option<&[f32; 10]> {
        Some(&MONO_SUBSCRIPTS)
    }

    fn measure(&self, text: &str, style: crate::frame::FontStyleRef) -> crate::geom::SizePx {
        crate::geom::SizePx {
            w: text.chars().count() as f32 * MONO_ADVANCE * style.size,
            h: self.line_height(style),
        }
    }

    fn line_height(&self, style: crate::frame::FontStyleRef) -> f32 {
        style.size * 1.2
    }
}

/// Simple moving average over `n` values; NaN before the first full window.
fn sma(values: &[f64], n: usize) -> Vec<f64> {
    let mut sum = 0.0;
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            sum += v;
            if i >= n {
                sum -= values.get(i - n).copied().unwrap_or(0.0);
            }
            if i + 1 < n { f64::NAN } else { sum / n as f64 }
        })
        .collect()
}

fn on_times(data: &OhlcColumns, value: Vec<f64>, color: Option<Vec<u32>>) -> SeriesData {
    SeriesData::Value(ValueColumns {
        time: data.time.clone(),
        value,
        color,
    })
}

/// Fills an empty, sized chart with one series of every non-candle kind
/// (golden images, demos):
///
/// - main pane: 120 hourly OHLC bars, a 5-bar average as a solid line and a
///   20-bar average as a dashed step line;
/// - second pane: an area of the closes with a 6-bar gap;
/// - third pane: the difference of the averages as a baseline around 0, over
///   a histogram of its change with per-point colors.
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::series_kinds_scene;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// series_kinds_scene(&mut chart, 0)?;
/// assert_eq!(chart.panes().count(), 3);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn series_kinds_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    let pane = chart.main_pane();
    let data = random_walk(14, 120, start_time, 3_600_000)?;
    let fast = sma(&data.close, 5);
    let slow = sma(&data.close, 20);

    let bars = chart.add_series(pane, SeriesKind::OhlcBar, SeriesOptions::default())?;
    chart.set_data(bars, SeriesData::Ohlc(data.clone()))?;
    let line = chart.add_series(pane, SeriesKind::Line, SeriesOptions::default())?;
    chart.set_data(line, on_times(&data, fast.clone(), None))?;
    let step = chart.add_series(
        pane,
        SeriesKind::Line,
        SeriesOptions {
            color: Some(Rgba::hex(0x00FF_9800)),
            line_width: Css(1.0),
            line_style: LineDash::Dashed,
            step: true,
            ..SeriesOptions::default()
        },
    )?;
    chart.set_data(step, on_times(&data, slow.clone(), None))?;

    let area_pane = chart.add_pane(PaneOptions {
        stretch_factor: 0.5,
    });
    let area = chart.add_series(area_pane, SeriesKind::Area, SeriesOptions::default())?;
    let close: Vec<f64> = data
        .close
        .iter()
        .enumerate()
        .map(|(i, &c)| if (60..66).contains(&i) { f64::NAN } else { c })
        .collect();
    chart.set_data(area, on_times(&data, close, None))?;

    let osc_pane = chart.add_pane(PaneOptions {
        stretch_factor: 0.5,
    });
    let osc: Vec<f64> = fast.iter().zip(&slow).map(|(f, s)| f - s).collect();
    let change: Vec<f64> = core::iter::once(f64::NAN)
        .chain(osc.windows(2).map(|w| w[1] - w[0]))
        .collect();
    let colors = change
        .iter()
        .map(|v| if *v >= 0.0 { 0x26A6_9A80 } else { 0xEF53_5080 })
        .collect();
    let hist = chart.add_series(osc_pane, SeriesKind::Histogram, SeriesOptions::default())?;
    chart.set_data(hist, on_times(&data, change, Some(colors)))?;
    let baseline = chart.add_series(osc_pane, SeriesKind::Baseline, SeriesOptions::default())?;
    chart.set_data(baseline, on_times(&data, osc, None))?;

    chart.time_scale().set_bar_spacing(6.0);
    Ok(())
}

/// Fills an empty, sized chart with candles (and their volume) whose time
/// axis is merged with a line at half-hour offsets: the candles get a gap bar at every
/// inserted time, and the
/// volume skips them. With markers: stacked above and below, with
/// text, all four shapes, one on the line, and one on a gap bar (not drawn).
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::merged_axis_scene;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// merged_axis_scene(&mut chart, 0)?;
/// assert_eq!(chart.data_view().time_index().len(), 60);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn merged_axis_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::chart::{
        Marker,
        MarkerPosition::{AboveBar, BelowBar},
        MarkerShape::{ArrowDown, ArrowUp, Circle, Square},
    };
    const HOUR: i64 = 3_600_000;
    let pane = chart.main_pane();
    let data = random_walk(11, 40, start_time, HOUR)?;
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(candles))?;
    let line = chart.add_series(
        pane,
        SeriesKind::Line,
        SeriesOptions {
            color: Some(Rgba::hex(0x00FF_9800)),
            ..SeriesOptions::default()
        },
    )?;
    chart.set_data(candles, SeriesData::Ohlc(data.clone()))?;
    // Bars 10..30 of the line sit half an hour after the candles.
    let (time, value) = (10..30)
        .map(|i| {
            (
                data.time[i] + HOUR / 2,
                (data.open[i + 1] + data.close[i]) * 0.5,
            )
        })
        .unzip();
    chart.set_data(
        line,
        SeriesData::Value(ValueColumns {
            time,
            value,
            color: None,
        }),
    )?;
    let (up, down) = (Rgba::hex(0x0026_A69A), Rgba::hex(0x00EF_5350));
    let marker = |i: usize, position, shape, color, text: &str| Marker {
        time: data.time[i],
        position,
        shape,
        color: Some(color),
        text: text.into(),
        ..Marker::default()
    };
    let mut gap = marker(12, AboveBar, Circle, down, "gap");
    gap.time += HOUR / 2;
    chart.set_markers(
        candles,
        vec![
            marker(35, AboveBar, ArrowDown, down, "Sell"),
            marker(3, BelowBar, ArrowUp, up, "Buy"),
            marker(3, BelowBar, Circle, up, ""),
            marker(8, AboveBar, Square, down, ""),
            marker(8, AboveBar, ArrowDown, down, "S"),
            gap,
        ],
    )?;
    chart.set_markers(
        line,
        vec![Marker {
            time: data.time[20] + HOUR / 2,
            position: BelowBar,
            shape: Circle,
            ..Marker::default()
        }],
    )?;
    chart.time_scale().set_bar_spacing(12.0);
    Ok(())
}

/// Fills an empty, sized chart with a comparison in percent: candles and a second symbol
/// at about three times the price
/// as an area, on a `Percentage` scale, so both start at 0 %; with two price
/// lines on the candles (one titled).
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::percentage_scene;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// percentage_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn percentage_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::price_scale::{PriceScaleMode, ScaleSide};
    let pane = chart.main_pane();
    let data = random_walk(11, 60, start_time, 3_600_000)?;
    let other = random_walk(15, 60, start_time, 3_600_000)?;
    let last = data.close.last().copied().unwrap_or(100.0);
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(data))?;
    let area = chart.add_series(
        pane,
        SeriesKind::Area,
        SeriesOptions {
            color: Some(Rgba::hex(0x00FF_9800)),
            ..SeriesOptions::default()
        },
    )?;
    let value = other.close.iter().map(|c| c * 3.0).collect();
    chart.set_data(
        area,
        SeriesData::Value(ValueColumns {
            time: other.time,
            value,
            color: None,
        }),
    )?;
    // Price lines in the series' units: they follow the relative scale.
    chart.create_price_line(
        candles,
        crate::chart::PriceLineOptions {
            price: last * 1.02,
            color: Some(Rgba::hex(0x009C_27B0)),
            title: "target".into(),
            ..Default::default()
        },
    )?;
    chart.create_price_line(
        candles,
        crate::chart::PriceLineOptions {
            price: last * 0.985,
            line_width: crate::geom::Css(2.0),
            line_style: crate::frame::LineDash::Solid,
            ..Default::default()
        },
    )?;
    if let Some(scale) = chart.price_scale(pane, ScaleSide::Right) {
        scale.set_mode(PriceScaleMode::Percentage)?;
    }
    chart.time_scale().set_bar_spacing(12.0);
    Ok(())
}

/// Fills an empty, sized chart with candles and every built-in drawing tool:
/// lines, a channel (selected, so its handles
/// show), shapes, a text, measurements, Fibonacci tools and positions, the
/// short position partly past the last bar (extrapolated times).
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::drawing::BUILTIN_KINDS;
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::drawings_scene;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// drawings_scene(&mut chart, 0)?;
/// // The 16 kinds of the first batch.
/// assert_eq!(chart.drawings().ids().count(), BUILTIN_KINDS[..16].len());
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn drawings_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::drawing::{Anchor, DrawingData, DrawingStyle, Param};
    const HOUR: i64 = 3_600_000;
    let pane = chart.main_pane();
    let data = random_walk(17, 80, start_time, HOUR)?;
    let lo = data.low.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = data.high.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(data))?;
    // (bar, fraction of the price range)
    let at = |bar: i64, f: f64| Anchor {
        time: start_time + bar * HOUR,
        price: lo + (hi - lo) * f,
    };
    let orange = Rgba::hex(0x00FF_9800);
    let purple = Rgba::hex(0x009C_27B0);
    let styled = |line: Rgba, dash: LineDash, width: f32| DrawingStyle {
        line,
        line_width: Css(width),
        line_dash: dash,
        fill: line.with_alpha(40),
        text: line,
        ..DrawingStyle::default()
    };
    let mut text = DrawingData::new("text", vec![at(3, 0.02)]);
    text.set("text", Param::Text("FerroChart".into()));
    let drawings = [
        DrawingData::new("trend_line", vec![at(2, 0.2), at(18, 0.6)]),
        DrawingData {
            style: styled(orange, LineDash::Solid, 2.0),
            ..DrawingData::new("ray", vec![at(5, 0.9), at(12, 0.86)])
        },
        DrawingData {
            style: styled(purple, LineDash::Dashed, 1.0),
            ..DrawingData::new("extended_line", vec![at(60, 0.1), at(70, 0.16)])
        },
        DrawingData::new("horizontal_line", vec![at(0, 0.97)]),
        DrawingData {
            style: styled(orange, LineDash::Dotted, 1.0),
            ..DrawingData::new("horizontal_ray", vec![at(40, 0.04)])
        },
        DrawingData::new("vertical_line", vec![at(30, 0.0)]),
        DrawingData::new(
            "parallel_channel",
            vec![at(20, 0.3), at(35, 0.5), at(20, 0.15)],
        ),
        DrawingData {
            style: styled(purple, LineDash::Solid, 1.0),
            ..DrawingData::new("rectangle", vec![at(38, 0.62), at(48, 0.8)])
        },
        DrawingData {
            style: styled(orange, LineDash::Solid, 2.0),
            ..DrawingData::new("ellipse", vec![at(50, 0.55), at(58, 0.75)])
        },
        text,
        DrawingData::new("price_range", vec![at(62, 0.6), at(66, 0.85)]),
        DrawingData::new("date_range", vec![at(68, 0.4), at(76, 0.5)]),
        DrawingData::new("fib_retracement", vec![at(40, 0.12), at(52, 0.45)]),
        DrawingData {
            style: styled(purple, LineDash::Solid, 1.0),
            ..DrawingData::new("fib_extension", vec![at(2, 0.3), at(8, 0.42), at(12, 0.33)])
        },
        DrawingData::new("long_position", vec![at(72, 0.7), at(78, 0.85)]),
        DrawingData::new("short_position", vec![at(80, 0.3), at(88, 0.2)]),
    ];
    let mut channel = None;
    for d in &drawings {
        let id = chart.drawings().add(pane, d)?;
        if d.kind == "parallel_channel" {
            channel = Some(id);
        }
    }
    chart.drawings().select(channel)?;
    chart.time_scale().set_bar_spacing(7.5);
    chart.time_scale().set_right_offset(14.0);
    Ok(())
}

/// Fills an empty, sized chart with candles and every second-batch drawing
/// tool: the three pitchforks, a Gann fan,
/// Fibonacci time zones, XABCD, head and shoulders (selected: 7 handles),
/// Elliott waves (the correction past the last bar), a path, a brush, an
/// arrow and a callout. Shared by the golden tests of both backends.
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::{Chart, ChartOptions};
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::drawings2_scene;
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// drawings2_scene(&mut chart, 0)?;
/// assert_eq!(chart.drawings().ids().count(), 13);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn drawings2_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::drawing::{Anchor, DrawingData, DrawingStyle, Param};
    const HOUR: i64 = 3_600_000;
    let pane = chart.main_pane();
    let data = random_walk(23, 80, start_time, HOUR)?;
    let lo = data.low.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = data.high.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(data))?;
    // (bar, fraction of the price range); bars may be fractional (brush).
    let at = |bar: f64, f: f64| Anchor {
        time: start_time + (bar * HOUR as f64) as i64,
        price: lo + (hi - lo) * f,
    };
    let orange = Rgba::hex(0x00FF_9800);
    let purple = Rgba::hex(0x009C_27B0);
    let teal = Rgba::hex(0x0000_897B);
    let styled = |line: Rgba, width: f32| DrawingStyle {
        line,
        line_width: Css(width),
        fill: line.with_alpha(36),
        text: line,
        ..DrawingStyle::default()
    };
    let with = |d: DrawingData, style: DrawingStyle| DrawingData { style, ..d };
    let pts = |v: &[(f64, f64)]| v.iter().map(|&(b, f)| at(b, f)).collect::<Vec<_>>();
    let brush: Vec<Anchor> = (0..48)
        .map(|j| {
            let b = 48.0 + f64::from(j) * 0.25;
            at(b, 0.9 + 0.04 * (f64::from(j) * 0.5).sin())
        })
        .collect();
    let mut callout = DrawingData::new("callout", pts(&[(36.0, 0.98), (40.0, 0.86)]));
    callout.set("text", Param::Text("Breakout".into()));
    let drawings = [
        DrawingData::new("pitchfork", pts(&[(2.0, 0.45), (8.0, 0.7), (12.0, 0.5)])),
        with(
            DrawingData::new(
                "schiff_pitchfork",
                pts(&[(14.0, 0.8), (20.0, 0.97), (24.0, 0.84)]),
            ),
            styled(orange, 1.0),
        ),
        with(
            DrawingData::new(
                "modified_schiff_pitchfork",
                pts(&[(26.0, 0.1), (31.0, 0.3), (35.0, 0.16)]),
            ),
            styled(purple, 1.0),
        ),
        with(
            DrawingData::new("gann_fan", pts(&[(60.0, 0.05), (66.0, 0.2)])),
            styled(teal, 1.0),
        ),
        with(
            DrawingData::new("fib_time_zones", pts(&[(44.0, 0.0), (46.0, 0.0)])),
            styled(purple, 1.0),
        ),
        DrawingData::new(
            "xabcd_pattern",
            pts(&[
                (2.0, 0.05),
                (6.0, 0.3),
                (9.0, 0.15),
                (13.0, 0.26),
                (17.0, 0.02),
            ]),
        ),
        with(
            DrawingData::new(
                "head_and_shoulders",
                pts(&[
                    (38.0, 0.45),
                    (41.0, 0.6),
                    (44.0, 0.48),
                    (48.0, 0.75),
                    (52.0, 0.5),
                    (55.0, 0.62),
                    (58.0, 0.44),
                ]),
            ),
            styled(orange, 2.0),
        ),
        DrawingData::new(
            "elliott_impulse",
            pts(&[
                (62.0, 0.45),
                (65.0, 0.6),
                (67.0, 0.52),
                (71.0, 0.82),
                (73.0, 0.72),
                (77.0, 0.9),
            ]),
        ),
        with(
            DrawingData::new(
                "elliott_correction",
                pts(&[(79.0, 0.9), (82.0, 0.7), (84.0, 0.8), (88.0, 0.6)]),
            ),
            styled(purple, 1.0),
        ),
        with(
            DrawingData::new(
                "path",
                pts(&[
                    (18.0, 0.55),
                    (22.0, 0.62),
                    (25.0, 0.5),
                    (30.0, 0.66),
                    (33.0, 0.58),
                ]),
            ),
            styled(teal, 2.0),
        ),
        with(DrawingData::new("brush", brush), styled(orange, 3.0)),
        DrawingData::new("arrow", pts(&[(86.0, 0.25), (91.0, 0.4)])),
        callout,
    ];
    let mut selected = None;
    for d in &drawings {
        let id = chart.drawings().add(pane, d)?;
        if d.kind == "head_and_shoulders" {
            selected = Some(id);
        }
    }
    chart.drawings().select(selected)?;
    chart.time_scale().set_bar_spacing(7.5);
    chart.time_scale().set_right_offset(14.0);
    Ok(())
}

/// A chart showing a crosshair and range received from a synced chart:
/// 60 hourly candles with the volume in a second pane,
/// the range set by a `VisibleRange` message and the crosshair inside bar 31
/// (between bar times), so only the vertical line and the time label show.
/// Shared by the golden tests of both backends.
///
/// # Errors
///
/// Errors of the chart calls (none for an empty chart).
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::synced_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// synced_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn synced_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::sync::SyncMessage;
    const HOUR: i64 = 3_600_000;
    let candles = chart.add_series(
        chart.main_pane(),
        SeriesKind::Candlestick,
        SeriesOptions::default(),
    )?;
    let volume_pane = chart.add_pane(PaneOptions {
        stretch_factor: 0.35,
    });
    chart.add_series(
        volume_pane,
        SeriesKind::Volume,
        SeriesOptions::volume_of(candles),
    )?;
    chart.set_data(
        candles,
        SeriesData::Ohlc(random_walk(19, 60, start_time, HOUR)?),
    )?;
    // Build once so the plot width is known when the range is applied.
    chart.build_frame(0.0, &MonoMeasurer);
    chart.apply_sync(SyncMessage::VisibleRange {
        from: start_time + 10 * HOUR,
        to: start_time + 70 * HOUR,
    });
    chart.apply_sync(SyncMessage::Crosshair {
        time: Some(start_time + 31 * HOUR + 20 * 60_000),
    });
    Ok(())
}

/// The transform-series scene of the golden images: 3000 one-minute
/// bars as Renko bricks (`SeriesKind::Candlestick`), Kagi lines
/// (`SeriesKind::Kagi`) or Point & Figure columns (`SeriesKind::PointFigure`),
/// with volume, at a spacing that shows the shapes.
///
/// # Errors
///
/// [`Error::InvalidArgument`] for another kind; the chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::transform_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// transform_scene(&mut chart, 0, SeriesKind::Kagi)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn transform_scene(chart: &mut Chart, start_time: Timestamp, kind: SeriesKind) -> Result<()> {
    use crate::plugin::SeriesTransform;
    use crate::transform::{Kagi, PointFigure, Renko, Reversal};
    let (t, spacing): (Box<dyn SeriesTransform>, f64) = match kind {
        SeriesKind::Candlestick => (Box::new(Renko::fixed(0.5)?), 8.0),
        SeriesKind::Kagi => (Box::new(Kagi::new(Reversal::Fixed(0.8))?), 10.0),
        SeriesKind::PointFigure => (Box::new(PointFigure::new(0.5, 3)?), 14.0),
        _ => return Err(Error::InvalidArgument("not a transform scene")),
    };
    let pane = chart.main_pane();
    let id = chart.add_transform_series(pane, t, SeriesOptions::default())?;
    chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(id))?;
    chart.set_data(
        id,
        SeriesData::Ohlc(random_walk(21, 3_000, start_time, 60_000)?),
    )?;
    chart.time_scale().set_bar_spacing(spacing);
    Ok(())
}

/// Golden scene for `Points` series: 120 candles with a SAR-like
/// series of 3 px dots below the lows in up runs and above the highs in down
/// runs (with a few gaps), and every tenth bar marked by an 8 px orange dot.
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::points_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// points_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn points_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    let pane = chart.main_pane();
    let bars = random_walk(13, 120, start_time, 60_000)?;
    let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
    let dots = |width: f32, color: Option<Rgba>| SeriesOptions {
        color,
        line_width: Css(width),
        price_line_visible: false,
        last_value_visible: false,
        ..SeriesOptions::default()
    };
    let sar = chart.add_series(
        pane,
        SeriesKind::Points,
        dots(3.0, Some(Rgba::hex(0x0029_62FF))),
    )?;
    let marks = chart.add_series(
        pane,
        SeriesKind::Points,
        dots(8.0, Some(Rgba::hex(0x00FF_9800))),
    )?;
    let n = bars.time.len();
    let sar_values = (0..n)
        .map(|i| match i {
            40..=43 => f64::NAN,
            _ if (i / 15) % 2 == 0 => bars.low[i] - 0.4,
            _ => bars.high[i] + 0.4,
        })
        .collect();
    let mark_values = (0..n)
        .map(|i| if i % 10 == 5 { bars.close[i] } else { f64::NAN })
        .collect();
    let value = |v: Vec<f64>| {
        SeriesData::Value(ValueColumns {
            time: bars.time.clone(),
            value: v,
            color: None,
        })
    };
    chart.set_data(sar, value(sar_values))?;
    chart.set_data(marks, value(mark_values))?;
    chart.set_data(candles, SeriesData::Ohlc(bars))?;
    chart.time_scale().set_bar_spacing(6.0);
    Ok(())
}

/// Golden scene for bands: 200 candles with a band between two lines
/// around the closes (default fill, under the candles), and below them a
/// pane with two crossing lines filled green where the first is above and
/// red where it is below, with a gap.
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::band_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// band_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn band_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::chart::BandOptions;
    let main = chart.main_pane();
    let bars = random_walk(17, 200, start_time, 60_000)?;
    let n = bars.time.len();
    let line = |chart: &mut Chart, pane, color: u32| {
        chart.add_series(
            pane,
            SeriesKind::Line,
            SeriesOptions {
                color: Some(Rgba::hex(color)),
                line_width: Css(1.0),
                price_line_visible: false,
                last_value_visible: false,
                ..SeriesOptions::default()
            },
        )
    };
    let values = |v: Vec<f64>| {
        SeriesData::Value(ValueColumns {
            time: bars.time.clone(),
            value: v,
            color: None,
        })
    };
    let upper = line(chart, main, 0x0029_62FF)?;
    let lower = line(chart, main, 0x0029_62FF)?;
    let wave = |i: usize, period: f64| (i as f64 / period).sin();
    chart.set_data(
        upper,
        values((0..n).map(|i| bars.close[i] + 1.5 + wave(i, 9.0)).collect()),
    )?;
    chart.set_data(
        lower,
        values((0..n).map(|i| bars.close[i] - 1.5 - wave(i, 7.0)).collect()),
    )?;
    chart.add_band(upper, lower, BandOptions::default())?;
    let candles = chart.add_series(main, SeriesKind::Candlestick, SeriesOptions::default())?;
    chart.set_data(candles, SeriesData::Ohlc(bars.clone()))?;

    let pane = chart.add_pane(PaneOptions::default());
    let a = line(chart, pane, 0x0026_A69A)?;
    let b = line(chart, pane, 0x00EF_5350)?;
    let gap = |i: usize, v: f64| if (120..124).contains(&i) { f64::NAN } else { v };
    chart.set_data(
        a,
        values((0..n).map(|i| gap(i, 2.0 * wave(i, 11.0))).collect()),
    )?;
    chart.set_data(
        b,
        values(
            (0..n)
                .map(|i| gap(i, 1.5 * (i as f64 / 17.0).cos()))
                .collect(),
        ),
    )?;
    chart.add_band(
        a,
        b,
        BandOptions {
            fill: Some(Rgba::hex(0x0026_A69A).with_alpha(0x55)),
            fill_crossed: Some(Rgba::hex(0x00EF_5350).with_alpha(0x55)),
        },
    )?;
    chart.time_scale().set_bar_spacing(4.0);
    Ok(())
}

/// A procedural avatar for image-marker tests: `n`×`n` pixels, a colored
/// disc with a lighter ring and a dark diagonal band, transparent corners
/// (so clipping, the border and premultiplied edges all show).
pub fn test_avatar(n: u32, hue: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    let c = (n as f32 - 1.0) * 0.5;
    let base = [hue, 255 - hue / 2, 128u8.wrapping_add(hue)];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            let r = (dx * dx + dy * dy).sqrt() / (n as f32 * 0.5);
            let px = if r > 1.0 {
                [0, 0, 0, 0]
            } else if r > 0.75 {
                [255, 255, 255, 220]
            } else if (dx - dy).abs() < n as f32 * 0.12 {
                [20, 24, 32, 255]
            } else {
                [base[0], base[1], base[2], 255]
            };
            out.extend_from_slice(&px);
        }
    }
    out
}

/// Fills an empty, sized chart with image markers and badges (r21):
/// letter badges, wallet avatars with buy/sell borders stacked on bars (one
/// stack folded by `marker_stack_limit`), a square image, a wide image and
/// a removed one.
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::image_markers_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// image_markers_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn image_markers_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::chart::{
        Marker,
        MarkerPosition::{AboveBar, BelowBar},
        MarkerShape::Square,
    };
    use crate::icon::IconImage;
    let bars = random_walk(23, 60, start_time, 60_000)?;
    let candles = chart.add_series(
        chart.main_pane(),
        SeriesKind::Candlestick,
        SeriesOptions {
            marker_stack_limit: Some(5),
            ..SeriesOptions::default()
        },
    )?;
    chart.set_data(candles, SeriesData::Ohlc(bars.clone()))?;
    let mut avatars = Vec::new();
    for (k, n) in [(0_u8, 48_u32), (70, 40), (140, 64), (210, 100)] {
        let px = test_avatar(n, k);
        avatars.push(chart.add_icon(IconImage {
            width: n,
            height: n,
            rgba: &px,
        })?);
    }
    // A 2:1 gradient: fitted into the middle half of the tile.
    let wide: Vec<u8> = (0..16_u8)
        .flat_map(|y| (0..32_u8).flat_map(move |x| [x * 8, y * 16, 200, 255]))
        .collect();
    let wide = chart.add_icon(IconImage {
        width: 32,
        height: 16,
        rgba: &wide,
    })?;
    let gone = chart.add_icon(IconImage {
        width: 1,
        height: 1,
        rgba: &[255, 255, 0, 255],
    })?;
    chart.remove_icon(gone)?;
    let (buy, sell) = (Rgba::hex(0x0026_A69A), Rgba::hex(0x00EF_5350));
    let t = |i: usize| bars.time[i];
    let badge = |i: usize, label: &str, color: u32, position| Marker {
        time: t(i),
        position,
        label: label.into(),
        color: Some(Rgba::hex(color)),
        size: Some(Css(18.0)),
        border: Some(Rgba::hex(color).with_alpha(160)),
        border_width: Css(1.5),
        ..Marker::default()
    };
    let avatar = |i: usize, k: usize, sold: bool| Marker {
        time: t(i),
        position: if sold { AboveBar } else { BelowBar },
        icon: Some(avatars[k % avatars.len()]),
        size: Some(Css(20.0)),
        border: Some(if sold { sell } else { buy }),
        ..Marker::default()
    };
    let mut markers = vec![
        badge(8, "DB", 0x002F_BF71, BelowBar),
        badge(8, "M", 0x003B_82F6, BelowBar),
        badge(16, "DS", 0x00D9_3B3B, AboveBar),
        Marker {
            time: t(24),
            shape: Square,
            icon: Some(avatars[2]),
            size: Some(Css(24.0)),
            text: "KOL".into(),
            ..Marker::default()
        },
        Marker {
            time: t(28),
            icon: Some(wide),
            size: Some(Css(22.0)),
            color: Some(Rgba::hex(0x0013_1722)),
            ..Marker::default()
        },
        Marker {
            time: t(32),
            icon: Some(gone),
            size: Some(Css(16.0)),
            ..Marker::default()
        },
    ];
    for (i, n) in [(40, 3), (43, 7), (46, 4), (49, 2)] {
        for k in 0..n {
            markers.push(avatar(i, k, true));
        }
        markers.push(avatar(i + 1, n, false));
    }
    chart.set_markers(candles, markers)?;
    chart.time_scale().set_bar_spacing(12.0);
    chart.scroll_to_realtime();
    Ok(())
}

/// The characters [`cjk_text_scene`] draws outside ASCII, for subsetting a
/// test font (the golden tests load a Noto Sans SC subset of exactly these).
pub const CJK_SCENE_CHARS: &str = "买入卖出狙聪明钱开发者加仓止盈";

/// Fills an empty, sized chart with non-ASCII text (r21): Chinese
/// marker texts, one-character Chinese badges, and a price line title that
/// ends in an emoji no test font has (drawn as `□`). Rendered with a CJK
/// fallback font holding [`CJK_SCENE_CHARS`].
///
/// # Errors
///
/// The chart's errors.
///
/// # Examples
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::cjk_text_scene;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
/// cjk_text_scene(&mut chart, 0)?;
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub fn cjk_text_scene(chart: &mut Chart, start_time: Timestamp) -> Result<()> {
    use crate::chart::{
        Marker,
        MarkerPosition::{AboveBar, BelowBar},
        MarkerShape::{ArrowDown, ArrowUp},
        PriceLineOptions,
    };
    let bars = random_walk(31, 60, start_time, 60_000)?;
    let candles = chart.add_series(
        chart.main_pane(),
        SeriesKind::Candlestick,
        SeriesOptions::default(),
    )?;
    chart.set_data(candles, SeriesData::Ohlc(bars.clone()))?;
    let t = |i: usize| bars.time[i];
    let text = |i: usize, s: &str, sold: bool| Marker {
        time: t(i),
        position: if sold { AboveBar } else { BelowBar },
        shape: if sold { ArrowDown } else { ArrowUp },
        color: Some(Rgba::hex(if sold { 0x00EF_5350 } else { 0x0026_A69A })),
        text: s.into(),
        ..Marker::default()
    };
    let badge = |i: usize, s: &str, color: u32| Marker {
        time: t(i),
        position: AboveBar,
        label: s.into(),
        color: Some(Rgba::hex(color)),
        size: Some(Css(18.0)),
        ..Marker::default()
    };
    let markers = vec![
        text(6, "买入", false),
        badge(12, "狙", 0x00D9_3B3B),
        text(18, "加仓 x2", false),
        badge(24, "聪", 0x002F_BF71),
        text(30, "开发者卖出", true),
        text(40, "止盈 +12%", true),
        badge(48, "DB", 0x003B_82F6),
    ];
    chart.set_markers(candles, markers)?;
    let last = bars.close.last().copied().unwrap_or(100.0);
    chart.create_price_line(
        candles,
        PriceLineOptions {
            price: last + 1.5,
            title: "聪明钱 🚀".into(),
            ..PriceLineOptions::default()
        },
    )?;
    chart.time_scale().set_bar_spacing(12.0);
    chart.scroll_to_realtime();
    Ok(())
}
