'use client';

import { useEffect, useState, type CSSProperties } from 'react';
import { categories, type Category, type Demo } from '@/catalog';
import { Thumb } from './Thumb';
import { Viewer, type OpenRequest } from './Viewer';

const STACK_DEPTH = 3;
/** The use cases shown as stacks; the rest open from the pills or "Browse all". */
const FEATURED = ['price', 'orderflow', 'perps', 'options', 'quant', 'custom'];
/** Card i of a stack at rest, and fanned out on hover (globals.css `.card`). */
const REST = ['none', 'translate(10px, -10px) rotate(2deg) scale(0.97)', 'translate(20px, -20px) rotate(4deg) scale(0.94)'];
const FAN = ['translate(-10px, -14px) rotate(-1.5deg)', 'translate(22px, 4px) rotate(4deg)', 'translate(48px, 18px) rotate(9deg)'];

function Card({ demo, i }: { demo: Demo; i: number }) {
  const [loaded, setLoaded] = useState(false);
  return (
    <figure
      data-demo={demo.id}
      style={{ '--i': i, '--rest': REST[i], '--fan': FAN[i] } as CSSProperties}
      className={`card ${i === 0 ? 'top' : ''} ${loaded ? '' : 'shimmer'} absolute inset-0 m-0 overflow-hidden rounded-2xl border border-line bg-chart shadow-[0_30px_60px_-30px_rgb(13_15_20/0.45)]`}
    >
      <Thumb id={demo.id} onLoad={() => setLoaded(true)} />
      <figcaption className="absolute bottom-3 left-3.5 rounded-full bg-white/85 px-2.5 py-[5px] font-mono text-xs leading-none font-medium text-ink backdrop-blur-sm">
        {demo.title}
      </figcaption>
    </figure>
  );
}

function Stack({ cat, onOpen }: { cat: Category; onOpen: (card: HTMLElement | null) => void }) {
  const open = (el: HTMLElement) => onOpen(el.querySelector('.card.top'));
  return (
    <div
      className="stack group grid scroll-mt-28 cursor-pointer content-start gap-[34px] outline-none"
      id={`cat-${cat.id}`}
      data-cat={cat.id}
      tabIndex={0}
      role="button"
      aria-label={`${cat.title}: ${cat.demos.length} examples`}
      style={{ '--accent': cat.accent } as CSSProperties}
      onClick={(e) => open(e.currentTarget)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          open(e.currentTarget);
        }
      }}
    >
      <div className="relative mt-10 mr-[56px] aspect-[16/10] rounded-[18px] outline-offset-[12px] group-focus-visible:outline-2 group-focus-visible:outline-[var(--accent)]">
        {/* Bottom card first: later cards paint on top. */}
        {cat.demos
          .slice(0, STACK_DEPTH)
          .map((d, i) => <Card key={d.id} demo={d} i={i} />)
          .reverse()}
      </div>
      <div className="grid grid-cols-[auto_1fr_auto] items-baseline gap-x-[18px] gap-y-1.5">
        <span className="font-mono text-[13px] font-medium accent-ink">{cat.kicker}</span>
        <h3 className="text-[clamp(22px,1.9vw,28px)]">{cat.title}</h3>
        <span className="text-sm text-muted transition duration-300 ease-spring group-hover:translate-x-1 group-hover:text-ink">
          {cat.demos.length} examples →
        </span>
        <p className="col-start-2 col-end-[-1] m-0 max-w-[46ch] text-muted">{cat.blurb}</p>
      </div>
    </div>
  );
}

/**
 * The chart library: a stack of cards for a few use cases, pills for all of
 * them, and the viewer that opens over the page instead of a second-level
 * page.
 */
export function Library() {
  const [request, setRequest] = useState<OpenRequest | null>(null);
  const featured = FEATURED.map((id) => categories.find((c) => c.id === id)).filter((c): c is Category => !!c);
  const openCat = (cat: Category, card: HTMLElement | null = null) => setRequest({ cat, demo: cat.demos[0], card });

  // Deep links: #/<category>/<chart>, on load and when a link on the page sets one.
  useEffect(() => {
    const open = () => {
      const [, catId, demoId] = location.hash.match(/^#\/([\w-]+)\/([\w-]+)/) ?? [];
      const cat = categories.find((c) => c.id === catId);
      if (cat) setRequest({ cat, demo: cat.demos.find((d) => d.id === demoId) ?? cat.demos[0], card: null });
    };
    open();
    window.addEventListener('hashchange', open);
    return () => window.removeEventListener('hashchange', open);
  }, []);

  return (
    <>
      <nav aria-label="Use cases" className="mt-10 flex gap-2 max-md:-mx-[clamp(20px,6vw,96px)] max-md:overflow-x-auto max-md:px-[clamp(20px,6vw,96px)] max-md:pb-1 max-md:[scrollbar-width:none] md:flex-wrap">
        {categories.map((cat) => (
          <button
            key={cat.id}
            type="button"
            data-cat={cat.id}
            onClick={() => openCat(cat)}
            style={{ '--accent': cat.accent } as CSSProperties}
            className="cat-pill shrink-0 cursor-pointer whitespace-nowrap rounded-full border border-line bg-white/70 px-3.5 py-1.5 text-[13px] text-muted transition duration-300 ease-spring hover:border-[var(--accent)] hover:text-ink"
          >
            {cat.title} <span className="font-mono text-xs text-faint">{cat.demos.length}</span>
          </button>
        ))}
      </nav>
      <div id="stacks" className="mt-[clamp(48px,6vw,80px)] grid gap-x-[clamp(28px,3.4vw,56px)] gap-y-[clamp(48px,6vw,88px)] md:grid-cols-2 desk:grid-cols-3">
        {featured.map((cat) => (
          <Stack key={cat.id} cat={cat} onOpen={(card) => openCat(cat, card)} />
        ))}
      </div>
      <div className="mt-[clamp(56px,6vw,88px)] flex justify-center">
        <button
          type="button"
          id="browse-all"
          onClick={() => setRequest({ cat: categories[0], demo: categories[0].demos[0], card: null, all: true })}
          className="group inline-flex h-12 cursor-pointer items-center gap-3 rounded-full border border-line bg-white px-6 font-medium shadow-[0_14px_40px_-24px_rgb(13_15_20/0.4)] transition duration-300 ease-spring hover:-translate-y-0.5"
        >
          Browse all {categories.length} use cases
          <span className="transition duration-300 ease-spring group-hover:translate-x-1">→</span>
        </button>
      </div>
      <Viewer request={request} onClosed={() => setRequest(null)} />
    </>
  );
}
