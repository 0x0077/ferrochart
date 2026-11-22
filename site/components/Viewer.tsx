'use client';

import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { allDemos, categories, groupsOf, preludeOf, type Category, type Demo } from '@/catalog';
import { highlight } from '@/lib/highlight.js';
import { grow, shrink } from '@/lib/reveal.js';
import { Thumb } from './Thumb';
import { AllCharts } from './AllCharts';
import { Details } from './Details';

/** `all`: open on the all-charts layer (the homepage's "Browse all"). */
export type OpenRequest = { cat: Category; demo: Demo; card: HTMLElement | null; all?: boolean };

type Reveal = { started: Promise<void>; done: Promise<void>; finish(): void };
type Stage = {
  show(d: Demo, mode: 'switch' | 'intro' | 'plain'): Promise<void>;
  reveal(o: { duration?: number }): Reveal;
  skip(): void;
  clear(): Promise<void>;
  destroy(): Promise<void>;
  moveTo(el: HTMLElement | null): void;
};

type Phase = 'hidden' | 'entering' | 'open' | 'closing';

declare global {
  interface Window {
    /** For automated browser checks (tests/web/site-check.mjs). */
    __site?: { ready: boolean; shown: string | null; errors: string[]; show?: (id: string) => void };
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
const catOf = (d: Demo) => categories.find((c) => c.demos.includes(d));
/** The example as a user would write it. */
export const source = (d: Demo) => `${preludeOf(d)}\n${d.code}`;

/**
 * The opening: the panel grows out of the card with the chart in it, empty
 * (grid and axes); then the engine draws the data from left to right
 * (`playIntro`); the rest of the viewer arrives as the drawing ends.
 */
const GROW_MS = 680;
const DRAW_MS = 900;
/** Longest wait for the chart to start drawing before the rest arrives anyway. */
const DRAW_WAIT_MS = 1500;
/** The rest of the viewer arrives when the drawing is this far across. */
const SHELL_AT = 0.75;

/** Whether `el` is at least partly in the viewport (the page may have scrolled under the viewer). */
function onScreen(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;
}

function matches(d: Demo, q: string): boolean {
  const hay = `${d.title} ${d.group ?? ''} ${d.api.join(' ')} ${d.about} ${d.id}`.toLowerCase();
  return q.split(/\s+/).every((w) => hay.includes(w));
}

const pill =
  'inline-flex h-9 shrink-0 cursor-pointer items-center justify-center gap-2 rounded-full border border-line px-3 text-[13px] whitespace-nowrap text-ink transition hover:border-ink/20 hover:bg-ink/[0.04] desk:px-3.5 aria-pressed:border-ink/25 aria-pressed:bg-ink/[0.06]';
const round =
  'grid size-9 shrink-0 cursor-pointer place-items-center rounded-full border border-line text-muted transition hover:border-ink/20 hover:text-ink';

/**
 * The chart viewer over the blurred page: the category's examples on the
 * left (searchable across all categories), the title, notes and controls
 * on top, the live chart below. Integration code is a drawer over the
 * chart; "All charts" and "Details" are layers.
 */
export function Viewer({ request, onClosed }: { request: OpenRequest | null; onClosed: () => void }) {
  const [phase, setPhase] = useState<Phase>('hidden');
  const [shell, setShell] = useState(false);
  /** 'grow': the panel grows out of the clicked card; 'fade': deep links, reduced motion. */
  const [motion, setMotion] = useState<'grow' | 'fade'>('fade');
  const [cat, setCat] = useState<Category>(categories[0]);
  const [demo, setDemo] = useState<Demo>(categories[0].demos[0]);
  const [query, setQuery] = useState('');
  const [live, setLive] = useState('Starting…');
  const [renderer, setRenderer] = useState<string | null>(null);
  const [copy, setCopy] = useState('Copy');
  const [code, setCode] = useState(false);
  const [all, setAll] = useState(false);
  const [details, setDetails] = useState(false);
  const host = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const closeBtn = useRef<HTMLButtonElement>(null);
  const listEl = useRef<HTMLDivElement>(null);
  const stage = useRef<Promise<Stage> | null>(null);
  const phaseRef = useRef<Phase>('hidden');
  phaseRef.current = phase;
  const accentRef = useRef(categories[0].accent);
  accentRef.current = cat.accent;
  const catRef = useRef(cat);
  catRef.current = cat;
  /** The card the viewer grew out of (hidden meanwhile), and the skip of the opening. */
  const card = useRef<HTMLElement | null>(null);
  const skip = useRef<(() => void) | null>(null);

  // The imperative stage, made once.
  useEffect(() => {
    const site: NonNullable<Window['__site']> = { ready: false, shown: null, errors: [] };
    window.__site = site;
    let made: Stage | null = null;
    let dead = false;
    stage.current = import('@/lib/stage.js').then((m) => {
      const s: Stage = m.createStage(host.current as HTMLElement, {
        onShown: (d: Demo, r: string) => {
          setLive(`Live · ${r}`);
          setRenderer(r);
          site.shown = d.id;
        },
        onError: (d: Demo, message: string) => {
          setLive('Failed');
          site.errors.push(`${d.id}: ${message}`);
        },
        accent: () => accentRef.current,
      });
      if (dead) s.destroy();
      made = s;
      return s;
    });
    site.ready = true;
    return () => {
      dead = true;
      made?.destroy();
    };
  }, []);

  const select = useCallback((d: Demo) => {
    const c = catOf(d);
    if (c) setCat(c);
    setDemo(d);
    setLive('Starting…');
    skip.current?.();
    stage.current?.then((s) => s.show(d, 'switch'));
  }, []);

  // Test hook: switch to a chart by id.
  useEffect(() => {
    if (window.__site) window.__site.show = (id) => {
      const d = allDemos.find((x) => x.id === id);
      if (d) select(d);
    };
  }, [select]);

  // Open on request.
  useEffect(() => {
    if (!request) return;
    setCat(request.cat);
    setDemo(request.demo);
    setQuery('');
    setLive('Starting…');
    setCode(false);
    setAll(request.all ?? false);
    setDetails(false);
    setShell(false);
    setMotion(request.card && !reducedMotion() ? 'grow' : 'fade');
    setPhase('entering');
  }, [request]);

  // After the viewer is laid out: grow out of the card, draw the chart, then
  // bring in the rest. Any click, key or wheel skips to the end.
  useEffect(() => {
    if (phase !== 'entering' || !request) return;
    document.body.classList.add('viewer-open');
    const hero = document.getElementById('hero-art');
    if (hero) hero.dataset.paused = '1';
    const from = request.card;
    const p = panel.current as HTMLDivElement;
    requestAnimationFrame(() => setPhase('open'));
    closeBtn.current?.focus({ preventScroll: true });
    if (motion !== 'grow' || !from) {
      setShell(true);
      stage.current?.then((s) => s.show(request.demo, 'plain'));
      return;
    }
    let cancelled = false;
    let growth: Animation | null = null;
    let shellTimer = 0;
    card.current = from;
    growth = grow(p, from.getBoundingClientRect(), { duration: GROW_MS });
    from.style.opacity = '0';
    stage.current?.then((s) => s.show(request.demo, 'intro'));
    const finish = () => {
      if (cancelled) return;
      cancelled = true;
      clearTimeout(shellTimer);
      try { growth?.finish(); } catch { /* gone */ }
      stage.current?.then((s) => s.skip());
      setShell(true);
      off();
    };
    const onInput = (e: Event) => {
      if (e.type === 'keydown' && (e as KeyboardEvent).key === 'Escape') return; // closes
      finish();
    };
    const events = ['pointerdown', 'keydown', 'wheel'] as const;
    const off = () => events.forEach((t) => removeEventListener(t, onInput, true));
    events.forEach((t) => addEventListener(t, onInput, { capture: true, passive: true }));
    skip.current = finish;
    (async () => {
      // The reveal waits for a chart that is still starting.
      await sleep(GROW_MS);
      if (cancelled) return;
      const s = await stage.current;
      if (!s || cancelled) return;
      const r = s.reveal({ duration: DRAW_MS });
      await Promise.race([r.started, sleep(DRAW_WAIT_MS)]);
      if (cancelled) return;
      shellTimer = window.setTimeout(() => setShell(true), DRAW_MS * SHELL_AT);
      await r.done;
      if (!cancelled) {
        cancelled = true;
        setShell(true);
        off();
      }
    })();
    // No cleanup: the phase turns to 'open' right after, and the opening
    // goes on. Closing (or any input) ends it through `skip`.
    // Runs once per request.
  }, [phase === 'entering', request]);

  // The address follows the chart: the example's own page (app/charts).
  useEffect(() => {
    if (phase === 'open' || phase === 'entering') history.replaceState(null, '', `/charts/${cat.id}/${demo.id}${location.search}`);
  }, [phase, cat, demo]);

  const close = useCallback(() => {
    if (phaseRef.current === 'hidden' || phaseRef.current === 'closing') return;
    skip.current?.();
    setPhase('closing');
    setShell(false);
    setDetails(false);
    setAll(false);
    setCode(false);
    history.replaceState(null, '', `/${location.search}`); // back to the homepage's address
    const p = panel.current as HTMLDivElement;
    const from = card.current;
    // Back into the current category's top card (the category may have changed).
    const to = document.querySelector<HTMLElement>(`#cat-${catRef.current.id} .card.top`);
    const back = !!to && !reducedMotion() && p.dataset.motion === 'grow' && onScreen(to);
    // Shrink into the card with the chart still on it; destroy it after.
    // (Hidden only once the chart is destroyed: a hidden canvas has no size.)
    if (!back) setMotion('fade'); // fades out instead
    if (back && to !== from) {
      if (from) from.style.opacity = '';
      to.style.opacity = '0';
    }
    const shrunk = back ? shrink(p, to.getBoundingClientRect()).finished.catch(() => {}) : sleep(480);
    shrunk
      .then(() => stage.current?.then((s) => s.clear()))
      .then(() => {
        if (phaseRef.current !== 'closing') return; // reopened meanwhile
        p.getAnimations().forEach((a) => a.cancel());
        if (from) from.style.opacity = '';
        if (to) to.style.opacity = '';
        card.current = null;
        setPhase('hidden');
        document.body.classList.remove('viewer-open');
        const hero = document.getElementById('hero-art');
        if (hero) delete hero.dataset.paused;
        onClosed();
      });
  }, [onClosed]);

  // What the list shows: the category by group, or search hits everywhere.
  const q = query.trim().toLowerCase();
  const sections = useMemo(() => {
    if (!q) return groupsOf(cat.demos);
    return categories
      .map((c) => ({ name: c.title, demos: c.demos.filter((d) => matches(d, q)) }))
      .filter((s) => s.demos.length);
  }, [cat, q]);
  const flat = useMemo(() => sections.flatMap((s) => s.demos), [sections]);

  /** Previous or next: in the search hits, or around the category. */
  const step = useCallback((by: number) => {
    const list = q ? flat : cat.demos;
    if (!list.length) return;
    const i = list.indexOf(demo);
    select(list[(Math.max(i, 0) + by + list.length) % list.length]);
  }, [q, flat, cat, demo, select]);

  useEffect(() => {
    if (phase === 'hidden') return;
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing = t?.tagName === 'INPUT';
      if (e.key === 'Escape') {
        if (details) setDetails(false);
        else if (typing && query) setQuery('');
        else if (all) setAll(false);
        else if (code) setCode(false);
        else close();
        return;
      }
      if (details || typing || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === 'a' || e.key === 'A') setAll((v) => !v);
      else if (e.key === 'c' || e.key === 'C') setCode((v) => !v);
      else if (/^Arrow(Up|Down|Left|Right)$/.test(e.key) && !t?.closest('canvas')) {
        e.preventDefault();
        step(e.key === 'ArrowDown' || e.key === 'ArrowRight' ? 1 : -1);
      }
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, [phase, details, all, code, query, close, step]);

  // Keep the current item in view.
  useEffect(() => {
    listEl.current
      ?.querySelector(`[data-demo="${demo.id}"]`)
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
  }, [demo, sections]);

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(source(demo));
      setCopy('Copied');
    } catch {
      setCopy('Copy failed');
    }
    setTimeout(() => setCopy('Copy'), 1400);
  };

  const moveChart = useCallback((el: HTMLElement | null) => {
    stage.current?.then((s) => s.moveTo(el));
  }, []);

  const pos = cat.demos.indexOf(demo);
  const html = useMemo(() => highlight(source(demo)), [demo]);
  return (
    <div
      id="viewer"
      className="viewer fixed inset-0 z-50"
      role="dialog"
      aria-modal="true"
      aria-labelledby="stage-title"
      hidden={phase === 'hidden'}
      data-state={phase === 'open' ? 'open' : 'closed'}
      data-phase={phase}
      style={{ '--accent': cat.accent } as CSSProperties}
    >
      <div className="backdrop absolute inset-0 bg-[rgb(248_247_244/0.55)] backdrop-blur-[22px] backdrop-saturate-[1.2]" onClick={close} />
      <div
        ref={panel}
        data-shell={shell ? 'on' : 'off'}
        data-motion={motion}
        data-all={all ? 'on' : 'off'}
        className="panel absolute inset-0 grid grid-cols-1 grid-rows-[auto_auto_minmax(320px,1fr)] gap-x-5 gap-y-3 overflow-y-auto border-line bg-glass p-4 shadow-[0_60px_120px_-40px_rgb(13_15_20/0.35)] backdrop-blur-xl tab:inset-[clamp(8px,1.8vw,24px)] tab:grid-cols-[200px_minmax(0,1fr)] tab:grid-rows-[auto_minmax(0,1fr)] tab:gap-y-0 tab:overflow-visible tab:rounded-[26px] tab:border tab:p-6 tab:pb-5 desk:grid-cols-[248px_minmax(0,1fr)]"
      >
        <aside className="shell-rail flex min-h-0 flex-col tab:row-span-2">
          <div className="px-1 pt-1 pb-3">
            <span className="font-mono text-xs font-medium accent-ink">
              {cat.kicker} · {cat.demos.length} charts
            </span>
            <h2 className="mt-1 text-[22px]">{cat.title}</h2>
          </div>
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search ${allDemos.length} charts`}
            aria-label="Search charts"
            className="mb-3 h-9 rounded-[10px] border border-line bg-white px-3.5 text-[13px] text-ink outline-none placeholder:text-faint focus:border-[var(--accent)]"
          />
          <div
            ref={listEl}
            id="viewer-list"
            className="flex shrink-0 gap-2.5 overflow-x-auto pb-1 tab:block tab:min-h-0 tab:shrink tab:space-y-4 tab:overflow-x-visible tab:overflow-y-auto tab:pr-1 [scrollbar-color:#d3d6dc_transparent] [scrollbar-width:thin]"
          >
            {sections.map((s) => (
              <section key={s.name ?? '-'} className="contents tab:block">
                {s.name && sections.length > 1 && (
                  <h4 className="hidden px-1 pb-2 font-mono text-[11px] tracking-[0.12em] text-faint uppercase tab:block">{s.name}</h4>
                )}
                <ol className="contents tab:grid tab:gap-2.5">
                  {s.demos.map((d) => (
                    <li key={d.id} className="lazy-item w-[150px] shrink-0 tab:w-auto">
                      <button
                        type="button"
                        data-demo={d.id}
                        aria-current={d === demo}
                        onClick={() => select(d)}
                        className="item block w-full cursor-pointer overflow-hidden rounded-[12px] border border-line bg-white text-left transition duration-300 ease-spring hover:border-ink/20 tab:hover:translate-x-[3px] aria-[current=true]:border-[var(--accent)] aria-[current=true]:shadow-[0_0_0_1px_var(--accent),0_16px_40px_-18px_var(--accent)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]"
                      >
                        <div className={`aspect-[16/9.7] bg-chart transition-[filter] ${d === demo ? '' : 'saturate-[0.55]'}`}>
                          <Thumb id={d.id} />
                        </div>
                        <span className="flex items-baseline gap-2 border-t border-line px-3 pt-[9px] pb-2.5">
                          <span className={`truncate text-[13px] ${d === demo ? 'text-ink' : 'text-muted'}`}>{d.title}</span>
                          <span className="ml-auto font-mono text-[10px] text-faint">
                            {q ? catOf(d)?.title : String(cat.demos.indexOf(d) + 1).padStart(2, '0')}
                          </span>
                        </span>
                      </button>
                    </li>
                  ))}
                </ol>
              </section>
            ))}
            {!flat.length && <p className="px-2 text-sm text-faint">No chart matches “{query}”.</p>}
          </div>
        </aside>

        <header className="shell-head flex min-w-0 flex-col gap-2.5 tab:pb-4">
          <div className="flex min-w-0 items-center gap-3.5">
            <h3 id="stage-title" className="min-w-0 flex-1 truncate text-[22px] desk:text-2xl">{demo.title}</h3>
            <span id="stage-live" className="live-dot hidden shrink-0 items-center gap-[7px] font-mono text-xs font-medium text-muted desk:inline-flex">
              {live}
            </span>
            <span className="hidden shrink-0 font-mono text-xs text-faint desk:inline">
              {pos + 1} / {cat.demos.length}
            </span>
            <div className="flex shrink-0 gap-1.5">
              <button type="button" id="all-toggle" className={pill} aria-pressed={all} title="All charts (A)" onClick={() => setAll((v) => !v)}>
                <svg viewBox="0 0 16 16" aria-hidden="true" className="size-3.5 fill-current"><path d="M1 1h6v6H1zM9 1h6v6H9zM1 9h6v6H1zM9 9h6v6H9z" /></svg>
                <span className="hidden desk:inline">All charts</span>
              </button>
              <button type="button" id="details-open" className={pill} title="Details" onClick={() => setDetails(true)}>
                <span className="hidden desk:inline">Details</span>
                <svg viewBox="0 0 16 16" aria-hidden="true" className="size-3.5 fill-none stroke-current stroke-[1.5]"><circle cx="8" cy="8" r="6.5" /><path d="M8 7v4.5M8 4.6v.2" strokeLinecap="round" /></svg>
              </button>
              <button type="button" id="code-toggle" className={`${pill} mr-1.5 font-mono text-xs`} aria-pressed={code} title="Integration code (C)" onClick={() => setCode((v) => !v)}>
                {'</>'}
                <span className="hidden desk:inline">Code</span>
              </button>
              <button type="button" className={`${round} hidden tab:grid`} title="Previous (↑)" aria-label="Previous chart" onClick={() => step(-1)}>↑</button>
              <button type="button" className={`${round} hidden tab:grid`} title="Next (↓)" aria-label="Next chart" onClick={() => step(1)}>↓</button>
              <button
                ref={closeBtn}
                type="button"
                aria-label="Close"
                title="Close (Esc)"
                onClick={close}
                className="close ml-1.5 grid size-9 shrink-0 cursor-pointer place-items-center rounded-full border border-line text-ink transition duration-300 ease-spring hover:rotate-90 hover:bg-ink/[0.06]"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true" className="size-4 stroke-current stroke-2 [stroke-linecap:round]">
                  <path d="M6 6l12 12M18 6L6 18" />
                </svg>
              </button>
            </div>
          </div>
          <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2">
            <p id="about" className="prose-code m-0 max-w-[860px] flex-[1_1_420px] text-sm text-muted" dangerouslySetInnerHTML={{ __html: highlight.inlineCode(demo.about) }} />
            <div id="api" className="flex flex-wrap gap-1.5">
              {demo.api.map((a) => (
                <code key={a} className="rounded-[7px] border border-line bg-white px-2 py-1 font-mono text-[11.5px] text-muted">{a}</code>
              ))}
            </div>
          </div>
        </header>

        <section className="chart-area relative flex min-h-0 min-w-0 flex-col tab:col-start-2">
          <div ref={host} id="canvas-host" className="canvas-host relative min-h-0 flex-1 overflow-hidden rounded-[14px] border border-line bg-chart" />
          <p className="shell-hint mx-1 mt-2.5 mb-0 flex gap-4 text-xs text-faint">
            <span>Drag to pan · scroll or pinch to zoom · double-click to reset</span>
            <span className="ml-auto hidden font-mono tab:inline">{demo.id}</span>
          </p>

          <div
            className="code-drawer absolute top-3 right-3 bottom-9 z-[4] flex w-[min(400px,calc(100%-24px))] flex-col"
            data-open={code ? 'on' : 'off'}
            aria-hidden={!code}
          >
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-chart-line bg-[rgb(10_12_16/0.94)] shadow-[0_24px_60px_rgb(0_0_0/0.45)] backdrop-blur-md">
              <div className="flex items-center gap-2.5 border-b border-chart-line py-2.5 pr-2.5 pl-[18px]">
                <span className="text-[11px] tracking-[0.08em] text-on-chart-muted uppercase">Integration</span>
                <span className="font-mono text-[11px] text-on-chart-muted">JavaScript</span>
                <button
                  id="copy"
                  type="button"
                  tabIndex={code ? 0 : -1}
                  onClick={onCopy}
                  className="ml-auto h-7 cursor-pointer rounded-lg border border-chart-line px-3 text-xs text-on-chart hover:bg-white/5"
                >
                  {copy}
                </button>
                <button type="button" tabIndex={code ? 0 : -1} aria-label="Hide code" onClick={() => setCode(false)} className="grid size-7 cursor-pointer place-items-center rounded-lg text-on-chart-muted hover:bg-white/5 hover:text-on-chart">✕</button>
              </div>
              <pre className="m-0 flex-1 overflow-auto px-[18px] py-3.5 font-mono text-[12px] leading-[1.65] [overflow-wrap:anywhere] whitespace-pre-wrap text-[#c9d1dc] [scrollbar-color:#262b33_transparent] [scrollbar-width:thin]">
                <code id="code" dangerouslySetInnerHTML={{ __html: html }} />
              </pre>
            </div>
          </div>

          {all && <AllCharts current={demo} onPick={(d) => { setAll(false); select(d); }} />}
        </section>
      </div>

      {details && (
        <Details
          cat={cat}
          demo={demo}
          renderer={renderer}
          code={html}
          copyLabel={copy}
          onCopy={onCopy}
          onClose={() => setDetails(false)}
          onSlot={moveChart}
        />
      )}
    </div>
  );
}
