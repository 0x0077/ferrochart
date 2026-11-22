//! Indicator benchmarks: `update_last` with
//! all indicators attached, and the full and incremental cost of each one.
#![allow(clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, OhlcColumns, SeriesId};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::plugin::{Indicator, Placement};
use ferrochart_core::testing::random_walk;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_indicators as ind;
use ferrochart_indicators::{Source, VwapAnchor};

type Make = fn() -> Box<dyn Indicator>;

fn all() -> Vec<(&'static str, Make)> {
    vec![
        ("sma", || Box::new(ind::sma(20, Source::Close).unwrap())),
        ("ema", || Box::new(ind::ema(20, Source::Close).unwrap())),
        ("wma", || Box::new(ind::wma(20, Source::Close).unwrap())),
        ("hma", || Box::new(ind::hma(20, Source::Close).unwrap())),
        ("vwma", || Box::new(ind::vwma(20, Source::Close).unwrap())),
        ("dema", || Box::new(ind::dema(20, Source::Close).unwrap())),
        ("tema", || Box::new(ind::tema(20, Source::Close).unwrap())),
        ("volume_ma", || Box::new(ind::volume_ma(20).unwrap())),
        ("boll", || {
            Box::new(ind::boll(20, 2.0, Source::Close).unwrap())
        }),
        ("keltner", || Box::new(ind::keltner(20, 2.0, 10).unwrap())),
        ("donchian", || Box::new(ind::donchian(20).unwrap())),
        ("ichimoku", || {
            Box::new(ind::ichimoku(9, 26, 52, 26).unwrap())
        }),
        ("sar", || Box::new(ind::sar(0.02, 0.02, 0.2).unwrap())),
        ("supertrend", || Box::new(ind::supertrend(10, 3.0).unwrap())),
        ("vwap", || Box::new(ind::vwap(VwapAnchor::Day, 0).unwrap())),
        ("macd", || {
            Box::new(ind::macd(12, 26, 9, Source::Close).unwrap())
        }),
        ("rsi", || Box::new(ind::rsi(14, Source::Close).unwrap())),
        ("stoch", || Box::new(ind::stoch(14, 3, 3).unwrap())),
        ("stochrsi", || {
            Box::new(ind::stochrsi(14, 14, 3, 3, Source::Close).unwrap())
        }),
        ("cci", || Box::new(ind::cci(20).unwrap())),
        ("willr", || Box::new(ind::willr(14).unwrap())),
        ("mfi", || Box::new(ind::mfi(14).unwrap())),
        ("adx", || Box::new(ind::adx(14).unwrap())),
        ("atr", || Box::new(ind::atr(14).unwrap())),
        ("obv", || Box::new(ind::obv().unwrap())),
        ("cmf", || Box::new(ind::cmf(20).unwrap())),
        ("roc", || Box::new(ind::roc(10, Source::Close).unwrap())),
        ("momentum", || {
            Box::new(ind::momentum(10, Source::Close).unwrap())
        }),
        ("trix", || Box::new(ind::trix(15, Source::Close).unwrap())),
        ("aroon", || Box::new(ind::aroon(14).unwrap())),
    ]
}

fn chart(data: &OhlcColumns) -> (Chart, SeriesId) {
    let mut chart = Chart::new(ChartOptions::default());
    let id = chart
        .add_series(
            chart.main_pane(),
            SeriesKind::Candlestick,
            SeriesOptions::default(),
        )
        .unwrap();
    chart.set_data(id, SeriesData::Ohlc(data.clone())).unwrap();
    (chart, id)
}

fn attach(chart: &mut Chart, id: SeriesId, i: Box<dyn Indicator>) {
    let p = i.default_placement();
    chart.add_indicator(id, i, p).unwrap();
}

/// Ticks of the last bar.
fn ticker(data: &OhlcColumns) -> impl FnMut() -> BarUpdate {
    let last = data.time.len() - 1;
    let (time, open) = (data.time[last], data.open[last]);
    let mut i = 0_u32;
    move || {
        i = i.wrapping_add(1);
        let close = open + f64::from(i % 17) * 0.01 - 0.08;
        BarUpdate {
            time,
            open,
            high: open.max(close) + 0.05,
            low: open.min(close) - 0.05,
            close,
            volume: Some(1000.0 + f64::from(i % 100)),
        }
    }
}

/// P5: `update_last` on 1M bars, without and with all 30 indicators.
fn update_last(c: &mut Criterion) {
    let mut g = c.benchmark_group("update_last");
    let data = random_walk(3, 1_000_000, 0, 60_000).unwrap();
    let (mut bare, id) = chart(&data);
    let mut tick = ticker(&data);
    g.bench_function("1M_no_indicators", |b| {
        b.iter(|| bare.update_last(id, tick()).unwrap());
    });
    let (mut full, id) = chart(&data);
    for (_, make) in all() {
        attach(&mut full, id, make());
    }
    let mut tick = ticker(&data);
    g.bench_function("1M_30_indicators", |b| {
        b.iter(|| full.update_last(id, tick()).unwrap());
    });
    g.finish();
}

/// Each indicator alone: one real-time tick (incremental) on 100k bars.
fn incremental(c: &mut Criterion) {
    let mut g = c.benchmark_group("incremental");
    let data = random_walk(4, 100_000, 0, 60_000).unwrap();
    for (name, make) in all() {
        let (mut chart, id) = chart(&data);
        attach(&mut chart, id, make());
        let mut tick = ticker(&data);
        g.bench_function(name, |b| b.iter(|| chart.update_last(id, tick()).unwrap()));
    }
    g.finish();
}

/// Each indicator: attaching it to 1M bars (full computation and storing its
/// outputs).
fn full(c: &mut Criterion) {
    let mut g = c.benchmark_group("full_1M");
    g.sample_size(10);
    let data = random_walk(5, 1_000_000, 0, 60_000).unwrap();
    let (mut chart, id) = chart(&data);
    for (name, make) in all() {
        g.bench_function(name, |b| {
            b.iter(|| {
                let i = make();
                let p = match i.default_placement() {
                    Placement::NewPane => Placement::Overlay,
                    p => p,
                };
                let h = chart.add_indicator(id, i, p).unwrap();
                chart.remove_indicator(h).unwrap();
            });
        });
    }
    // All 30 attached, then the source replaced: the full recomputation
    // core schedules (in parallel with `--features ferrochart-core/rayon`).
    let (mut chart, id) = self::chart(&data);
    for (_, make) in all() {
        attach(&mut chart, id, make());
    }
    g.bench_function("set_data_30_indicators", |b| {
        b.iter(|| chart.set_data(id, SeriesData::Ohlc(data.clone())).unwrap());
    });
    g.finish();
}

criterion_group!(benches, update_last, incremental, full);
criterion_main!(benches);
