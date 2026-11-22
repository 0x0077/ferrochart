'use client';

import { useEffect, useRef } from 'react';
import { useSearchParams } from 'next/navigation';
import { findDemo } from '@/catalog';

declare global {
  interface Window {
    __thumb?: { ready: boolean; error: string | null };
  }
}

/** One example, full-page; sets `window.__thumb.ready` once it has drawn. */
export function ThumbChart() {
  const id = useSearchParams().get('demo');
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const status = { ready: false, error: null as string | null };
    window.__thumb = status;
    document.documentElement.classList.add('thumb-page');
    let dispose: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const engine = await import('@/lib/engine.js');
        const demo = id ? findDemo(id) : undefined;
        if (!demo) throw new Error(`unknown demo '${id}'`);
        const canvas = ref.current;
        if (!canvas || cancelled) return;
        const handle = await engine.run(demo, canvas);
        dispose = () => void engine.dispose(handle, null);
        if (cancelled) { dispose(); return; }
        handle.stop?.();
        handle.chart?.setCrosshairMode('hidden');
        // A few frames for the first render to land.
        await new Promise((r) => setTimeout(r, 400));
        status.ready = true;
      } catch (e) {
        status.error = String((e as Error)?.message ?? e);
      }
    })();
    return () => {
      cancelled = true;
      dispose?.();
      document.documentElement.classList.remove('thumb-page');
    };
  }, [id]);

  return <canvas id="chart" className="thumb-canvas" ref={ref} />;
}
