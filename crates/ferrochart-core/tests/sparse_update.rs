//! Real-time updates of sparse line-like series:
//! the slots filled by interpolation before the last point move with it, so
//! `update_last` leaves the series as a fresh `set_data` of the same points
//! would.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_core::chart::{SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, DataChange, OhlcColumns, SeriesId, ValueColumns};
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::{Chart, ChartOptions};

fn value_at(chart: &Chart, id: SeriesId, logical: i64) -> Option<f64> {
    let view = chart.data_view().ohlc(id, 0)?;
    view.close.at(view.first, logical)
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

/// Candles on three minutes, and a `kind` series given only the first and
/// the last of them.
fn chart_with(kind: SeriesKind) -> (Chart, SeriesId) {
    let mut chart = Chart::new(ChartOptions::default());
    let pane = chart.main_pane();
    let candles = chart
        .add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())
        .unwrap();
    let series = chart
        .add_series(pane, kind, SeriesOptions::default())
        .unwrap();
    chart
        .set_data(
            candles,
            SeriesData::Ohlc(OhlcColumns {
                time: vec![0, 60_000, 120_000],
                open: vec![100.0; 3],
                high: vec![101.0; 3],
                low: vec![99.0; 3],
                close: vec![100.0; 3],
                volume: None,
            }),
        )
        .unwrap();
    chart
        .set_data(
            series,
            SeriesData::Value(ValueColumns {
                time: vec![0, 120_000],
                value: vec![10.0, 30.0],
                color: None,
            }),
        )
        .unwrap();
    (chart, series)
}

#[test]
fn update_last_moves_interpolated_slots() {
    for kind in [SeriesKind::Line, SeriesKind::Area, SeriesKind::Baseline] {
        let (mut chart, series) = chart_with(kind);
        assert_eq!(value_at(&chart, series, 1), Some(20.0));
        let change = chart.update_last(series, tick(120_000, 50.0)).unwrap();
        assert_eq!(change, DataChange::Replace, "{kind:?}");
        assert_eq!(value_at(&chart, series, 2), Some(50.0));
        assert_eq!(value_at(&chart, series, 1), Some(30.0), "{kind:?}");
    }
}

#[test]
fn histograms_keep_their_gaps() {
    let (mut chart, series) = chart_with(SeriesKind::Histogram);
    let change = chart.update_last(series, tick(120_000, 50.0)).unwrap();
    assert_eq!(change, DataChange::UpdateLast);
    assert!(value_at(&chart, series, 1).is_none_or(f64::is_nan));
}
