//! Order flow in JS (rules in `docs/book/src/orderflow.md`): `addFootprint`,
//! `addHeatmap`, `addTpo` and the
//! series' `appendTrades`, `appendDepth` and `appendBars`. With WebGPU the
//! heatmap draws through its compute shader; with WebGL2 through its
//! portable path (same output).
// JSDoc tags (`@param`) are for the generated typings; errors are thrown.
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use ferrochart_core::chart::SeriesOptions;
use ferrochart_core::data::{OhlcColumns, PaneId};
use ferrochart_core::plugin::CustomSeries;
use ferrochart_orderflow::{
    DepthSnapshot, Footprint, Heatmap, TickColumns, Tpo, append_bars, append_depth, append_trades,
};
use js_sys::{Array, Float64Array, Int8Array};
use wasm_bindgen::prelude::*;

use crate::api::series;
use crate::js;
use crate::orderflow::{self as of, V};
use crate::web::{FerroChart, Series, f64_column, js_err, time_column};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** Options of `addFootprint` (docs: order flow rules). */
export interface FootprintOptions {
  /** Minimum price increment. */
  tickSize: number;
  /** Candle period, ms: columns start at its multiples (UTC). */
  timeframeMs: number;
  /** Ticks per row. Default 1. */
  rowSize?: number;
  /** Default `'bidAsk'`. */
  mode?: 'bidAsk' | 'delta' | 'volume';
  /** Diagonal imbalance ratio (≥ 1). Default 3; `null` turns imbalances off. */
  imbalanceRatio?: number | null;
  /** Outline each column's point of control. Default true. */
  showPoc?: boolean;
  /** Default: the theme's candle and text colors. */
  colors?: { buy?: string; sell?: string; volume?: string; text?: string; poc?: string };
}

/** Options of `addHeatmap`. */
export interface HeatmapOptions {
  tickSize: number;
  /** Column length, ms (e.g. 1000; may be finer than the candles). */
  columnMs: number;
  /** Order book snapshots (`appendDepth`) or trades (`appendTrades`). */
  source: 'depth' | 'trades';
  rowSize?: number;
  /** Default `'log'`. */
  scale?: 'log' | 'linear';
  /** Default `{ percentile: 0.99 }` of the visible cells. */
  range?: { percentile: number } | { lo: number; hi: number };
  /** Whether the heatmap takes part in auto scaling. Default false. */
  autoScale?: boolean;
  /** Five CSS colors from the smallest to the largest value. */
  colors?: [string, string, string, string, string];
}

/** Options of `addTpo`. */
export interface TpoOptions {
  tickSize: number;
  rowSize?: number;
  /** Period (one letter each), ms. Default 30 min. */
  periodMs?: number;
  /** Session length, ms (≤ 1 day). Default 24 h. */
  sessionMs?: number;
  /** Session start after UTC midnight, ms. Default 0. */
  sessionOffsetMs?: number;
  /** Share of the TPOs in the value area. Default 0.7. */
  valueArea?: number;
  colors?: { letters?: string; profile?: string; poc?: string; initialBalance?: string };
}

/** Trades of `appendTrades`: columns of the same length. */
export interface TradeData {
  /** UTC ms, non-decreasing. */
  time: BigInt64Array | Float64Array | number[];
  price: Float64Array | number[];
  size: Float64Array | number[];
  /** Aggressor: 1 buy, -1 sell, 0 unknown (tick rule). Default: all unknown. */
  side?: Int8Array | number[];
}

/** An order book snapshot of `appendDepth` (bids and asks, in book order). */
export interface DepthSnapshot {
  time: number | bigint;
  prices: Float64Array | number[];
  sizes: Float64Array | number[];
}
"#;

/// Reads `o` by dotted keys (`'colors.buy'`, `'colors.0'`).
fn reader(o: &JsValue) -> impl Fn(&str) -> V + '_ {
    move |key| {
        let mut v = o.clone();
        for part in key.split('.') {
            v = js::get(&v, part);
        }
        if v.is_undefined() {
            V::Missing
        } else if v.is_null() {
            V::Null
        } else if let Some(n) = v.as_f64() {
            V::Number(n)
        } else if let Some(s) = v.as_string() {
            V::Text(s)
        } else if let Some(b) = v.as_bool() {
            V::Bool(b)
        } else {
            V::Other
        }
    }
}

fn side_column(data: &JsValue) -> Result<Vec<f64>, JsError> {
    let v = js::get(data, "side");
    if v.is_undefined() || v.is_null() {
        return Ok(Vec::new());
    }
    if let Some(a) = v.dyn_ref::<Int8Array>() {
        return Ok(a.to_vec().into_iter().map(f64::from).collect());
    }
    if Array::is_array(&v) {
        return Ok(Float64Array::new(&v).to_vec());
    }
    Err(js_err(
        "'side' must be an Int8Array or an array of 1, -1 and 0",
    ))
}

#[wasm_bindgen]
impl FerroChart {
    fn add_order_flow(
        &self,
        make: impl FnOnce(&ferrochart_core::theme::Theme) -> Result<Box<dyn CustomSeries>, JsError>,
        pane: Option<u32>,
    ) -> Result<Series, JsError> {
        let theme = self.theme()?;
        let plugin = make(&theme)?;
        let id = self
            .with_chart(|c| {
                let pane = pane.map_or_else(|| c.main_pane(), PaneId);
                c.add_custom_series(pane, plugin, SeriesOptions::default())
            })?
            .map_err(js_err)?;
        Ok(series(&self.shared, id))
    }

    /// Adds a footprint: per candle, the sell and buy volume at each price
    /// level, fed with `appendTrades`. Shaded cells from 12 px per bar,
    /// numbers from 60 px: create the chart with a `maxBarSpacing` above 60
    /// to zoom in that far.
    ///
    /// @param {FootprintOptions} options
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addFootprint)]
    pub fn add_footprint(
        &self,
        #[wasm_bindgen(unchecked_param_type = "FootprintOptions")] options: &JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        self.add_order_flow(
            |theme| {
                let o = of::footprint(&reader(options), theme).map_err(js_err)?;
                Ok(Box::new(Footprint::new(o).map_err(js_err)?))
            },
            pane,
        )
    }

    /// Adds a heatmap of order book depth (`appendDepth`) or traded volume
    /// (`appendTrades`) per time column and price level, placed by time
    /// between the candles.
    ///
    /// @param {HeatmapOptions} options
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addHeatmap)]
    pub fn add_heatmap(
        &self,
        #[wasm_bindgen(unchecked_param_type = "HeatmapOptions")] options: &JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        self.add_order_flow(
            |_| {
                let o = of::heatmap(&reader(options)).map_err(js_err)?;
                Ok(Box::new(Heatmap::new(o).map_err(js_err)?))
            },
            pane,
        )
    }

    /// Adds a TPO (market profile) series, one slot per session, fed with
    /// candles through `appendBars`. Letters need wide slots: create the
    /// chart with a large `maxBarSpacing`; otherwise histograms are drawn.
    ///
    /// @param {TpoOptions} options
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addTpo)]
    pub fn add_tpo(
        &self,
        #[wasm_bindgen(unchecked_param_type = "TpoOptions")] options: &JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        self.add_order_flow(
            |theme| {
                let o = of::tpo(&reader(options), theme).map_err(js_err)?;
                Ok(Box::new(Tpo::new(o).map_err(js_err)?))
            },
            pane,
        )
    }
}

#[wasm_bindgen]
impl Series {
    /// Adds trades to a footprint or a trade heatmap. Trades in the last
    /// column update it; later ones open new columns.
    ///
    /// @param {TradeData} trades
    #[wasm_bindgen(js_name = appendTrades)]
    pub fn append_trades(
        &self,
        #[wasm_bindgen(unchecked_param_type = "TradeData")] trades: &JsValue,
    ) -> Result<(), JsError> {
        let req = |k| f64_column(trades, k, true).map(Option::unwrap_or_default);
        let side = of::sides(&side_column(trades)?)
            .map_err(|i| js_err(format!("side[{i}] must be 1, -1 or 0")))?;
        let columns = TickColumns {
            time: time_column(trades)?,
            price: req("price")?,
            size: req("size")?,
            side,
        };
        self.with_chart(|c| append_trades(c, self.id, &columns))?
            .map_err(js_err)
    }

    /// Adds an order book snapshot to a depth heatmap: within a column each
    /// level keeps its largest size.
    ///
    /// @param {DepthSnapshot} snapshot
    #[wasm_bindgen(js_name = appendDepth)]
    pub fn append_depth(
        &self,
        #[wasm_bindgen(unchecked_param_type = "DepthSnapshot")] snapshot: &JsValue,
    ) -> Result<(), JsError> {
        let req = |k| f64_column(snapshot, k, true).map(Option::unwrap_or_default);
        let (prices, sizes) = (req("prices")?, req("sizes")?);
        let s = DepthSnapshot {
            time: js::time_ms(&js::get(snapshot, "time"), "time")?,
            prices: &prices,
            sizes: &sizes,
        };
        self.with_chart(|c| append_depth(c, self.id, &s))?
            .map_err(js_err)
    }

    /// Adds candles (time, high and low are used) to a TPO series. Bars in
    /// the last session update it (a live candle sent again widens its
    /// period); bars outside the sessions are skipped.
    ///
    /// @param {OhlcData} bars
    #[wasm_bindgen(js_name = appendBars)]
    pub fn append_bars(
        &self,
        #[wasm_bindgen(unchecked_param_type = "OhlcData")] bars: &JsValue,
    ) -> Result<(), JsError> {
        let req = |k| f64_column(bars, k, true).map(Option::unwrap_or_default);
        let high = req("high")?;
        let low = req("low")?;
        let n = high.len();
        let opt = |k| f64_column(bars, k, false).map(|c| c.unwrap_or_else(|| vec![f64::NAN; n]));
        let columns = OhlcColumns {
            time: time_column(bars)?,
            open: opt("open")?,
            high,
            low,
            close: opt("close")?,
            volume: None,
        };
        self.with_chart(|c| append_bars(c, self.id, &columns))?
            .map_err(js_err)
    }
}
