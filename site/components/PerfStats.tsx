'use client';

import { useEffect, useRef, useState } from 'react';

export type Stat = {
  value: number;
  /** Decimals shown. */
  digits?: number;
  unit: string;
  label: string;
  note: string;
  /** Bar length, 0–1: the measurement as a share of its target. */
  share: number;
};

/**
 * The performance cards: the numbers count up and the bars grow the first
 * time the grid scrolls into view. Screen readers and reduced motion get the
 * final values at once.
 */
export function PerfStats({ stats }: { stats: Stat[] }) {
  const ref = useRef<HTMLDivElement>(null);
  const [t, setT] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setT(1);
      return;
    }
    let raf = 0;
    const io = new IntersectionObserver(
      ([e]) => {
        if (!e.isIntersecting) return;
        io.disconnect();
        const t0 = performance.now();
        const step = (now: number) => {
          const k = Math.min(1, (now - t0) / 1400);
          setT(1 - (1 - k) ** 3);
          if (k < 1) raf = requestAnimationFrame(step);
        };
        raf = requestAnimationFrame(step);
      },
      { threshold: 0.3 },
    );
    io.observe(el);
    return () => {
      io.disconnect();
      cancelAnimationFrame(raf);
    };
  }, []);

  return (
    <div
      ref={ref}
      className="grid gap-px overflow-hidden rounded-[22px] border border-line bg-line shadow-[0_30px_60px_-40px_rgb(13_15_20/0.25)] sm:grid-cols-2 lg:grid-cols-4"
    >
      {stats.map((s) => (
        <div key={s.label} className="flex flex-col gap-3.5 bg-white px-7 pt-8 pb-7">
          <div
            role="text"
            aria-label={`${s.value.toFixed(s.digits ?? 0)} ${s.unit}`}
            className="flex items-baseline gap-1.5 text-[clamp(40px,4.2vw,54px)] leading-none font-semibold tracking-[-0.04em] tabular-nums"
          >
            <span aria-hidden="true">{(s.value * t).toFixed(s.digits ?? 0)}</span>
            <span aria-hidden="true" className="text-[22px] tracking-[-0.01em] text-faint">
              {s.unit}
            </span>
          </div>
          <div aria-hidden="true" className="h-1 overflow-hidden rounded bg-[#f0efea]">
            <div
              className="h-full rounded bg-[linear-gradient(90deg,#0f9d7a,#5b5ef0)] transition-[width] duration-[1200ms] ease-[cubic-bezier(0.2,0.8,0.2,1)] motion-reduce:transition-none"
              style={{ width: t > 0 ? `${Math.round(s.share * 100)}%` : '0%' }}
            />
          </div>
          <p className="m-0 text-[15px] leading-normal text-muted">{s.label}</p>
          <p className="m-0 mt-auto font-mono text-[11.5px] text-faint">{s.note}</p>
        </div>
      ))}
    </div>
  );
}
