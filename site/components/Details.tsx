'use client';

import { useEffect, useRef } from 'react';
import type { Category, Demo } from '@/catalog';
import { highlight } from '@/lib/highlight.js';

/** How the example's code makes its chart: a FerroChart, a FerroPlot or both. */
function kindOf(d: Demo): { create: string; data: string } {
  const plot = d.code.includes('FerroPlot.create');
  const chart = d.code.includes('FerroChart.create');
  if (plot && !chart) {
    return { create: "const plot = await FerroPlot.create(canvas, { theme: 'dark' });", data: "plot.addMark('line').setData({ x, y });" };
  }
  return {
    create: "const chart = await FerroChart.create(canvas, { theme: 'dark' });",
    data: 'chart.addCandlestickSeries().setData({ time, open, high, low, close });',
  };
}

/**
 * The full description of the current example over the viewer: what it
 * shows, the live chart (the viewer's own, moved here with its state, so
 * there is still one GPU device), how to use it, the API and the code.
 */
export function Details({
  cat,
  demo,
  renderer,
  code,
  copyLabel,
  onCopy,
  onClose,
  onSlot,
}: {
  cat: Category;
  demo: Demo;
  renderer: string | null;
  code: string;
  copyLabel: string;
  onCopy: () => void;
  onClose: () => void;
  /** Called with the element the live chart should move into (null: back). */
  onSlot: (el: HTMLElement | null) => void;
}) {
  const slot = useRef<HTMLDivElement>(null);
  const closeBtn = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    onSlot(slot.current);
    closeBtn.current?.focus({ preventScroll: true });
    return () => onSlot(null);
  }, [onSlot]);

  const k = kindOf(demo);
  const steps = [
    { t: 'Install the package', c: 'npm install @ferrochart/core' },
    { t: 'Create it on a canvas', c: k.create },
    { t: 'Give it your data: this page uses a synthetic feed from its own server', c: k.data },
    { t: 'Use the API of this example: the full code is below', c: demo.api.join(' · ') },
  ];
  const facts = [
    ['Example', demo.id],
    ['Renderer', renderer ?? 'Starting…'],
    ['Data', 'Synthetic market data from this site. Replace it with your own source.'],
    ['Licence', 'Apache-2.0'],
  ];

  return (
    <>
      <div className="fixed inset-0 z-[70] bg-[rgb(20_22_28/0.22)] backdrop-blur-[6px]" onClick={onClose} />
      <div
        id="details"
        role="dialog"
        aria-modal="true"
        aria-labelledby="details-title"
        className="layer-in fixed top-[clamp(8px,4vh,40px)] bottom-[clamp(8px,4vh,40px)] left-1/2 z-[71] w-[min(1180px,calc(100vw-16px))] -translate-x-1/2 overflow-y-auto rounded-[20px] border border-line bg-white shadow-[0_40px_120px_rgb(13_15_20/0.3)] [scrollbar-color:#d3d6dc_transparent] [scrollbar-width:thin]"
      >
        <div className="sticky top-0 z-[2] flex items-center gap-3.5 border-b border-line bg-white/90 px-[22px] py-[18px] backdrop-blur-md">
          <span className="font-mono text-xs whitespace-nowrap accent-ink">
            {cat.kicker} · {cat.title}
          </span>
          <h3 id="details-title" className="min-w-0 flex-1 truncate text-xl">{demo.title}</h3>
          <button
            ref={closeBtn}
            type="button"
            aria-label="Close details"
            onClick={onClose}
            className="grid size-[34px] shrink-0 cursor-pointer place-items-center rounded-full border border-line hover:bg-ink/[0.06]"
          >
            ✕
          </button>
        </div>
        <div className="flex flex-col gap-9 p-[22px]">
          <p className="prose-code m-0 max-w-[760px] text-base text-muted" dangerouslySetInnerHTML={{ __html: highlight.inlineCode(demo.about) }} />
          <div ref={slot} id="details-chart" className="relative h-[min(52vh,520px)] min-h-[300px] overflow-hidden rounded-[14px] border border-line bg-chart" />
          <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,400px),1fr))] gap-9">
            <div className="flex flex-col gap-7">
              <div className="flex flex-col gap-3.5">
                <h4 className="text-[17px] font-semibold">How to use it</h4>
                {steps.map((s, i) => (
                  <div key={s.t} className="grid grid-cols-[28px_minmax(0,1fr)] items-start gap-3">
                    <span className="grid size-6 place-items-center rounded-full border border-line font-mono text-[11px] text-muted">{i + 1}</span>
                    <div className="flex min-w-0 flex-col gap-1.5">
                      <span className="text-sm text-ink">{s.t}</span>
                      <code className="max-w-full self-start overflow-x-auto rounded-[7px] border border-line bg-chart px-[9px] py-[5px] font-mono text-xs whitespace-nowrap text-[#b5dc7d]">{s.c}</code>
                    </div>
                  </div>
                ))}
              </div>
              <div className="flex flex-col gap-3">
                <h4 className="text-[17px] font-semibold">API used</h4>
                <div className="flex flex-wrap gap-2">
                  {demo.api.map((a) => (
                    <code key={a} className="rounded-[7px] border border-line bg-bg px-[9px] py-[5px] font-mono text-xs text-ink">{a}</code>
                  ))}
                </div>
              </div>
              <dl className="m-0 grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-2.5 text-[13.5px]">
                {facts.map(([k2, v]) => (
                  <div key={k2} className="contents">
                    <dt className="text-faint">{k2}</dt>
                    <dd className="m-0 text-ink">{v}</dd>
                  </div>
                ))}
              </dl>
            </div>
            <div className="flex min-w-0 flex-col self-start overflow-hidden rounded-[14px] border border-line bg-chart">
              <div className="flex items-center gap-2.5 border-b border-chart-line py-2.5 pr-2.5 pl-[18px]">
                <span className="text-[11px] tracking-[0.08em] text-on-chart-muted uppercase">Integration</span>
                <button type="button" onClick={onCopy} className="ml-auto h-7 cursor-pointer rounded-lg border border-chart-line px-3 text-xs text-on-chart hover:bg-white/5">
                  {copyLabel}
                </button>
              </div>
              <pre className="m-0 overflow-x-auto px-[18px] py-3.5 font-mono text-[12.5px] leading-[1.65] text-[#c9d1dc]">
                <code dangerouslySetInnerHTML={{ __html: code }} />
              </pre>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
