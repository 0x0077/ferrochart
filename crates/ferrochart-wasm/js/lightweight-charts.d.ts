// Types of the lightweight-charts compatibility layer of @ferrochart/core.
// A subset of the lightweight-charts v4/v5 API.
import type { FerroChart } from './ferrochart_wasm.js';

export type UTCTimestamp = number;
export interface BusinessDay { year: number; month: number; day: number }
/** UTC seconds, 'YYYY-MM-DD' or a business day. */
export type Time = UTCTimestamp | string | BusinessDay;
export type Logical = number;

export declare const ColorType: { readonly Solid: 'solid'; readonly VerticalGradient: 'gradient' };
export declare const CrosshairMode: { readonly Normal: 0; readonly Magnet: 1; readonly Hidden: 2; readonly MagnetOHLC: 3 };
export declare const LineStyle: { readonly Solid: 0; readonly Dotted: 1; readonly Dashed: 2; readonly LargeDashed: 3; readonly SparseDotted: 4 };
export declare const LineType: { readonly Simple: 0; readonly WithSteps: 1; readonly Curved: 2 };
export declare const PriceScaleMode: { readonly Normal: 0; readonly Logarithmic: 1; readonly Percentage: 2; readonly IndexedTo100: 3 };

export type SeriesType = 'Candlestick' | 'Bar' | 'Line' | 'Area' | 'Baseline' | 'Histogram';
export interface SeriesDefinition<T extends SeriesType> { readonly type: T }
export declare const CandlestickSeries: SeriesDefinition<'Candlestick'>;
export declare const BarSeries: SeriesDefinition<'Bar'>;
export declare const LineSeries: SeriesDefinition<'Line'>;
export declare const AreaSeries: SeriesDefinition<'Area'>;
export declare const BaselineSeries: SeriesDefinition<'Baseline'>;
export declare const HistogramSeries: SeriesDefinition<'Histogram'>;

export interface OhlcData { time: Time; open: number; high: number; low: number; close: number }
export interface SingleValueData { time: Time; value: number }
export interface HistogramData extends SingleValueData { color?: string }
export interface WhitespaceData { time: Time }

export interface SeriesDataItemTypeMap {
  Candlestick: OhlcData | WhitespaceData;
  Bar: OhlcData | WhitespaceData;
  Line: SingleValueData | WhitespaceData;
  Area: SingleValueData | WhitespaceData;
  Baseline: SingleValueData | WhitespaceData;
  Histogram: HistogramData | WhitespaceData;
}

export interface PriceFormat { type?: 'price' | 'volume' | 'percent'; precision?: number; minMove?: number }

/** Options that have an equivalent; others are accepted and ignored. */
export interface SeriesOptions {
  upColor?: string;
  downColor?: string;
  color?: string;
  lineColor?: string;
  topColor?: string;
  bottomColor?: string;
  topLineColor?: string;
  bottomLineColor?: string;
  topFillColor1?: string;
  bottomFillColor1?: string;
  baseValue?: { type: 'price'; price: number };
  base?: number;
  lineWidth?: number;
  lineStyle?: number;
  lineType?: number;
  lastValueVisible?: boolean;
  priceLineVisible?: boolean;
  priceFormat?: PriceFormat;
  /** Only 'right' exists; other ids put the series on a pane of its own. */
  priceScaleId?: string;
  [option: string]: unknown;
}

export interface PriceLineOptions {
  price: number;
  color?: string;
  lineWidth?: number;
  lineStyle?: number;
  axisLabelVisible?: boolean;
  title?: string;
}

export interface IPriceLine {
  applyOptions(options: Partial<PriceLineOptions>): void;
  options(): Partial<PriceLineOptions>;
}

export interface SeriesMarker {
  time: Time;
  position: 'aboveBar' | 'belowBar' | 'inBar';
  shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown';
  color?: string;
  text?: string;
}

export interface PriceScaleOptions {
  scaleMargins?: { top: number; bottom: number };
  mode?: number;
  invertScale?: boolean;
  autoScale?: boolean;
}

export interface IPriceScaleApi {
  applyOptions(options: PriceScaleOptions): void;
  options(): PriceScaleOptions;
}

export interface ISeriesApi<T extends SeriesType = SeriesType> {
  seriesType(): T;
  setData(data: SeriesDataItemTypeMap[T][]): void;
  update(bar: SeriesDataItemTypeMap[T]): void;
  applyOptions(options: SeriesOptions): void;
  options(): SeriesOptions;
  priceScale(): IPriceScaleApi;
  createPriceLine(options: PriceLineOptions): IPriceLine;
  removePriceLine(line: IPriceLine): void;
  /** lightweight-charts v4. */
  setMarkers(markers: SeriesMarker[]): void;
  markers(): SeriesMarker[];
  dataByIndex(logical: number): SeriesDataItemTypeMap[T] | null;
  /** Not supported yet: always `null`. */
  priceToCoordinate(price: number): number | null;
  /** Not supported yet: always `null`. */
  coordinateToPrice(y: number): number | null;
}

export interface LogicalRange { from: Logical; to: Logical }
export interface TimeRange { from: Time; to: Time }

export interface ITimeScaleApi {
  fitContent(): void;
  scrollToRealTime(): void;
  scrollToPosition(position: number, animated?: boolean): void;
  scrollPosition(): number;
  getVisibleLogicalRange(): LogicalRange | null;
  setVisibleLogicalRange(range: LogicalRange): void;
  getVisibleRange(): TimeRange | null;
  setVisibleRange(range: TimeRange): void;
  logicalToCoordinate(logical: Logical): number | null;
  coordinateToLogical(x: number): Logical | null;
  timeToCoordinate(time: Time): number | null;
  coordinateToTime(x: number): Time | null;
  subscribeVisibleLogicalRangeChange(handler: (range: LogicalRange | null) => void): void;
  unsubscribeVisibleLogicalRangeChange(handler: (range: LogicalRange | null) => void): void;
  subscribeVisibleTimeRangeChange(handler: (range: TimeRange | null) => void): void;
  unsubscribeVisibleTimeRangeChange(handler: (range: TimeRange | null) => void): void;
  applyOptions(options: { barSpacing?: number; rightOffset?: number; [option: string]: unknown }): void;
  options(): { barSpacing: number; rightOffset: number };
  width(): number;
}

export interface MouseEventParams {
  time?: Time;
  logical?: Logical;
  point?: { x: number; y: number };
  paneIndex?: number;
  seriesData: Map<ISeriesApi, OhlcData | SingleValueData>;
}

export interface ChartOptions {
  width?: number;
  height?: number;
  autoSize?: boolean;
  /** `background.color` picks the dark or the light theme. */
  layout?: { background?: { type?: string; color?: string }; textColor?: string; [option: string]: unknown };
  crosshair?: { mode?: number; [option: string]: unknown };
  timeScale?: { barSpacing?: number; rightOffset?: number; [option: string]: unknown };
  [option: string]: unknown;
}

export interface IChartApi {
  addSeries<T extends SeriesType>(definition: SeriesDefinition<T>, options?: SeriesOptions, paneIndex?: number): ISeriesApi<T>;
  addCandlestickSeries(options?: SeriesOptions): ISeriesApi<'Candlestick'>;
  addBarSeries(options?: SeriesOptions): ISeriesApi<'Bar'>;
  addLineSeries(options?: SeriesOptions): ISeriesApi<'Line'>;
  addAreaSeries(options?: SeriesOptions): ISeriesApi<'Area'>;
  addBaselineSeries(options?: SeriesOptions): ISeriesApi<'Baseline'>;
  addHistogramSeries(options?: SeriesOptions): ISeriesApi<'Histogram'>;
  removeSeries(series: ISeriesApi): void;
  timeScale(): ITimeScaleApi;
  priceScale(priceScaleId?: string, paneIndex?: number): IPriceScaleApi;
  subscribeCrosshairMove(handler: (param: MouseEventParams) => void): void;
  unsubscribeCrosshairMove(handler: (param: MouseEventParams) => void): void;
  /** Not supported yet (no-op). */
  subscribeClick(handler: (param: MouseEventParams) => void): void;
  unsubscribeClick(handler: (param: MouseEventParams) => void): void;
  applyOptions(options: ChartOptions): void;
  options(): ChartOptions;
  resize(width: number, height: number): void;
  remove(): void;
  /** FerroChart extension: resolves once the GPU is set up and queued calls ran. */
  whenReady(): Promise<void>;
  /** FerroChart extension: the underlying chart, `null` until ready. */
  readonly ferrochart: FerroChart | null;
}

export interface ISeriesMarkersPluginApi {
  setMarkers(markers: SeriesMarker[]): void;
  markers(): SeriesMarker[];
  detach(): void;
}

export declare function createChart(container: HTMLElement | string, options?: ChartOptions): IChartApi;
export declare function createSeriesMarkers(series: ISeriesApi, markers?: SeriesMarker[]): ISeriesMarkersPluginApi;
export declare function isUTCTimestamp(time: Time): time is UTCTimestamp;
export declare function isBusinessDay(time: Time): time is BusinessDay;
