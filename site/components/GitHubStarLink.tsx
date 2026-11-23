'use client';

import { useEffect, useState } from 'react';

/** Where `/api/...` goes (see `lib/engine.js`). */
const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? '';

/** 12345 → "12.3k", 1500000 → "1.5M". */
function short(n: number) {
  if (n < 1000) return String(n);
  if (n < 1e6) return `${(n / 1e3).toFixed(n < 1e5 ? 1 : 0).replace(/\.0$/, '')}k`;
  return `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M`;
}

/**
 * The star button. Starts with the build-time count (`initial`) and replaces
 * it with the current one from the data API; keeps it if the API fails.
 */
export function GitHubStarLink({
  repo,
  initial,
  label,
  labelCount,
}: {
  repo: string;
  initial: number | null;
  label: string;
  labelCount: string;
}) {
  const [n, setN] = useState(initial);
  useEffect(() => {
    const ctl = new AbortController();
    fetch(`${API_BASE}/api/github/stars`, { signal: ctl.signal })
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => {
        if (d && d.repo === repo && typeof d.stars === 'number') setN(d.stars);
      })
      .catch(() => {});
    return () => ctl.abort();
  }, [repo]);
  return (
    <a
      id="github-star"
      href={`https://github.com/${repo}`}
      target="_blank"
      rel="noopener"
      aria-label={n === null ? label : labelCount.replace('{n}', n.toLocaleString('en-US'))}
      className="inline-flex h-[34px] items-center gap-2 rounded-full bg-ink pr-3 pl-[11px] text-[13.5px] font-medium text-white shadow-[0_6px_18px_-10px_rgb(13_15_20/0.6)] transition-colors duration-200 hover:bg-[#2a2b31]"
    >
      <svg viewBox="0 0 16 16" aria-hidden="true" className="size-[15px] fill-current">
        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
      </svg>
      Star
      {n !== null && <span className="font-mono text-xs tabular-nums opacity-65">{short(n)}</span>}
    </a>
  );
}
