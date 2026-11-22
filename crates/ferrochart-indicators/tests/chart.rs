//! Indicators attached to a `Chart`: outputs land in
//! value series at the source's slots, stay equal to a full computation under
//! real-time updates, survive axis rebuilds, and follow the removal rules.
#![allow(clippy::unwrap_used)]

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, OhlcColumns, SeriesId, ValueColumns};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::plugin::{Indicator, Placement};
use ferrochart_core::testing::random_walk;
use ferrochart_core::{Chart, ChartOptions, Error};
use ferrochart_indicators as ind;
use ferrochart_indicators::{Source, VwapAnchor};

const HOUR: i64 = 3_600_000;

/// Every built-in indicator with its default parameters.
fn all() -> Vec<Box<dyn Indicator>> {
    let c = Source::Close;
    vec![
        Box::new(ind::sma(20, c).unwrap()),
        Box::new(ind::ema(20, c).unwrap()),
        Box::new(ind::wma(20, c).unwrap()),
        Box::new(ind::hma(20, c).unwrap()),
        Box::new(ind::vwma(20, c).unwrap()),
        Box::new(ind::dema(20, c).unwrap()),
        Box::new(ind::tema(20, c).unwrap()),
        Box::new(ind::volume_ma(20).unwrap()),
        Box::new(ind::boll(20, 2.0, c).unwrap()),
        Box::new(ind::keltner(20, 2.0, 10).unwrap()),
        Box::new(ind::donchian(20).unwrap()),
        Box::new(ind::ichimoku(9, 26, 52, 26).unwrap()),
        Box::new(ind::sar(0.02, 0.02, 0.2).unwrap()),
        Box::new(ind::supertrend(10, 3.0).unwrap()),
        Box::new(ind::vwap(VwapAnchor::Day, 0).unwrap()),
        Box::new(ind::macd(12, 26, 9, c).unwrap()),
        Box::new(ind::rsi(14, c).unwrap()),
        Box::new(ind::stoch(14, 3, 3).unwrap()),
        Box::new(ind::stochrsi(14, 14, 3, 3, c).unwrap()),
        Box::new(ind::cci(20).unwrap()),
        Box::new(ind::willr(14).unwrap()),
        Box::new(ind::mfi(14).unwrap()),
        Box::new(ind::adx(14).unwrap()),
        Box::new(ind::atr(14).unwrap()),
        Box::new(ind::obv().unwrap()),
        Box::new(ind::cmf(20).unwrap()),
        Box::new(ind::roc(10, c).unwrap()),
        Box::new(ind::momentum(10, c).unwrap()),
        Box::new(ind::trix(15, c).unwrap()),
        Box::new(ind::aroon(14).unwrap()),
    ]
}

fn slice(d: &OhlcColumns, from: usize, to: usize) -> OhlcColumns {
    OhlcColumns {
        time: d.time[from..to].to_vec(),
        open: d.open[from..to].to_vec(),
        high: d.high[from..to].to_vec(),
        low: d.low[from..to].to_vec(),
        close: d.close[from..to].to_vec(),
        volume: d.volume.as_ref().map(|v| v[from..to].to_vec()),
    }
}

fn bar(d: &OhlcColumns, i: usize) -> BarUpdate {
    BarUpdate {
        time: d.time[i],
        open: d.open[i],
        high: d.high[i],
        low: d.low[i],
        close: d.close[i],
        volume: d.volume.as_ref().map(|v| v[i]),
    }
}

/// A chart with candles, overlays on the main pane and oscillators below.
fn chart_with(data: OhlcColumns, indicators: Vec<Box<dyn Indicator>>) -> (Chart, Vec<SeriesId>) {
    let mut chart = Chart::new(ChartOptions::default());
    let candles = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart.set_data(candles, SeriesData::Ohlc(data)).unwrap();
    let mut outputs = Vec::new();
    for i in indicators {
        let placement = i.default_placement();
        let id = chart.add_indicator(candles, i, placement).unwrap();
        outputs.extend_from_slice(chart.indicator_series(id).unwrap());
    }
    (chart, outputs)
}

/// `(logical index, value)` of every point of a value series.
fn points(chart: &Chart, id: SeriesId) -> Vec<(i64, u64)> {
    let view = chart.data_view();
    let Some(v) = view.ohlc(id, 0) else {
        return Vec::new();
    };
    (0..v.close.len())
        .map(|i| (v.first + i as i64, v.close.get(i).unwrap().to_bits()))
        .collect()
}

/// Appends and real-time updates through the chart give exactly the
/// outputs of a chart built from the final data.
#[test]
fn realtime_updates_equal_a_fresh_chart() {
    let data = random_walk(3, 400, 1_704_067_200_000, HOUR).unwrap();
    let (mut live, out_live) = chart_with(slice(&data, 0, 250), all());
    let candles = SeriesId(0);
    // A batch append, then bar by bar with three ticks each.
    live.append(candles, SeriesData::Ohlc(slice(&data, 250, 300)))
        .unwrap();
    for i in 300..400 {
        let mut b = bar(&data, i);
        let (close, high, low) = (b.close, b.high, b.low);
        for k in [0.25, 0.75] {
            b.close = b.open + (close - b.open) * k;
            b.high = b.open.max(b.close).max(high * 0.999).min(high);
            b.low = b.open.min(b.close).min(low * 1.001).max(low);
            live.update_last(candles, b).unwrap();
        }
        live.update_last(candles, bar(&data, i)).unwrap();
    }
    let (fresh, out_fresh) = chart_with(data, all());
    assert_eq!(out_live.len(), out_fresh.len());
    assert_eq!(out_live.len(), 48);
    for (a, b) in out_live.iter().zip(&out_fresh) {
        let p = points(&live, *a);
        let finite = p
            .iter()
            .filter(|&&(_, v)| f64::from_bits(v).is_finite())
            .count();
        assert!(finite >= 150, "output series {a:?}: {finite} values");
        assert_eq!(p, points(&fresh, *b), "output series {a:?}");
    }
}

/// Replacing or prepending the source recomputes all indicators
/// level by level (in parallel with core's `rayon` feature): the outputs,
/// including an indicator on another indicator's output, equal a chart where
/// each indicator was computed alone after the data was set.
#[test]
fn full_recomputation_equals_one_by_one() {
    let data = random_walk(8, 600, 1_704_067_200_000, HOUR).unwrap();
    let chain = |chart: &mut Chart, outputs: &mut Vec<SeriesId>| {
        // RSI of the first output (SMA): a second level.
        let id = chart
            .add_indicator(
                outputs[0],
                Box::new(ind::rsi(14, Source::Close).unwrap()),
                Placement::NewPane,
            )
            .unwrap();
        outputs.extend_from_slice(chart.indicator_series(id).unwrap());
    };
    let (mut fresh, mut out_fresh) = chart_with(data.clone(), all());
    chain(&mut fresh, &mut out_fresh);

    let (mut live, mut out_live) = chart_with(slice(&data, 0, 50), all());
    chain(&mut live, &mut out_live);
    let candles = SeriesId(0);
    live.set_data(candles, SeriesData::Ohlc(slice(&data, 200, 600)))
        .unwrap();
    live.prepend(candles, SeriesData::Ohlc(slice(&data, 0, 200)))
        .unwrap();
    assert_eq!(out_live.len(), out_fresh.len());
    // Prepending moves logical indices; compare the values in order.
    let values =
        |c: &Chart, id: SeriesId| -> Vec<u64> { points(c, id).into_iter().map(|p| p.1).collect() };
    for (a, b) in out_live.iter().zip(&out_fresh) {
        assert_eq!(values(&live, *a), values(&fresh, *b), "output series {a:?}");
    }
    let last = *out_live.last().unwrap();
    assert!(
        points(&live, last)
            .iter()
            .any(|&(_, v)| f64::from_bits(v).is_finite())
    );
}

/// A time-axis rebuild recomputes the indicators; at the candles'
/// times the outputs do not change, between them lines interpolate.
#[test]
fn outputs_survive_an_axis_rebuild() {
    let data = random_walk(5, 120, 1_704_067_200_000, HOUR).unwrap();
    let (plain, plain_out) = chart_with(
        data.clone(),
        vec![Box::new(ind::sma(10, Source::Close).unwrap())],
    );
    let (mut merged, merged_out) = chart_with(
        data.clone(),
        vec![Box::new(ind::sma(10, Source::Close).unwrap())],
    );
    let line = merged
        .add_series(
            merged.main_pane(),
            SeriesKind::Line,
            SeriesOptions::default(),
        )
        .unwrap();
    // Half-hour offsets: a slot between every two candles.
    let time: Vec<i64> = data.time[..60].iter().map(|t| t + HOUR / 2).collect();
    let value = vec![100.0; time.len()];
    merged
        .set_data(
            line,
            SeriesData::Value(ValueColumns {
                time,
                value,
                color: None,
            }),
        )
        .unwrap();
    let view_m = merged.data_view();
    let index_m = view_m.time_index();
    let view_p = plain.data_view();
    let index_p = view_p.time_index();
    let (sm, sp) = (
        view_m.ohlc(merged_out[0], 0).unwrap(),
        view_p.ohlc(plain_out[0], 0).unwrap(),
    );
    let at = |v: &ferrochart_core::data::OhlcLevelView<'_>, j: i64| {
        v.close.get((j - v.first) as usize).unwrap()
    };
    for &t in &data.time {
        let a = at(&sm, index_m.index_of(t).unwrap());
        let b = at(&sp, index_p.index_of(t).unwrap());
        assert_eq!(a.to_bits(), b.to_bits(), "time {t}");
    }
    // An inserted slot between two valid outputs is their midpoint.
    let t = data.time[30] + HOUR / 2;
    let j = index_m.index_of(t).unwrap();
    let (l, r) = (at(&sm, j - 1), at(&sm, j + 1));
    assert!((at(&sm, j) - f64::midpoint(l, r)).abs() < 1e-9);
}

/// Removal rules, chained indicators, volume checks.
#[test]
fn indicators_follow_the_removal_rules() {
    let data = random_walk(9, 200, 0, HOUR).unwrap();
    let (mut chart, _) = chart_with(data, Vec::new());
    let candles = SeriesId(0);
    assert_eq!(chart.panes().count(), 1);
    let rsi = chart
        .add_indicator(
            candles,
            Box::new(ind::rsi(14, Source::Close).unwrap()),
            Placement::NewPane,
        )
        .unwrap();
    assert_eq!(chart.panes().count(), 2);
    let rsi_line = chart.indicator_series(rsi).unwrap()[0];
    // An indicator on an indicator: the SMA of the RSI, on the RSI's pane.
    let rsi_pane = chart.panes().nth(1).unwrap();
    let smooth = chart
        .add_indicator(
            rsi_line,
            Box::new(ind::sma(5, Source::Close).unwrap()),
            Placement::Pane(rsi_pane),
        )
        .unwrap();
    let smooth_line = chart.indicator_series(smooth).unwrap()[0];
    assert!(!points(&chart, smooth_line).is_empty());
    // Outputs cannot be removed on their own.
    assert!(matches!(
        chart.remove_series(rsi_line),
        Err(Error::InvalidArgument(_))
    ));
    // Removing the RSI removes the chained SMA and the RSI's pane.
    chart.remove_indicator(rsi).unwrap();
    assert!(chart.indicator_series(smooth).is_none());
    assert_eq!(chart.panes().count(), 1);
    assert_eq!(chart.remove_indicator(rsi), Err(Error::UnknownId));
    // Removing the source removes its indicators.
    let sma = chart
        .add_indicator(
            candles,
            Box::new(ind::sma(5, Source::Close).unwrap()),
            Placement::Overlay,
        )
        .unwrap();
    chart.remove_series(candles).unwrap();
    assert!(chart.indicator_series(sma).is_none());
    // Volume: refused on a line, accepted on candles with volume.
    let line = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Line,
            SeriesOptions::default(),
        )
        .unwrap();
    assert!(matches!(
        chart.add_indicator(line, Box::new(ind::obv().unwrap()), Placement::NewPane),
        Err(Error::InvalidArgument(_))
    ));
    // Price indicators work on value series.
    chart
        .set_data(
            line,
            SeriesData::Value(ValueColumns {
                time: (0..50).collect(),
                value: (0..50).map(f64::from).collect(),
                color: None,
            }),
        )
        .unwrap();
    let sma = chart
        .add_indicator(
            line,
            Box::new(ind::sma(10, Source::Close).unwrap()),
            Placement::Overlay,
        )
        .unwrap();
    let out = chart.indicator_series(sma).unwrap()[0];
    let last = points(&chart, out).last().map(|&(_, v)| f64::from_bits(v));
    assert_eq!(last, Some(44.5));
}

/// The constructors validate their parameters.
#[test]
fn constructors_validate_parameters() {
    assert!(ind::sma(0, Source::Close).is_err());
    assert!(ind::hma(1, Source::Close).is_err());
    assert!(ind::boll(20, f64::NAN, Source::Close).is_err());
    assert!(ind::sar(0.3, 0.02, 0.2).is_err());
    assert!(ind::keltner(20, -1.0, 10).is_err());
    let m = ind::macd(12, 26, 9, Source::Close).unwrap();
    assert_eq!(m.warmup(), 33);
    assert_eq!(m.outputs().len(), 3);
    assert_eq!(m.default_placement(), Placement::NewPane);
    assert!(ind::vwma(20, Source::Close).unwrap().needs_volume());
}

/// BOLL, Keltner and Ichimoku fill between their lines, SAR is
/// drawn as dots, and the volume MA sits on the volume's pane and scale.
#[test]
fn bands_points_and_the_volume_placement() {
    use ferrochart_core::frame::{DrawCmd, LayerId};
    use ferrochart_core::geom::SizeCss;
    use ferrochart_core::testing::MonoMeasurer;

    let mut chart = Chart::new(ChartOptions::default());
    chart.resize(SizeCss { w: 800.0, h: 450.0 }, 1.0);
    let main = chart.main_pane();
    let candles = chart
        .add_series(main, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    chart
        .add_series(main, SeriesKind::Volume, SeriesOptions::volume_of(candles))
        .unwrap();
    chart
        .set_data(
            candles,
            SeriesData::Ohlc(random_walk(4, 300, 0, 60_000).unwrap()),
        )
        .unwrap();
    let mut add = |ind: Box<dyn Indicator>| {
        let p = ind.default_placement();
        chart.add_indicator(candles, ind, p).unwrap()
    };
    let boll = add(Box::new(ind::boll(20, 2.0, Source::Close).unwrap()));
    let keltner = add(Box::new(ind::keltner(20, 2.0, 10).unwrap()));
    let ichimoku = add(Box::new(ind::ichimoku(9, 26, 52, 26).unwrap()));
    let donchian = add(Box::new(ind::donchian(20).unwrap()));
    let sar = add(Box::new(ind::sar(0.02, 0.02, 0.2).unwrap()));
    let vma = add(Box::new(ind::volume_ma(20).unwrap()));
    for id in [boll, keltner, ichimoku] {
        assert_eq!(chart.indicator_bands(id).map(<[_]>::len), Some(1));
    }
    assert_eq!(chart.indicator_bands(donchian).map(<[_]>::len), Some(0));
    let cloud = chart.indicator_bands(ichimoku).unwrap()[0];
    let cloud_opts = *chart.band_options(cloud).unwrap();
    assert!(cloud_opts.fill.is_some() && cloud_opts.fill_crossed.is_some());
    assert_ne!(cloud_opts.fill, cloud_opts.fill_crossed);
    let sar_out = chart.indicator_series(sar).unwrap()[0];
    assert_eq!(chart.series_kind(sar_out), Some(SeriesKind::Points));
    // The volume MA: on the main pane (where the volume is), no new pane.
    let vma_out = chart.indicator_series(vma).unwrap()[0];
    assert_eq!(chart.series_pane(vma_out), Some(main));
    assert_eq!(chart.panes().count(), 1);

    chart.build_frame(0.0, &MonoMeasurer);
    let draws: Vec<_> = chart.frame().layers[LayerId::Main as usize]
        .cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Series(s) => Some(*s),
            _ => None,
        })
        .collect();
    let volume = draws.iter().find(|d| d.kind == SeriesKind::Volume).unwrap();
    let line = draws.iter().find(|d| d.series == vma_out).unwrap();
    assert_eq!(
        line.y_transform, volume.y_transform,
        "the volume MA uses the volume scale"
    );
    let candle = draws
        .iter()
        .find(|d| d.kind == SeriesKind::Candlestick)
        .unwrap();
    assert_ne!(candle.y_transform, volume.y_transform);
    assert_eq!(
        draws.iter().filter(|d| d.kind == SeriesKind::Band).count(),
        3
    );

    chart.remove_indicator(boll).unwrap();
    assert_eq!(chart.indicator_bands(boll), None);
    assert_eq!(chart.band_options(cloud), Some(&cloud_opts));

    // Without a volume series, the volume MA gets a new pane.
    let mut chart = Chart::new(ChartOptions::default());
    let c = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart
        .set_data(c, SeriesData::Ohlc(random_walk(4, 300, 0, 60_000).unwrap()))
        .unwrap();
    chart
        .add_indicator(c, Box::new(ind::volume_ma(20).unwrap()), Placement::Volume)
        .unwrap();
    assert_eq!(chart.panes().count(), 2);
}
