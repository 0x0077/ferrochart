/** One chart example. `code` is what runs in the viewer (with `canvas`,
 * `side`, `FerroChart`, `FerroPlot`, `fetch` and `WebSocket` in scope) and,
 * with its prelude (`preludeOf`), what the integration panel shows. It
 * creates a FerroChart, a FerroPlot or both. */
export interface Demo {
  id: string;
  title: string;
  /** Sub-group in the viewer's list. */
  group?: string;
  /** Prose; `backticks` become code. */
  about: string;
  /** API names shown as chips. */
  api: string[];
  code: string;
  /** A second canvas, `side`, right of the chart: for a FerroPlot attached
   * with `chart.attachSidePlot`. */
  side?: boolean;
}

export interface Category {
  id: string;
  title: string;
  /** Two-digit number shown before the title. */
  kicker: string;
  blurb: string;
  accent: string;
  demos: Demo[];
}
