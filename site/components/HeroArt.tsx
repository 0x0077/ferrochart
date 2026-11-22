'use client';

import { useEffect, useRef } from 'react';

/** The hero's particle artwork (lib/hero.js), started in the browser. */
export function HeroArt() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    import('@/lib/hero.js').then(({ startHero }) => {
      if (!cancelled && ref.current) stop = startHero(ref.current);
    });
    return () => {
      cancelled = true;
      stop?.();
    };
  }, []);
  // Fills the hero section behind the text (the section is `relative`).
  return <canvas id="hero-art" ref={ref} aria-hidden="true" className="absolute inset-0 block size-full" />;
}
