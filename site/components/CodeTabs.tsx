'use client';

import { useState } from 'react';
import { highlight } from '@/lib/highlight';

type Lang = { id: string; label: string; lang: string; file: string; install: string; code: string };

/**
 * The same chart in each language, kept to the real APIs (each package's
 * README, the .d.ts, ferrochart.h). React and Vue use @ferrochart/core in a
 * component; there are no separate wrapper packages.
 */
const LANGS: Lang[] = [
  {
    id: 'js',
    label: 'JavaScript',
    lang: 'js',
    file: 'main.js',
    install: 'npm install @ferrochart/core',
    code: `import { FerroChart } from '@ferrochart/core';

const chart = await FerroChart.create(canvas, { theme: 'dark' });
const candles = chart.addCandlestickSeries();
chart.addVolumeSeries(candles);
candles.setData(bars); // [{ time, open, high, low, close, volume }, ...]
chart.addIndicator(candles, 'ema', { n: 20 });
chart.addIndicator(candles, 'macd'); // in a new pane`,
  },
  {
    id: 'ts',
    label: 'TypeScript',
    lang: 'js',
    file: 'chart.ts',
    install: 'npm install @ferrochart/core',
    code: `import { FerroChart, type OhlcRow } from '@ferrochart/core';

const canvas = document.querySelector<HTMLCanvasElement>('#chart')!;
const chart = await FerroChart.create(canvas, { theme: 'dark' });

const candles = chart.addCandlestickSeries();
const bars: OhlcRow[] = await loadBars(); // time: UTC ms or Date
candles.setData(bars);
chart.addIndicator(candles, 'rsi', { n: 14 }); // in a new pane

chart.on('crosshairMove', ({ point }) => console.log(point?.bar));`,
  },
  {
    id: 'react',
    label: 'React',
    lang: 'js',
    file: 'PriceChart.tsx',
    install: 'npm install @ferrochart/core',
    code: `import { useEffect, useRef } from 'react';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

export function PriceChart({ bars }: { bars: OhlcRow[] }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let chart: FerroChart | undefined;
    let gone = false;
    FerroChart.create(canvas.current!, { theme: 'dark' }).then((c) => {
      if (gone) return c.destroy();
      chart = c;
      chart.addCandlestickSeries().setData(bars);
    });
    return () => { gone = true; chart?.destroy(); };
  }, [bars]);
  return <canvas ref={canvas} style={{ width: '100%', height: 480 }} />;
}`,
  },
  {
    id: 'vue',
    label: 'Vue',
    lang: 'js',
    file: 'PriceChart.vue',
    install: 'npm install @ferrochart/core',
    code: `<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue';
import { FerroChart, type OhlcRow } from '@ferrochart/core';

const props = defineProps<{ bars: OhlcRow[] }>();
const canvas = ref<HTMLCanvasElement>();
let chart: FerroChart | undefined;

onMounted(async () => {
  chart = await FerroChart.create(canvas.value!, { theme: 'dark' });
  chart.addCandlestickSeries().setData(props.bars);
});
onBeforeUnmount(() => chart?.destroy());
</script>

<template>
  <canvas ref="canvas" style="width: 100%; height: 480px" />
</template>`,
  },
  {
    id: 'rust',
    label: 'Rust',
    lang: 'rust',
    file: 'main.rs',
    install: 'cargo add ferrochart-core ferrochart-indicators ferrochart-winit',
    code: `use ferrochart_core::{Chart, ChartOptions, theme::Theme};
use ferrochart_indicators::named::ChartExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut chart = Chart::new(ChartOptions { theme: Theme::dark(), ..Default::default() });
    let candles = chart.add_candlestick()?;
    chart.add_volume(candles)?;
    chart.set_data(candles, load_bars()?)?; // OhlcColumns: time, open, high, low, close
    chart.add_indicator_by_name(candles, "ema", &[("n", 20.into())])?;
    ferrochart_winit::show(chart)?; // Metal, Vulkan or DirectX 12
    Ok(())
}`,
  },
  {
    id: 'python',
    label: 'Python',
    lang: 'python',
    file: 'chart.py',
    install: 'pip install ferrochart',
    code: `import ferrochart as fc

chart = fc.Chart(theme="dark", width=1200, height=700)
candles = chart.add_series("candlestick")
candles.set_data(df)  # pandas or polars: time, open, high, low, close[, volume]
chart.add_volume_series(candles)
chart.add_indicator(candles, "ema", n=20)
chart.add_indicator(candles, "macd")  # in a new pane

chart.show()  # a native window; in Jupyter the chart is a live widget`,
  },
  {
    id: 'c',
    label: 'C',
    lang: 'c',
    file: 'chart.c',
    install: 'cargo build -p ferrochart-ffi --release  # library + ferrochart.h',
    code: `#include "ferrochart.h"

FerroChartOptions o = { .size = sizeof o };
ferrochart_options_default(&o);
o.theme = FERROCHART_THEME_DARK;
o.width = 1200; o.height = 700; o.dpr = 2;

FerroChart *chart;
if (ferrochart_create(&o, &chart) != FERROCHART_OK) return report(ferrochart_last_error());

uint64_t candles, ema;
ferrochart_add_series(chart, FERROCHART_MAIN_PANE, "candlestick", NULL, &candles);
ferrochart_set_data_ohlc(chart, candles, time, open, high, low, close, volume, n);
ferrochart_add_indicator(chart, candles, "ema", "{\\"n\\": 20}", FERROCHART_PLACEMENT_DEFAULT, &ema);

ferrochart_render_rgba(chart, now_ms, pixels, stride, size); // or into a native window
ferrochart_destroy(chart);`,
  },
];

const icon = { width: 13, height: 13, viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, 'aria-hidden': true } as const;
const CopyIcon = () => (
  <svg {...icon} strokeLinejoin="round">
    <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
    <path d="M10.5 3.5v-.5a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h.5" />
  </svg>
);
const CheckIcon = () => (
  <svg {...icon} strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 8.5l3.2 3L13 4.5" />
  </svg>
);

function Copy({ text, label }: { text: string; label: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      aria-label={label}
      className="copy inline-flex items-center gap-1.5 rounded-md px-2 py-1 font-mono text-[11px] text-on-chart-muted transition hover:bg-white/10 hover:text-on-chart"
      onClick={() =>
        navigator.clipboard?.writeText(text).then(() => {
          setDone(true);
          setTimeout(() => setDone(false), 1400);
        })
      }
    >
      {done ? <CheckIcon /> : <CopyIcon />}
      {done ? 'Copied' : 'Copy'}
    </button>
  );
}

/** A dark editor window with one tab per language. */
export function CodeTabs() {
  const [cur, setCur] = useState(LANGS[0]);
  return (
    <div id="code-tabs" className="overflow-hidden rounded-[20px] border border-chart-line bg-chart text-on-chart shadow-[0_40px_80px_-40px_rgb(13_15_20/0.6)]">
      <div role="tablist" aria-label="Language" className="flex items-center gap-1 overflow-x-auto border-b border-chart-line px-3 pt-3 [scrollbar-width:none]">
        {LANGS.map((l) => (
          <button
            key={l.id}
            type="button"
            role="tab"
            id={`tab-${l.id}`}
            aria-selected={l.id === cur.id}
            aria-controls="code-panel"
            onClick={() => setCur(l)}
            className={`-mb-px shrink-0 whitespace-nowrap rounded-t-lg border-b-2 px-3.5 pt-1.5 pb-2.5 text-[13px] transition ${
              l.id === cur.id ? 'border-white text-on-chart' : 'border-transparent text-on-chart-muted hover:text-on-chart'
            }`}
          >
            {l.label}
          </button>
        ))}
        <span className="ml-auto flex items-center gap-2 pb-2">
          <span className="font-mono text-[11px] text-on-chart-muted max-sm:hidden">{cur.file}</span>
          <Copy text={cur.code} label="Copy the code" />
        </span>
      </div>
      <div className="flex items-center justify-between gap-3 border-b border-chart-line bg-white/[0.03] px-5 py-2.5 font-mono text-[13px]">
        <span className="truncate">
          <span className="select-none text-on-chart-muted">$ </span>
          {cur.install}
        </span>
        <Copy text={cur.install} label="Copy the install command" />
      </div>
      {/* Every snippet sits in the same grid cell, the hidden ones invisible:
          the panel always has the height of the longest, so switching tabs
          does not move the page. */}
      <div id="code-panel" role="tabpanel" aria-labelledby={`tab-${cur.id}`} className="grid">
        {LANGS.map((l) => (
          <pre
            key={l.id}
            aria-hidden={l.id !== cur.id}
            className={`col-start-1 row-start-1 m-0 overflow-x-auto px-5 py-5 font-mono text-[13px] leading-[1.7] ${l.id === cur.id ? '' : 'invisible'}`}
          >
            <code dangerouslySetInnerHTML={{ __html: highlight(l.code, l.lang) }} />
          </pre>
        ))}
      </div>
    </div>
  );
}
