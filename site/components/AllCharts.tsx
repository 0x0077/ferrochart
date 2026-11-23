'use client';

import type { CSSProperties } from 'react';
import type { Demo } from '@/catalog';
import { useCatalog } from './I18n';
import { Thumb } from './Thumb';

/**
 * Every example of every category over the chart area, by category, as
 * card images (a live chart each would need a GPU device each).
 */
export function AllCharts({ current, onPick }: { current: Demo; onPick: (d: Demo) => void }) {
  const { categories } = useCatalog();
  return (
    <div
      id="all-charts"
      className="layer-in absolute inset-0 bottom-7 z-[5] flex flex-col gap-7 overflow-y-auto rounded-[14px] border border-line bg-white px-[22px] pt-5 pb-7 shadow-[0_30px_60px_-30px_rgb(13_15_20/0.3)] [scrollbar-color:#d3d6dc_transparent] [scrollbar-width:thin]"
    >
      {categories.map((c) => (
        <section key={c.id} className="flex flex-col gap-3" style={{ '--accent': c.accent } as CSSProperties}>
          <h4 className="sticky -top-5 z-[1] flex items-baseline gap-2.5 bg-white py-1.5">
            <span className="font-mono text-xs accent-ink">{c.kicker}</span>
            <span className="text-base font-semibold">{c.title}</span>
            <span className="font-mono text-xs text-faint">{c.demos.length}</span>
          </h4>
          <ol className="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-2.5 p-0">
            {c.demos.map((d) => (
              <li key={d.id} className="lazy-item">
                <button
                  type="button"
                  data-demo={d.id}
                  aria-current={d === current}
                  onClick={() => onPick(d)}
                  className="flex w-full cursor-pointer flex-col overflow-hidden rounded-[10px] border border-line bg-white text-left transition hover:border-ink/25 aria-[current=true]:border-[var(--accent)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]"
                >
                  <div className="aspect-[16/9] bg-chart">
                    <Thumb id={d.id} />
                  </div>
                  <span className="truncate border-t border-line px-2.5 py-2 text-[12.5px] text-ink">{d.title}</span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      ))}
    </div>
  );
}
