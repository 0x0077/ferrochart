//! The API of the browser binding: series of every
//! kind with options, panes, price scales, price lines, markers, indicators,
//! drawings, multi-chart sync and saved state.
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use ferrochart_core::Chart;
use ferrochart_core::chart::{BandOptions, PriceLineOptions, SeriesOptions};
use ferrochart_core::data::{
    BandId, DrawingId, IndicatorId, LayerPluginId, PaneId, PriceLineId, SeriesId,
};
use ferrochart_core::drawing::DrawingData;
use ferrochart_core::frame::SeriesKind;
use ferrochart_core::pane::PaneOptions;
use ferrochart_core::plugin::Placement;
use ferrochart_core::price_scale::ScaleSide;
use js_sys::{Array, Object};
use wasm_bindgen::prelude::*;

use crate::js;
use crate::params;
use crate::plugins;
use crate::web::{FerroChart, Peers, Series, Shared, js_err, schedule, set, with_shared};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** Series kinds of `FerroChart.addSeries`. */
export type SeriesKind = 'candlestick' | 'ohlc' | 'heikinAshi' | 'line' | 'area' | 'baseline' | 'histogram' | 'points';
/** A CSS color: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()` or `'transparent'` (color names are not accepted). */
export type Color = string;
export type LineStyle = 'solid' | 'dashed' | 'dotted';

/** Columns of `Series.setData` for line, area, baseline, histogram and points series. */
export interface ValueData {
  time: BigInt64Array | Float64Array | number[];
  /** `NaN` is a gap. */
  value: Float64Array | number[];
  /**
   * Per-point colors (`null`: the series color), or `0xRRGGBBAA` values.
   * Only histogram series draw them, and only at full detail (zoomed out far
   * enough that bars are merged, the series color is used). Other kinds
   * ignore them.
   */
  color?: (Color | null)[] | Uint32Array;
}

/** One point for `Series.update` of a single-value series. */
export interface ValuePoint {
  /** UTC milliseconds or a `Date`. */
  time: number | bigint | Date;
  value: number;
}

/** Series options. `null` colors use the theme. */
export interface SeriesOptions {
  /** Line color; up candles and bars; histogram bars; baseline line above the base. */
  color?: Color | null;
  /** Down candles and bars; baseline line below the base. */
  downColor?: Color | null;
  /** Area fill at the top; baseline fill above the base. */
  topFill?: Color | null;
  /** Area fill at the bottom; baseline fill below the base. */
  bottomFill?: Color | null;
  /** CSS px. */
  lineWidth?: number;
  /** Dash pattern of lines. Default `'solid'`. */
  lineStyle?: LineStyle;
  /** Step line. */
  step?: boolean;
  /** Baseline fill boundary, histogram bar base. */
  baseValue?: number;
  /** Label with the last value on the price axis. Default `true`; not drawn for volume series. */
  lastValueVisible?: boolean;
  /** Dotted horizontal line at the last value. Default `true`; not drawn for volume series. */
  priceLineVisible?: boolean;
  /** Display shift in bars. */
  shift?: number;
  /**
   * Decimals of price labels, 0 to 12. `null` (the default) is automatic:
   * at least 2 decimals and 4 significant digits of the visible prices, with
   * a subscript count of zeros for small prices (`0.0₄1234` is 0.00001234).
   */
  pricePrecision?: number | null;
  /**
   * Most markers stacked on one bar side; the rest fold into a "+k" badge
   * (`ClickTarget` `'markerOverflow'`). `null` (default): no limit.
   */
  markerStackLimit?: number | null;
}

export interface PriceLineOptions {
  /** Price, in the series' units. Default 0. */
  price?: number;
  /** Line and label color; `null` (default) uses the series' main color. */
  color?: Color | null;
  /** CSS px. Default 1. */
  lineWidth?: number;
  /** Default `'dashed'`. */
  lineStyle?: LineStyle;
  /** Label the price on the price axis. Default `true`. */
  axisLabelVisible?: boolean;
  /** Text above the line at the right edge of the plot; empty (default) for none. */
  title?: string;
}

export interface Marker {
  /**
   * UTC ms or a `Date`. Snaps to the first bar at or after it (the last bar
   * for later times); a marker on a gap is not drawn.
   */
  time: number | bigint | Date;
  /** Default `'aboveBar'`. */
  position?: 'aboveBar' | 'belowBar';
  /** Default `'circle'`. */
  shape?: 'circle' | 'square' | 'arrowUp' | 'arrowDown';
  /** `null` (default): the series' main color. */
  color?: Color | null;
  /** Text next to the marker; empty (default) for none. */
  text?: string;
  /** Diameter in CSS px, 4–64. `null` (default): the theme's size. */
  size?: number | null;
  /** Name of an image registered with `FerroChart.registerIcon`, drawn
   *  inside the shape (circle or square). `null` (default): none. */
  icon?: string | null;
  /** Up to 8 characters drawn inside the shape (circle or square), e.g. a
   *  badge `'DB'`; not with `icon`. Empty (default) for none. */
  label?: string;
  /** Ring inside the edge. `null` (default): none. */
  border?: Color | null;
  /** Ring width, CSS px. Default 2. */
  borderWidth?: number;
}

export interface PriceScaleOptions {
  mode?: 'normal' | 'logarithmic' | 'percentage' | 'indexedTo100';
  /** Setting `range` turns auto-scaling off. */
  autoScale?: boolean;
  inverted?: boolean;
  /** Fractions of the pane height. */
  margins?: { top: number; bottom: number };
  range?: { low: number; high: number };
}

export interface LogicalRange { from: number; to: number }

/** Transforms of `FerroChart.addTransformSeries` (https://ferrochart.com/docs/series). */
export type TransformName = 'renko' | 'rangeBars' | 'kagi' | 'pointFigure';
/**
 * Transform parameters (prices in the series' units):
 * renko `{ boxSize }` or `{ atr }` (ATR period) and `kind?`; rangeBars `{ range, kind? }`;
 * kagi `{ reversal }` (price) or `{ reversalPercent }`; pointFigure `{ boxSize, reversal? }` (boxes, default 3).
 */
export interface TransformParams {
  boxSize?: number;
  atr?: number;
  range?: number;
  /** Kagi: price; Point & Figure: boxes. */
  reversal?: number;
  reversalPercent?: number;
  /** Renko and range bars. Default `'candlestick'`. */
  kind?: 'candlestick' | 'ohlc';
}

/** Colors of `FerroChart.addBand`; `null` uses the default (the upper series' color at 15% opacity; `fillCrossed`: as `fill`). */
export interface BandOptions {
  /** Where the upper series is at or above the lower one. */
  fill?: Color | null;
  /** Where the upper series is below the lower one. */
  fillCrossed?: Color | null;
}

/** Where `addIndicator` puts the outputs: default per indicator, the source's pane, a new pane, the source's volume pane and scale, or a pane id. */
export type Placement = 'default' | 'overlay' | 'newPane' | 'volume' | number;

export interface Anchor {
  /** UTC ms (a `Date` is accepted too; anchors read back are numbers). */
  time: number | bigint | Date;
  price: number;
}

export interface DrawingStyle {
  /** Line color. Default `#2962ff`. */
  line?: Color;
  /** CSS px. Default 1. */
  lineWidth?: number;
  /** Default `'solid'`. */
  lineDash?: LineStyle;
  /** Fill of shapes and zones. Default `#2962ff` at 16% opacity. */
  fill?: Color;
  /** Text color. Default `#2962ff`. */
  text?: Color;
  /** CSS px. Default 12. */
  fontSize?: number;
}

export type DrawingParam = number | boolean | string | number[];

/** A drawing: kind (see `Drawings.kinds()`), anchors, style and tool parameters. */
export interface DrawingData {
  kind: string;
  anchors: Anchor[];
  style?: DrawingStyle;
  params?: Record<string, DrawingParam>;
}

export interface SyncOptions {
  /** Share the crosshair time. Default `true`. */
  crosshair?: boolean;
  /** Share the visible time range. Default `true`. */
  visibleRange?: boolean;
  /** Share the drawings of the main pane (charts of the same symbol). Default `false`. */
  drawings?: boolean;
}

/** A change of a synced drawing; `key` identifies the drawing in the group. */
export type DrawingSyncOp =
  | { type: 'add'; key: number; data: DrawingData; visible: boolean; locked: boolean }
  | { type: 'update'; key: number; data: DrawingData }
  | { type: 'setVisible'; key: number; visible: boolean }
  | { type: 'setLocked'; key: number; locked: boolean }
  | { type: 'remove'; key: number };

export type SyncMessage =
  | { type: 'crosshair'; time: number | null }
  | { type: 'visibleRange'; from: number; to: number };

/** Saved view, layout and drawings (`saveState`); plain JSON. */
export interface ChartState {
  version: number;
  barSpacing: number;
  rightOffset: number;
  crosshairMode: 'normal' | 'magnet' | 'hidden';
  panes: {
    stretch: number;
    priceScale: {
      mode: 'normal' | 'logarithmic' | 'percentage' | 'indexedTo100';
      inverted: boolean;
      margins: { top: number; bottom: number };
      range: { low: number; high: number } | null;
    };
  }[];
  drawings: { pane: number; visible: boolean; locked: boolean; data: DrawingData }[];
}
"#;

fn pane_id(v: Option<u32>, c: &Chart) -> PaneId {
    v.map_or_else(|| c.main_pane(), PaneId)
}

pub(crate) fn series(shared: &Rc<Shared>, id: SeriesId) -> Series {
    Series {
        shared: Rc::downgrade(shared),
        id,
    }
}

fn number_or_null(v: Option<f64>) -> JsValue {
    v.map_or(JsValue::NULL, JsValue::from_f64)
}

/// Removes the chart from its sync group in this page.
pub(crate) fn leave_group(shared: &Rc<Shared>) {
    if let Ok(mut state) = shared.state.try_borrow_mut() {
        state.scene.chart.leave_sync();
    }
    let peers = shared.sync_peers.borrow_mut().take();
    if let Some(peers) = peers {
        peers
            .borrow_mut()
            .retain(|w| w.upgrade().is_some_and(|s| !Rc::ptr_eq(&s, shared)));
        // The others drop the crosshair this chart showed.
        let members: Vec<Rc<Shared>> = peers.borrow().iter().filter_map(Weak::upgrade).collect();
        for m in &members {
            schedule(m);
        }
    }
}

#[wasm_bindgen]
impl FerroChart {
    fn add(
        &self,
        kind: SeriesKind,
        options: &JsValue,
        pane: Option<u32>,
    ) -> Result<Series, JsError> {
        let opts = js::series_options(SeriesOptions::default(), options)?;
        let id = self
            .with_chart(|c| {
                let pane = pane_id(pane, c);
                c.add_series(pane, kind, opts)
            })?
            .map_err(js_err)?;
        Ok(series(&self.shared, id))
    }

    /// Adds a series of `kind` to `pane` (default: the main pane).
    ///
    /// @param {SeriesKind} kind
    /// @param {SeriesOptions} [options]
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addSeries)]
    pub fn add_series(
        &self,
        #[wasm_bindgen(unchecked_param_type = "SeriesKind")] kind: &str,
        #[wasm_bindgen(unchecked_optional_param_type = "SeriesOptions")] options: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        let kind = params::series_kind(kind)
            .ok_or_else(|| js_err(format!("unknown series kind '{kind}'")))?;
        self.add(kind, &options.unwrap_or(JsValue::UNDEFINED), pane)
    }

    /// Adds a candlestick series (same as `addSeries('candlestick', …)`).
    ///
    /// @param {SeriesOptions} [options]
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addCandlestickSeries)]
    pub fn add_candlestick_series(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "SeriesOptions")] options: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        self.add(
            SeriesKind::Candlestick,
            &options.unwrap_or(JsValue::UNDEFINED),
            pane,
        )
    }

    /// Adds a series transform: `name` is `'renko'`, `'rangeBars'`,
    /// `'kagi'` or `'pointFigure'`, with the parameters described at
    /// <https://ferrochart.com/docs/series> (`{ boxSize: 0.5 }`, `{ atr: 14, kind: 'ohlc' }`,
    /// `{ range: 1 }`, `{ reversalPercent: 2 }`, `{ boxSize: 1, reversal: 3
    /// }`). The series takes OHLC bars (`setData`, `update`; a tick is a bar
    /// with four equal prices) and draws what the transform makes of them. A
    /// chart has at most one transform series and no other series but volume.
    ///
    /// @param {TransformName} name
    /// @param {TransformParams} params
    /// @param {SeriesOptions} [options]
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addTransformSeries)]
    pub fn add_transform_series(
        &self,
        #[wasm_bindgen(unchecked_param_type = "TransformName")] name: &str,
        #[wasm_bindgen(unchecked_param_type = "TransformParams")] params: &JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "SeriesOptions")] options: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] pane: Option<u32>,
    ) -> Result<Series, JsError> {
        let t = build_params(params, &format!("transform '{name}'"), |get| {
            params::transform(name, get)
        })?;
        let opts = js::series_options(
            SeriesOptions::default(),
            &options.unwrap_or(JsValue::UNDEFINED),
        )?;
        let id = self
            .with_chart(|c| {
                let pane = pane_id(pane, c);
                c.add_transform_series(pane, t, opts)
            })?
            .map_err(js_err)?;
        Ok(series(&self.shared, id))
    }

    /// Adds a volume histogram of the volume column of `source`, on `pane`
    /// (default: the main pane, in its bottom quarter).
    ///
    /// @param {Series} source
    /// @param {number} [pane]
    /// @returns {Series}
    #[wasm_bindgen(js_name = addVolumeSeries)]
    pub fn add_volume_series(&self, source: &Series, pane: Option<u32>) -> Result<Series, JsError> {
        let id = self
            .with_chart(|c| {
                let pane = pane_id(pane, c);
                c.add_series(
                    pane,
                    SeriesKind::Volume,
                    SeriesOptions::volume_of(source.id),
                )
            })?
            .map_err(js_err)?;
        Ok(series(&self.shared, id))
    }

    /// Id of the top pane.
    #[wasm_bindgen(getter, js_name = mainPane)]
    pub fn main_pane(&self) -> Result<u32, JsError> {
        self.with_chart(|c| c.main_pane().0)
    }

    /// Adds a pane below the others; returns its id. Throws for a stretch
    /// that is not a positive finite number (as `setPaneStretch`).
    ///
    /// @param {number} [stretch] height relative to the other panes (default 1)
    #[wasm_bindgen(js_name = addPane)]
    pub fn add_pane(&self, stretch: Option<f32>) -> Result<u32, JsError> {
        if let Some(s) = stretch.filter(|s| !(s.is_finite() && *s > 0.0)) {
            return Err(js_err(format!(
                "stretch must be a positive number, got {s}"
            )));
        }
        self.with_chart(|c| {
            c.add_pane(PaneOptions {
                stretch_factor: stretch.unwrap_or(1.0),
            })
            .0
        })
    }

    /// Removes a pane with its series and drawings (not the last one).
    #[wasm_bindgen(js_name = removePane)]
    pub fn remove_pane(&self, pane: u32) -> Result<(), JsError> {
        self.with_chart(|c| c.remove_pane(PaneId(pane)))?
            .map_err(js_err)
    }

    /// Registers an image under `name` for marker `icon`s and plugin `icon`
    /// commands. `source` is a URL (fetched; a cross-origin image needs CORS
    /// headers), an `HTMLImageElement`, `ImageBitmap`, `ImageData` or canvas.
    /// The image is scaled to fit 64×64 (aspect ratio kept). Registering a
    /// name again replaces its image, also in the markers using it. At most
    /// 1024 images per chart.
    ///
    /// @param {string} name
    /// @param {string | ImageBitmapSource} source
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = registerIcon, unchecked_return_type = "Promise<void>")]
    pub fn register_icon(
        &self,
        name: String,
        #[wasm_bindgen(unchecked_param_type = "string | ImageBitmapSource")] source: JsValue,
    ) -> js_sys::Promise {
        let shared = Rc::downgrade(&self.shared);
        wasm_bindgen_futures::future_to_promise(async move {
            if name.is_empty() {
                return Err(js_err("registerIcon: the name is empty").into());
            }
            let (width, height, rgba) = crate::icons::decode(source).await?;
            crate::host::with_weak(&shared, |s| -> Result<(), JsError> {
                let id = s
                    .chart
                    .add_icon(ferrochart_core::icon::IconImage {
                        width,
                        height,
                        rgba: &rgba,
                    })
                    .map_err(js_err)?;
                if let Some(old) = crate::icons::insert(s.icon_key, name, id) {
                    // Markers keep ids: point the ones using the old image
                    // at the new one, then drop the old image.
                    let chart = &mut s.chart;
                    s.icon_series.retain(|&series| {
                        let Some(markers) = chart.markers(series) else {
                            return false;
                        };
                        if markers.iter().any(|m| m.icon == Some(old)) {
                            let moved = markers
                                .iter()
                                .cloned()
                                .map(|mut m| {
                                    if m.icon == Some(old) {
                                        m.icon = Some(id);
                                    }
                                    m
                                })
                                .collect();
                            let _ = chart.set_markers(series, moved);
                        }
                        true
                    });
                    let _ = chart.remove_icon(old);
                }
                Ok(())
            })??;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Removes the image registered as `name`; markers using it draw their
    /// shape only. Returns whether there was one.
    #[wasm_bindgen(js_name = unregisterIcon)]
    pub fn unregister_icon(&self, name: &str) -> Result<bool, JsError> {
        self.shared.with_scene(|s| {
            crate::icons::remove(s.icon_key, name).is_some_and(|id| s.chart.remove_icon(id).is_ok())
        })
    }

    /// Adds a layer plugin: `plugin.build(ctx)` returns the commands
    /// to draw in every frame, in CSS px. `plugin.pane` (default: none, the
    /// whole chart) and `plugin.position` ('background', 'belowSeries',
    /// 'aboveSeries' (default) or 'overlay') are read once. Returns the
    /// layer id. Call `invalidateLayer` when the plugin's own content
    /// changes.
    ///
    /// @param {LayerPluginJs} plugin
    /// @returns {number}
    #[wasm_bindgen(js_name = addLayer)]
    pub fn add_layer(
        &self,
        #[wasm_bindgen(unchecked_param_type = "LayerPluginJs")] plugin: JsValue,
    ) -> Result<u32, JsError> {
        if !js::get(&plugin, "build").is_function() {
            return Err(js_err("a layer plugin needs a build(ctx) method"));
        }
        let position = match js::opt_string(&plugin, "position")? {
            Some(s) => plugins::layer_position(&s).ok_or_else(|| {
                js_err("position must be 'background', 'belowSeries', 'aboveSeries' or 'overlay'")
            })?,
            None => ferrochart_core::plugin::LayerPosition::AboveSeries,
        };
        let pane = js::opt_f64(&plugin, "pane")?
            .map(|p| {
                u32::try_from(p as i64)
                    .ok()
                    .filter(|_| p.fract() == 0.0)
                    .ok_or_else(|| js_err("invalid pane id"))
            })
            .transpose()?
            .map(PaneId);
        let layer = Box::new(plugins::JsLayer::new(plugin, position));
        let id = self
            .with_chart(|c| c.add_layer(pane, layer))?
            .map_err(js_err)?;
        Ok(id.0)
    }

    /// Fills the area between two single-value series of the same pane:
    /// `fill` where `upper` is at or above `lower`, `fillCrossed` where it is
    /// below (default: the upper series' color at 15% opacity). Returns the
    /// band id; the band is removed with either series.
    ///
    /// @param {Series} upper
    /// @param {Series} lower
    /// @param {BandOptions} [options]
    /// @returns {number}
    #[wasm_bindgen(js_name = addBand)]
    pub fn add_band(
        &self,
        upper: &Series,
        lower: &Series,
        #[wasm_bindgen(unchecked_optional_param_type = "BandOptions")] options: Option<JsValue>,
    ) -> Result<u32, JsError> {
        let opts = js::band_options(
            BandOptions::default(),
            &options.unwrap_or(JsValue::UNDEFINED),
        )?;
        let id = self
            .with_chart(|c| c.add_band(upper.id, lower.id, opts))?
            .map_err(js_err)?;
        Ok(id.0)
    }

    /// Changes some colors of a band; keys left out keep their value.
    ///
    /// @param {number} id
    /// @param {BandOptions} options
    #[wasm_bindgen(js_name = setBandOptions)]
    pub fn set_band_options(
        &self,
        id: u32,
        #[wasm_bindgen(unchecked_param_type = "BandOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        self.with_chart(|c| -> Result<(), JsError> {
            let base = *c
                .band_options(BandId(id))
                .ok_or_else(|| js_err("unknown band"))?;
            let opts = js::band_options(base, options)?;
            c.set_band_options(BandId(id), opts).map_err(js_err)
        })?
    }

    /// The colors of a band (`null` colors are the defaults); `null` for an
    /// unknown id.
    ///
    /// @param {number} id
    /// @returns {BandOptions | null}
    #[wasm_bindgen(js_name = bandOptions, unchecked_return_type = "BandOptions | null")]
    pub fn band_options(&self, id: u32) -> Result<JsValue, JsError> {
        self.with_chart(|c| {
            c.band_options(BandId(id))
                .map_or(JsValue::NULL, js::band_options_js)
        })
    }

    /// Removes a band.
    #[wasm_bindgen(js_name = removeBand)]
    pub fn remove_band(&self, id: u32) -> Result<(), JsError> {
        self.with_chart(|c| c.remove_band(BandId(id)))?
            .map_err(js_err)
    }

    /// Removes a layer plugin.
    #[wasm_bindgen(js_name = removeLayer)]
    pub fn remove_layer(&self, id: u32) -> Result<(), JsError> {
        self.with_chart(|c| c.remove_layer(LayerPluginId(id)))?
            .map_err(js_err)
    }

    /// Redraws a layer plugin after its own content changed.
    #[wasm_bindgen(js_name = invalidateLayer)]
    pub fn invalidate_layer(&self, id: u32) -> Result<(), JsError> {
        self.with_chart(|c| c.invalidate_layer(LayerPluginId(id)))?
            .map_err(js_err)
    }

    /// Pane ids, top to bottom.
    #[wasm_bindgen(unchecked_return_type = "number[]")]
    pub fn panes(&self) -> Result<Array, JsError> {
        self.with_chart(|c| c.panes().map(|p| JsValue::from(p.0)).collect())
    }

    /// Height of a pane relative to the others.
    #[wasm_bindgen(js_name = paneStretch)]
    pub fn pane_stretch(&self, pane: u32) -> Result<Option<f32>, JsError> {
        self.with_chart(|c| c.pane_stretch(PaneId(pane)))
    }

    /// Sets the height of a pane relative to the others.
    #[wasm_bindgen(js_name = setPaneStretch)]
    pub fn set_pane_stretch(&self, pane: u32, stretch: f32) -> Result<(), JsError> {
        self.with_chart(|c| c.set_pane_stretch(PaneId(pane), stretch))?
            .map_err(js_err)
    }

    /// The price scale of `pane` (default: the main pane).
    ///
    /// @returns {Required<PriceScaleOptions>}
    #[wasm_bindgen(js_name = priceScale, unchecked_return_type = "Required<PriceScaleOptions>")]
    pub fn price_scale(&self, pane: Option<u32>) -> Result<JsValue, JsError> {
        self.with_chart(|c| {
            let pane = pane_id(pane, c);
            let s = c.price_scale(pane, ScaleSide::Right)?;
            let o = Object::new();
            set(
                &o,
                "mode",
                &JsValue::from_str(params::scale_mode_name(s.mode())),
            );
            set(&o, "autoScale", &s.auto_scale().into());
            set(&o, "inverted", &s.inverted().into());
            let m = Object::new();
            set(&m, "top", &f64::from(s.margins().0).into());
            set(&m, "bottom", &f64::from(s.margins().1).into());
            set(&o, "margins", &m.into());
            let r = Object::new();
            set(&r, "low", &s.range().0.into());
            set(&r, "high", &s.range().1.into());
            set(&o, "range", &r.into());
            Some(JsValue::from(o))
        })?
        .ok_or_else(|| js_err("unknown pane"))
    }

    /// Changes the price scale of `pane` (default: the main pane). Absent
    /// keys keep their value. All or nothing: if a value is invalid, the call
    /// throws and the scale is left as it was. Setting `range` turns
    /// auto-scaling off, so `range` with `autoScale: true` throws.
    ///
    /// @param {PriceScaleOptions} options
    /// @param {number} [pane]
    #[wasm_bindgen(js_name = applyPriceScaleOptions)]
    pub fn apply_price_scale_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "PriceScaleOptions")] options: &JsValue,
        pane: Option<u32>,
    ) -> Result<(), JsError> {
        let mode = js::opt_string(options, "mode")?
            .map(|m| params::scale_mode(&m).ok_or_else(|| js_err(format!("unknown mode '{m}'"))))
            .transpose()?;
        let auto = js::opt_bool(options, "autoScale")?;
        let inverted = js::opt_bool(options, "inverted")?;
        let m = js::get(options, "margins");
        let margins = if m.is_object() {
            let top = js::opt_f64(&m, "top")?.ok_or_else(|| js_err("margins.top missing"))?;
            let bottom =
                js::opt_f64(&m, "bottom")?.ok_or_else(|| js_err("margins.bottom missing"))?;
            Some((top as f32, bottom as f32))
        } else {
            None
        };
        let r = js::get(options, "range");
        let range = if r.is_object() {
            let low = js::opt_f64(&r, "low")?.ok_or_else(|| js_err("range.low missing"))?;
            let high = js::opt_f64(&r, "high")?.ok_or_else(|| js_err("range.high missing"))?;
            Some((low, high))
        } else {
            None
        };
        if range.is_some() && auto == Some(true) {
            return Err(js_err("range turns autoScale off; pass one or the other"));
        }
        self.with_chart(|c| -> Result<(), JsError> {
            let pane = pane_id(pane, c);
            let s = c
                .price_scale(pane, ScaleSide::Right)
                .ok_or_else(|| js_err("unknown pane"))?;
            let before = (
                s.mode(),
                s.margins(),
                s.inverted(),
                s.range(),
                s.auto_scale(),
            );
            let apply = |s: &mut _| -> Result<(), JsError> {
                let s: &mut ferrochart_core::price_scale::PriceScale = s;
                if let Some(m) = mode {
                    s.set_mode(m).map_err(js_err)?;
                }
                if let Some((t, b)) = margins {
                    s.set_margins(t, b).map_err(js_err)?;
                }
                if let Some(i) = inverted {
                    s.set_inverted(i);
                }
                if let Some((lo, hi)) = range {
                    s.set_range(lo, hi).map_err(js_err)?;
                }
                if let Some(a) = auto {
                    s.set_auto_scale(a);
                }
                Ok(())
            };
            let result = apply(s);
            if result.is_err() {
                // Back to the state before the call (these values were valid).
                let (m, (t, b), i, (lo, hi), a) = before;
                let _ = s.set_mode(m);
                let _ = s.set_margins(t, b);
                s.set_inverted(i);
                if !a {
                    let _ = s.set_range(lo, hi);
                }
                s.set_auto_scale(a);
            }
            result
        })?
    }

    /// The crosshair mode.
    #[wasm_bindgen(getter, js_name = crosshairMode, unchecked_return_type = "'normal' | 'magnet' | 'hidden'")]
    pub fn crosshair_mode(&self) -> Result<String, JsError> {
        self.with_chart(|c| params::crosshair_mode_name(c.crosshair_mode()).to_owned())
    }

    /// Bar spacing in CSS px.
    #[wasm_bindgen(getter, js_name = barSpacing)]
    pub fn bar_spacing(&self) -> Result<f64, JsError> {
        let dpr = self.shared.state.try_borrow().map_or(1.0, |s| s.dpr);
        self.with_chart(|c| c.time_scale().bar_spacing() / dpr)
    }

    /// Empty space right of the last bar, in bars.
    #[wasm_bindgen(getter, js_name = rightOffset)]
    pub fn right_offset(&self) -> Result<f64, JsError> {
        self.with_chart(|c| c.time_scale().right_offset())
    }

    /// Sets the empty space right of the last bar, in bars (may be negative).
    #[wasm_bindgen(js_name = setRightOffset)]
    pub fn set_right_offset(&self, bars: f64) -> Result<(), JsError> {
        self.with_chart(|c| c.time_scale().set_right_offset(bars))
    }

    /// Logical indices of the first and last bar, `null` without data.
    /// Logical indices do not change when older data is prepended (the
    /// first one becomes negative).
    #[wasm_bindgen(js_name = dataRange, unchecked_return_type = "LogicalRange | null")]
    pub fn data_range(&self) -> Result<JsValue, JsError> {
        self.with_chart(|c| {
            let index = c.data_view().time_index();
            if index.is_empty() {
                return JsValue::NULL;
            }
            let o = Object::new();
            set(&o, "from", &JsValue::from_f64(index.first_index() as f64));
            set(&o, "to", &JsValue::from_f64((index.end_index() - 1) as f64));
            o.into()
        })
    }

    /// The visible logical range (plot edges, fractional).
    #[wasm_bindgen(js_name = visibleLogicalRange, unchecked_return_type = "LogicalRange")]
    pub fn visible_logical_range(&self) -> Result<JsValue, JsError> {
        self.with_chart(|c| {
            let (from, to) = c.time_scale().visible_range();
            let o = Object::new();
            set(&o, "from", &from.into());
            set(&o, "to", &to.into());
            o.into()
        })
    }

    /// Shows logical indices `from..to` across the plot (as far as the
    /// spacing limits allow); also before the first frame.
    #[wasm_bindgen(js_name = setVisibleLogicalRange)]
    pub fn set_visible_logical_range(&self, from: f64, to: f64) -> Result<(), JsError> {
        self.with_chart(|c| c.time_scale().set_visible_range(from, to))?
            .map_err(js_err)
    }

    /// Time (UTC ms) of the bar at logical index `index`, `null` outside the data.
    #[wasm_bindgen(js_name = timeAtIndex, unchecked_return_type = "number | null")]
    pub fn time_at_index(&self, index: f64) -> Result<JsValue, JsError> {
        self.with_chart(|c| {
            let t = c.data_view().time_index().time_at(index.round() as i64);
            number_or_null(t.map(|t| t as f64))
        })
    }

    /// Logical index of the bar at time `time` (UTC ms or a `Date`), `null`
    /// if no bar has it.
    #[wasm_bindgen(js_name = indexOfTime, unchecked_return_type = "number | null")]
    pub fn index_of_time(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | bigint | Date")] time: &JsValue,
    ) -> Result<JsValue, JsError> {
        let t = js::time_ms(time, "time")?;
        self.with_chart(|c| {
            number_or_null(c.data_view().time_index().index_of(t).map(|i| i as f64))
        })
    }

    /// x (CSS px from the canvas' left edge) of the center of logical index `index`.
    #[wasm_bindgen(js_name = logicalToCoordinate)]
    pub fn logical_to_coordinate(&self, index: f64) -> Result<f64, JsError> {
        let dpr = self.shared.state.try_borrow().map_or(1.0, |s| s.dpr);
        self.with_chart(|c| {
            let plot = c.layout().plot;
            (f64::from(plot.x) + f64::from(c.time_scale().index_to_x(index).0)) / dpr
        })
    }

    /// Logical index (fractional) at x (CSS px from the canvas' left edge).
    #[wasm_bindgen(js_name = coordinateToLogical)]
    pub fn coordinate_to_logical(&self, x: f64) -> Result<f64, JsError> {
        let dpr = self.shared.state.try_borrow().map_or(1.0, |s| s.dpr);
        self.with_chart(|c| {
            let plot = c.layout().plot;
            let px = x * dpr - f64::from(plot.x);
            c.time_scale()
                .x_to_index(ferrochart_core::geom::Px(px as f32))
        })
    }

    /// Attaches indicator `name` (e.g. `'sma'`, `'macd'`; the list and the
    /// parameters are at <https://ferrochart.com/docs/indicators>) computed
    /// from `source`. Parameters are in camelCase (`{ n: 50, source: 'hl2' }`,
    /// `{ fast: 12, slow: 26, signal: 9 }`); missing ones take the defaults.
    /// A key the indicator does not use throws, naming the accepted keys.
    ///
    /// @param {Series} source
    /// @param {string} name
    /// @param {Record<string, number | string>} [params]
    /// @param {Placement} [placement]
    /// @returns {Indicator}
    #[wasm_bindgen(js_name = addIndicator)]
    pub fn add_indicator(
        &self,
        source: &Series,
        name: &str,
        #[wasm_bindgen(unchecked_optional_param_type = "Record<string, number | string>")]
        params: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "Placement")] placement: Option<JsValue>,
    ) -> Result<Indicator, JsError> {
        let p = params.unwrap_or(JsValue::UNDEFINED);
        let ind = build_params(&p, &format!("indicator '{name}'"), |get| {
            params::indicator(name, get)
        })?;
        let placement = match placement {
            None => None,
            Some(v) if v.is_undefined() || v.is_null() => None,
            Some(v) => match (v.as_string().as_deref(), v.as_f64()) {
                (Some("default"), _) => None,
                (Some("overlay"), _) => Some(Placement::Overlay),
                (Some("newPane"), _) => Some(Placement::NewPane),
                (Some("volume"), _) => Some(Placement::Volume),
                (None, Some(p)) if p >= 0.0 && p.fract() == 0.0 => {
                    Some(Placement::Pane(PaneId(p as u32)))
                }
                _ => {
                    return Err(js_err(
                        "placement must be 'default', 'overlay', 'newPane', 'volume' or a pane id",
                    ));
                }
            },
        };
        let id = self
            .with_chart(|c| {
                use ferrochart_core::plugin::Indicator as _;
                let placement = placement.unwrap_or_else(|| ind.default_placement());
                c.add_indicator(source.id, Box::new(ind), placement)
            })?
            .map_err(js_err)?;
        Ok(Indicator {
            shared: Rc::downgrade(&self.shared),
            id,
        })
    }

    /// The drawings of this chart.
    #[wasm_bindgen(getter)]
    pub fn drawings(&self) -> Drawings {
        Drawings {
            shared: Rc::downgrade(&self.shared),
        }
    }

    /// Joins `group` (leaving the current one): crosshair and visible range
    /// follow the other charts of the group, by time.
    ///
    /// @param {SyncGroup} group
    /// @param {SyncOptions} [options]
    #[wasm_bindgen(js_name = joinSync)]
    pub fn join_sync(
        &self,
        group: &SyncGroup,
        #[wasm_bindgen(unchecked_optional_param_type = "SyncOptions")] options: Option<JsValue>,
    ) -> Result<(), JsError> {
        let opts = js::sync_options(&options.unwrap_or(JsValue::UNDEFINED))?;
        leave_group(&self.shared);
        self.with_chart(|c| c.join_sync(&group.group, opts))?;
        group.peers.borrow_mut().push(Rc::downgrade(&self.shared));
        *self.shared.sync_peers.borrow_mut() = Some(group.peers.clone());
        schedule(&self.shared);
        Ok(())
    }

    /// Leaves the sync group.
    #[wasm_bindgen(js_name = leaveSync)]
    pub fn leave_sync(&self) {
        leave_group(&self.shared);
        schedule(&self.shared);
    }

    /// What the chart shares with its group, `null` outside a group.
    #[wasm_bindgen(js_name = syncOptions, unchecked_return_type = "Required<SyncOptions> | null")]
    pub fn sync_options(&self) -> Result<JsValue, JsError> {
        self.with_chart(|c| c.sync_options().map_or(JsValue::NULL, js::sync_options_js))
    }

    /// Applies a message another chart posted (its `'sync'` event), e.g. one
    /// forwarded from a worker. It is not posted again.
    ///
    /// @param {SyncMessage} message
    #[wasm_bindgen(js_name = applySync)]
    pub fn apply_sync(
        &self,
        #[wasm_bindgen(unchecked_param_type = "SyncMessage")] message: &JsValue,
    ) -> Result<(), JsError> {
        let m = js::sync_message(message)?;
        self.with_chart(|c| c.apply_sync(m))
    }

    /// Takes the drawing sync operations this chart sent and the page has
    /// not taken yet (oldest first, at most 1024), e.g. to forward them to a
    /// chart in a worker with `applyDrawingSync`.
    ///
    /// @returns {DrawingSyncOp[]}
    #[wasm_bindgen(js_name = takeDrawingSyncOps, unchecked_return_type = "DrawingSyncOp[]")]
    pub fn take_drawing_sync_ops(&self) -> Result<Array, JsError> {
        let mut state = self
            .shared
            .state
            .try_borrow_mut()
            .map_err(|_| js_err("chart is busy"))?;
        let mut ops = Vec::new();
        state.scene.chart.take_drawing_sync_ops(&mut ops);
        state.scene.sync_ops.extend(ops);
        state.scene.sync_ops_reported = false;
        Ok(state
            .scene
            .sync_ops
            .drain(..)
            .map(|op| js::drawing_sync_op_js(&op))
            .collect())
    }

    /// Applies a drawing operation of another chart (not undoable, not sent
    /// again); received drawings go to the main pane.
    ///
    /// @param {DrawingSyncOp} op
    #[wasm_bindgen(js_name = applyDrawingSync)]
    pub fn apply_drawing_sync(
        &self,
        #[wasm_bindgen(unchecked_param_type = "DrawingSyncOp")] op: &JsValue,
    ) -> Result<(), JsError> {
        let op = js::drawing_sync_op(op)?;
        self.with_chart(|c| c.apply_drawing_sync(op))?
            .map_err(js_err)?;
        schedule(&self.shared);
        Ok(())
    }

    /// The synced drawings of this chart as `'add'` operations, for
    /// `loadDrawingSyncState` on a chart that joins from a worker.
    ///
    /// @returns {DrawingSyncOp[]}
    #[wasm_bindgen(js_name = drawingSyncState, unchecked_return_type = "DrawingSyncOp[]")]
    pub fn drawing_sync_state(&self) -> Result<Array, JsError> {
        self.with_chart(|c| {
            c.drawing_sync_state()
                .iter()
                .map(js::drawing_sync_op_js)
                .collect()
        })
    }

    /// Makes the synced drawings match `state` (from `drawingSyncState`).
    /// Throws, leaving the chart unchanged, for an invalid state.
    ///
    /// @param {DrawingSyncOp[]} state
    #[wasm_bindgen(js_name = loadDrawingSyncState)]
    pub fn load_drawing_sync_state(
        &self,
        #[wasm_bindgen(unchecked_param_type = "DrawingSyncOp[]")] state: &JsValue,
    ) -> Result<(), JsError> {
        let ops = Array::from(state)
            .iter()
            .map(|o| js::drawing_sync_op(&o))
            .collect::<Result<Vec<_>, _>>()?;
        self.with_chart(|c| c.load_drawing_sync_state(&ops))?
            .map_err(js_err)?;
        schedule(&self.shared);
        Ok(())
    }

    /// Saves the view, layout and drawings as plain JSON-compatible data. Data,
    /// series and indicators are not included.
    ///
    /// @returns {ChartState}
    #[wasm_bindgen(js_name = saveState, unchecked_return_type = "ChartState")]
    pub fn save_state(&self) -> Result<JsValue, JsError> {
        self.with_chart(|c| js::state_js(&c.save_state()))
    }

    /// Loads a state from `saveState` into a chart set up with the same panes.
    /// The drawings replace the current ones; the undo history is cleared.
    /// Throws, leaving the chart unchanged, if the state does not fit.
    ///
    /// @param {ChartState} state
    #[wasm_bindgen(js_name = loadState)]
    pub fn load_state(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ChartState")] state: &JsValue,
    ) -> Result<(), JsError> {
        let s = js::state(state)?;
        self.with_chart(|c| c.load_state(s))?.map_err(js_err)
    }
}

#[wasm_bindgen]
impl Series {
    /// The kind of the series: `'volume'` for volume series, `'custom'` for
    /// custom series, `'kagi'`/`'pointFigure'` for those transforms (Renko
    /// and range bars are `'candlestick'` or `'ohlc'`).
    #[wasm_bindgen(
        getter,
        unchecked_return_type = "SeriesKind | 'volume' | 'custom' | 'kagi' | 'pointFigure'"
    )]
    pub fn kind(&self) -> Result<String, JsError> {
        self.with_chart(|c| c.series_kind(self.id))?
            .map(|k| params::series_kind_name(k).to_owned())
            .ok_or_else(|| js_err("the series was removed"))
    }

    /// Id of the series, unique within its chart: handles of the same series
    /// (e.g. from a `click` event) have the same id.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u32 {
        self.id.0
    }

    /// Id of the pane the series is on.
    #[wasm_bindgen(getter)]
    pub fn pane(&self) -> Result<u32, JsError> {
        self.with_chart(|c| c.series_pane(self.id))?
            .map(|p| p.0)
            .ok_or_else(|| js_err("the series was removed"))
    }

    /// Current options.
    ///
    /// @returns {Required<SeriesOptions>}
    #[wasm_bindgen(unchecked_return_type = "Required<SeriesOptions>")]
    pub fn options(&self) -> Result<JsValue, JsError> {
        self.with_chart(|c| c.series_options(self.id).map(js::series_options_js))?
            .ok_or_else(|| js_err("the series was removed"))
    }

    /// Changes some options; keys left out keep their value.
    ///
    /// @param {SeriesOptions} options
    #[wasm_bindgen(js_name = applyOptions)]
    pub fn apply_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "SeriesOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        self.with_chart(|c| -> Result<(), JsError> {
            let base = *c
                .series_options(self.id)
                .ok_or_else(|| js_err("the series was removed"))?;
            let opts = js::series_options(base, options)?;
            c.set_series_options(self.id, opts).map_err(js_err)
        })?
    }

    /// Values at logical index `index`: `{ open, high, low, close, volume }`
    /// for OHLC series, `{ value }` for the others; `null` for no data or a gap.
    #[wasm_bindgen(js_name = dataAt, unchecked_return_type = "BarValues | { value: number } | null")]
    pub fn data_at(&self, index: f64) -> Result<JsValue, JsError> {
        let ohlc = !matches!(
            self.with_chart(|c| c.series_kind(self.id))?,
            Some(
                SeriesKind::Line
                    | SeriesKind::Area
                    | SeriesKind::Baseline
                    | SeriesKind::Histogram
                    | SeriesKind::Points
            )
        );
        self.with_chart(|c| {
            let j = index.round() as i64;
            let shift = c.series_options(self.id).map_or(0, |o| i64::from(o.shift));
            let view = c.data_view();
            let Some(v) = view.ohlc(self.id, 0) else {
                return JsValue::NULL;
            };
            let j = j - shift;
            let Some(close) = v.close.at(v.first, j).filter(|x| !x.is_nan()) else {
                return JsValue::NULL;
            };
            let o = Object::new();
            if ohlc {
                let get = |col: &ferrochart_core::data::Column<'_>| {
                    col.at(v.first, j).unwrap_or(f64::NAN)
                };
                set(&o, "open", &get(&v.open).into());
                set(&o, "high", &get(&v.high).into());
                set(&o, "low", &get(&v.low).into());
                set(&o, "close", &close.into());
                let vol = v.volume.as_ref().and_then(|c| c.at(v.first, j));
                set(&o, "volume", &number_or_null(vol));
            } else {
                set(&o, "value", &close.into());
            }
            o.into()
        })
    }

    /// Adds a horizontal line at a price of this series.
    ///
    /// @param {PriceLineOptions} options
    /// @returns {PriceLine}
    #[wasm_bindgen(js_name = createPriceLine)]
    pub fn create_price_line(
        &self,
        #[wasm_bindgen(unchecked_param_type = "PriceLineOptions")] options: &JsValue,
    ) -> Result<PriceLine, JsError> {
        let opts = js::price_line_options(PriceLineOptions::default(), options)?;
        let id = self
            .with_chart(|c| c.create_price_line(self.id, opts))?
            .map_err(js_err)?;
        Ok(PriceLine {
            shared: self.shared.clone(),
            id,
        })
    }

    /// Replaces the markers of this series.
    ///
    /// @param {Marker[]} markers
    #[wasm_bindgen(js_name = setMarkers)]
    pub fn set_markers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Marker[]")] markers: &JsValue,
    ) -> Result<(), JsError> {
        crate::host::with_weak(&self.shared, |s| -> Result<(), JsError> {
            let m = js::markers(markers, s.icon_key)?;
            let icons = m.iter().any(|m| m.icon.is_some());
            s.chart.set_markers(self.id, m).map_err(js_err)?;
            if icons {
                s.icon_series.insert(self.id);
            }
            Ok(())
        })?
    }

    /// The markers, sorted by time.
    ///
    /// @returns {Required<Marker>[]}
    #[wasm_bindgen(unchecked_return_type = "Required<Marker>[]")]
    pub fn markers(&self) -> Result<JsValue, JsError> {
        crate::host::with_weak(&self.shared, |s| {
            js::markers_js(s.chart.markers(self.id).unwrap_or(&[]), s.icon_key)
        })
    }

    /// Removes the series (an OHLC series also removes its volume series).
    pub fn remove(&self) -> Result<(), JsError> {
        self.with_chart(|c| c.remove_series(self.id))?
            .map_err(js_err)
    }
}

/// A price line created with `Series.createPriceLine`.
#[wasm_bindgen]
pub struct PriceLine {
    shared: Weak<Shared>,
    id: PriceLineId,
}

#[wasm_bindgen]
impl PriceLine {
    /// Current options.
    #[wasm_bindgen(unchecked_return_type = "Required<PriceLineOptions>")]
    pub fn options(&self) -> Result<JsValue, JsError> {
        with_shared(&self.shared, |c| {
            c.price_line(self.id).map(js::price_line_options_js)
        })?
        .ok_or_else(|| js_err("the price line was removed"))
    }

    /// Changes some options.
    ///
    /// @param {PriceLineOptions} options
    #[wasm_bindgen(js_name = applyOptions)]
    pub fn apply_options(
        &self,
        #[wasm_bindgen(unchecked_param_type = "PriceLineOptions")] options: &JsValue,
    ) -> Result<(), JsError> {
        with_shared(&self.shared, |c| -> Result<(), JsError> {
            let base = c
                .price_line(self.id)
                .cloned()
                .ok_or_else(|| js_err("the price line was removed"))?;
            let opts = js::price_line_options(base, options)?;
            c.update_price_line(self.id, opts).map_err(js_err)
        })?
    }

    /// Removes the price line.
    pub fn remove(&self) -> Result<(), JsError> {
        with_shared(&self.shared, |c| c.remove_price_line(self.id))?.map_err(js_err)
    }
}

/// An indicator attached with `FerroChart.addIndicator`.
#[wasm_bindgen]
pub struct Indicator {
    shared: Weak<Shared>,
    id: IndicatorId,
}

#[wasm_bindgen]
impl Indicator {
    /// The output series, e.g. MACD's macd, signal and histogram (style them
    /// with `applyOptions`).
    ///
    /// @returns {Series[]}
    #[wasm_bindgen(unchecked_return_type = "Series[]")]
    pub fn outputs(&self) -> Result<Array, JsError> {
        self.output_series()
    }

    /// Ids of the bands the indicator fills between its outputs (BOLL,
    /// Keltner, Ichimoku); change them with `FerroChart.setBandOptions`.
    ///
    /// @returns {number[]}
    #[wasm_bindgen(unchecked_return_type = "number[]")]
    pub fn bands(&self) -> Result<Array, JsError> {
        let ids = with_shared(&self.shared, |c| {
            c.indicator_bands(self.id).map(<[BandId]>::to_vec)
        })?
        .ok_or_else(|| js_err("the indicator was removed"))?;
        Ok(ids.into_iter().map(|b| JsValue::from(b.0)).collect())
    }

    fn output_series(&self) -> Result<Array, JsError> {
        let ids = with_shared(&self.shared, |c| {
            c.indicator_series(self.id).map(<[SeriesId]>::to_vec)
        })?
        .ok_or_else(|| js_err("the indicator was removed"))?;
        Ok(ids
            .into_iter()
            .map(|id| {
                JsValue::from(Series {
                    shared: self.shared.clone(),
                    id,
                })
            })
            .collect())
    }

    /// Removes the indicator, its outputs and the pane it created.
    pub fn remove(&self) -> Result<(), JsError> {
        with_shared(&self.shared, |c| c.remove_indicator(self.id))?.map_err(js_err)
    }
}

/// The drawings of a chart (`FerroChart.drawings`). Ids are numbers; changes
/// made here and by the user are undoable.
#[wasm_bindgen]
pub struct Drawings {
    shared: Weak<Shared>,
}

fn drawing_id(id: f64) -> Result<DrawingId, JsError> {
    if id >= 0.0 && id.fract() == 0.0 && id < 9.0e15 {
        Ok(DrawingId(id as u64))
    } else {
        Err(js_err("invalid drawing id"))
    }
}

#[wasm_bindgen]
impl Drawings {
    fn with<T>(
        &self,
        f: impl FnOnce(&mut ferrochart_core::drawing::DrawingManager) -> T,
    ) -> Result<T, JsError> {
        with_shared(&self.shared, |c| f(c.drawings()))
    }

    /// Kinds that `add` and `startTool` accept.
    #[wasm_bindgen(unchecked_return_type = "string[]")]
    pub fn kinds(&self) -> Result<Array, JsError> {
        self.with(|d| d.kinds().map(JsValue::from_str).collect())
    }

    /// Adds a drawing on `pane` (default: the main pane); returns its id.
    ///
    /// @param {DrawingData} data
    /// @param {number} [pane]
    pub fn add(
        &self,
        #[wasm_bindgen(unchecked_param_type = "DrawingData")] data: &JsValue,
        pane: Option<u32>,
    ) -> Result<f64, JsError> {
        let d: DrawingData = js::drawing_data(data)?;
        with_shared(&self.shared, |c| {
            let pane = pane_id(pane, c);
            c.drawings().add(pane, &d)
        })?
        .map(|id| id.0 as f64)
        .map_err(js_err)
    }

    /// Removes a drawing.
    pub fn remove(&self, id: f64) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.remove(id))?.map_err(js_err)
    }

    /// Ids, bottom to top.
    #[wasm_bindgen(unchecked_return_type = "number[]")]
    pub fn ids(&self) -> Result<Array, JsError> {
        self.with(|d| d.ids().map(|id| JsValue::from_f64(id.0 as f64)).collect())
    }

    /// The saved form of a drawing, `null` for an unknown id.
    #[wasm_bindgen(unchecked_return_type = "Required<DrawingData> | null")]
    pub fn get(&self, id: f64) -> Result<JsValue, JsError> {
        let id = drawing_id(id)?;
        self.with(|d| {
            d.data(id)
                .map_or(JsValue::NULL, |x| js::drawing_data_js(&x))
        })
    }

    /// The pane of a drawing.
    #[wasm_bindgen(js_name = paneOf)]
    pub fn pane_of(&self, id: f64) -> Result<Option<u32>, JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.pane_of(id).map(|p| p.0))
    }

    /// Moves the anchors (same number as the drawing has).
    ///
    /// @param {number} id
    /// @param {Anchor[]} anchors
    #[wasm_bindgen(js_name = setAnchors)]
    pub fn set_anchors(
        &self,
        id: f64,
        #[wasm_bindgen(unchecked_param_type = "Anchor[]")] anchors: &JsValue,
    ) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        let a = js::anchors(anchors)?;
        self.with(|d| d.set_anchors(id, &a))?.map_err(js_err)
    }

    /// Changes some style keys.
    ///
    /// @param {number} id
    /// @param {DrawingStyle} style
    #[wasm_bindgen(js_name = setStyle)]
    pub fn set_style(
        &self,
        id: f64,
        #[wasm_bindgen(unchecked_param_type = "DrawingStyle")] style: &JsValue,
    ) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| -> Result<(), JsError> {
            let base = d.data(id).ok_or_else(|| js_err("unknown drawing"))?.style;
            let s = js::drawing_style(base, style)?;
            d.set_style(id, s).map_err(js_err)
        })?
    }

    /// Sets a tool parameter, e.g. `'text'` of a text drawing or `'levels'`
    /// of a Fibonacci retracement.
    ///
    /// @param {number} id
    /// @param {string} key
    /// @param {DrawingParam} value
    #[wasm_bindgen(js_name = setParam)]
    pub fn set_param(
        &self,
        id: f64,
        key: &str,
        #[wasm_bindgen(unchecked_param_type = "DrawingParam")] value: &JsValue,
    ) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        let p = js::param(value, key)?;
        self.with(|d| d.set_param(id, key, p))?.map_err(js_err)
    }

    /// Shows or hides a drawing.
    #[wasm_bindgen(js_name = setVisible)]
    pub fn set_visible(&self, id: f64, visible: bool) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.set_visible(id, visible))?.map_err(js_err)
    }

    /// Whether a drawing is shown, `undefined` for an unknown id.
    #[wasm_bindgen(js_name = isVisible)]
    pub fn is_visible(&self, id: f64) -> Result<Option<bool>, JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.is_visible(id))
    }

    /// Locks a drawing against editing (it can still be selected and removed).
    #[wasm_bindgen(js_name = setLocked)]
    pub fn set_locked(&self, id: f64, locked: bool) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.set_locked(id, locked))?.map_err(js_err)
    }

    /// Whether a drawing is locked, `undefined` for an unknown id.
    #[wasm_bindgen(js_name = isLocked)]
    pub fn is_locked(&self, id: f64) -> Result<Option<bool>, JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.is_locked(id))
    }

    /// Draws a drawing above the others.
    #[wasm_bindgen(js_name = bringToFront)]
    pub fn bring_to_front(&self, id: f64) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.bring_to_front(id))?.map_err(js_err)
    }

    /// Draws a drawing below the others.
    #[wasm_bindgen(js_name = sendToBack)]
    pub fn send_to_back(&self, id: f64) -> Result<(), JsError> {
        let id = drawing_id(id)?;
        self.with(|d| d.send_to_back(id))?.map_err(js_err)
    }

    /// Selects a drawing (`null` clears the selection).
    pub fn select(&self, id: Option<f64>) -> Result<(), JsError> {
        let id = id.map(drawing_id).transpose()?;
        self.with(|d| d.select(id))?.map_err(js_err)
    }

    /// The selected drawing, `undefined` if none.
    #[wasm_bindgen(getter)]
    pub fn selected(&self) -> Result<Option<f64>, JsError> {
        self.with(|d| d.selected().map(|id| id.0 as f64))
    }

    /// Registers a drawing tool written in JS under `kind`, for this
    /// chart (`add`, `startTool`, `loadState`). `tool.anchorCount` says how
    /// it is created: a number (1–64) of clicks, `{ open: min }` (a click per
    /// anchor until a double click or Enter, at least `min`, 1–64) or
    /// `'freehand'` (press, drag, release; no handles); `tool.build(ctx)` returns its
    /// commands in CSS px (`ctx.anchors` have `time`, `price`, `x`, `y`;
    /// `ctx.style`, `ctx.params`); the optional `tool.hitTest(ctx, point)`
    /// returns `true` (the drawing), a handle number or `false` (default:
    /// anything inside the drawn bounds). Built-in kinds cannot be replaced;
    /// registering a JS kind again replaces it on every chart of the page.
    ///
    /// @param {string} kind
    /// @param {DrawingToolJs} tool
    pub fn register(
        &self,
        kind: &str,
        #[wasm_bindgen(unchecked_param_type = "DrawingToolJs")] tool: JsValue,
    ) -> Result<(), JsError> {
        if ferrochart_core::drawing::BUILTIN_KINDS.contains(&kind) {
            return Err(js_err("built-in drawing kinds cannot be replaced"));
        }
        if kind.is_empty() {
            return Err(js_err("the kind must not be empty"));
        }
        if !js::get(&tool, "build").is_function() {
            return Err(js_err("a drawing tool needs a build(ctx) method"));
        }
        let anchors = anchor_count(&js::get(&tool, "anchorCount"))?;
        // The chart must exist before the registry changes.
        self.with(|_| ())?;
        let kind = plugins::register_tool(kind, tool, anchors);
        self.with(|d| d.register(kind, plugins::js_tool))
    }

    /// Arms a tool: the next clicks on a plot create a drawing of `kind`.
    #[wasm_bindgen(js_name = startTool)]
    pub fn start_tool(&self, kind: &str) -> Result<(), JsError> {
        self.with(|d| d.start_tool(kind))?.map_err(js_err)
    }

    /// Cancels the armed tool or the drawing being created.
    #[wasm_bindgen(js_name = cancelTool)]
    pub fn cancel_tool(&self) -> Result<(), JsError> {
        self.with(ferrochart_core::drawing::DrawingManager::cancel_tool)
    }

    /// The armed tool, `undefined` if none.
    #[wasm_bindgen(getter, js_name = activeTool)]
    pub fn active_tool(&self) -> Result<Option<String>, JsError> {
        self.with(|d| d.active_tool().map(str::to_owned))
    }

    /// Undoes the last change; `false` if there was none.
    pub fn undo(&self) -> Result<bool, JsError> {
        self.with(ferrochart_core::drawing::DrawingManager::undo)
    }

    /// Redoes the last undone change; `false` if there was none.
    pub fn redo(&self) -> Result<bool, JsError> {
        self.with(ferrochart_core::drawing::DrawingManager::redo)
    }

    /// Whether there is a change to undo.
    #[wasm_bindgen(getter, js_name = canUndo)]
    pub fn can_undo(&self) -> Result<bool, JsError> {
        self.with(|d| d.can_undo())
    }

    /// Whether there is an undone change to redo.
    #[wasm_bindgen(getter, js_name = canRedo)]
    pub fn can_redo(&self) -> Result<bool, JsError> {
        self.with(|d| d.can_redo())
    }

    /// Forgets the undo and redo history.
    #[wasm_bindgen(js_name = clearHistory)]
    pub fn clear_history(&self) -> Result<(), JsError> {
        self.with(ferrochart_core::drawing::DrawingManager::clear_history)
    }
}

/// A group of charts whose crosshair, visible range and (optionally)
/// drawings follow each other. Charts join with `FerroChart.joinSync`.
#[wasm_bindgen]
pub struct SyncGroup {
    group: ferrochart_core::sync::SyncGroup,
    peers: Peers,
}

#[wasm_bindgen]
impl SyncGroup {
    /// Creates an empty group. Pages that forward drawing operations between
    /// groups (charts in workers) give each group its own `origin`
    /// (0..2^21, so that keys stay exact JS numbers).
    ///
    /// @param {number} [origin]
    #[wasm_bindgen(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new(origin: Option<u32>) -> Result<SyncGroup, JsError> {
        let origin = origin.unwrap_or(0);
        if origin >= 1 << 21 {
            return Err(js_err("origin must be below 2^21"));
        }
        Ok(SyncGroup {
            group: ferrochart_core::sync::SyncGroup::with_origin(origin),
            peers: Rc::new(RefCell::new(Vec::new())),
        })
    }
}

/// Names accepted by `FerroChart.addIndicator`.
#[wasm_bindgen(js_name = indicatorNames, unchecked_return_type = "string[]")]
pub fn indicator_names() -> Array {
    params::INDICATORS
        .iter()
        .map(|&n| JsValue::from_str(n))
        .collect()
}

/// Builds from the parameter object `p` (`undefined`/`null`: no parameters)
/// and rejects keys that `build` never reads, as the Python and C bindings do:
/// a dry run with every parameter missing collects the keys asked for.
fn build_params<T>(
    p: &JsValue,
    what: &str,
    build: impl Fn(&dyn Fn(&str) -> params::Value) -> Result<T, String>,
) -> Result<T, JsError> {
    if !(p.is_undefined() || p.is_null() || p.is_object()) {
        return Err(js_err(format!(
            "the parameters of {what} must be an object"
        )));
    }
    let asked = RefCell::new(Vec::<String>::new());
    let ask = |k: &str| {
        let mut a = asked.borrow_mut();
        if !a.iter().any(|x| x == k) {
            a.push(k.to_owned());
        }
    };
    let _ = build(&|k| {
        ask(k);
        params::Value::Missing
    });
    let read = param_reader(p);
    let built = build(&|k| {
        ask(k);
        read(k)
    });
    if p.is_object() {
        let asked = asked.into_inner();
        for key in Object::keys(&Object::from(p.clone())).iter() {
            let Some(key) = key.as_string() else { continue };
            if !asked.contains(&key) {
                return Err(js_err(format!(
                    "unknown parameter '{key}' for {what} (expected: {})",
                    asked.join(", ")
                )));
            }
        }
    }
    built.map_err(js_err)
}

/// Reads parameter `k` of the JS object `p` (`undefined`/`null`: missing).
fn param_reader(p: &JsValue) -> impl Fn(&str) -> params::Value + '_ {
    move |k| {
        let v = js::get(p, k);
        if v.is_undefined() || v.is_null() {
            params::Value::Missing
        } else if let Some(n) = v.as_f64() {
            params::Value::Number(n)
        } else if let Some(s) = v.as_string() {
            params::Value::Text(s)
        } else {
            params::Value::Other
        }
    }
}

/// `DrawingToolJs.anchorCount`: `n`, `{ open: min }` or `'freehand'`.
fn anchor_count(v: &JsValue) -> Result<ferrochart_core::drawing::AnchorCount, JsError> {
    use ferrochart_core::drawing::AnchorCount;
    let clicks = |n: Option<f64>| {
        n.filter(|n| n.fract() == 0.0 && (1.0..=64.0).contains(n))
            .map(|n| n as usize)
    };
    if v.as_string().as_deref() == Some("freehand") {
        return Ok(AnchorCount::Freehand);
    }
    if let Some(n) = clicks(v.as_f64()) {
        return Ok(AnchorCount::Fixed(n));
    }
    if let Some(min) = v
        .is_object()
        .then(|| clicks(js::get(v, "open").as_f64()))
        .flatten()
    {
        return Ok(AnchorCount::Open { min });
    }
    Err(js_err(
        "anchorCount must be an integer from 1 to 64, { open: 1..64 } or 'freehand'",
    ))
}
