import { CodeTabs } from '@/components/CodeTabs';
import { HeroArt } from '@/components/HeroArt';
import { Library } from '@/components/Library';
import { PerfStats, type Stat } from '@/components/PerfStats';
import { SiteFooter } from '@/components/SiteFooter';
import { SiteHeader } from '@/components/SiteHeader';
import { Thumb } from '@/components/Thumb';
import { allDemos, categories, preludeOf, type Category, type Demo } from '@/catalog';
import { CatalogProvider } from '@/components/I18n';
import { catalogText } from '@/lib/catalog-text';
import { fmt, localePath, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';

const pad = 'px-[clamp(20px,6vw,96px)]';
const eyebrow = 'mb-[18px] font-mono text-xs font-medium uppercase tracking-[0.16em] text-muted';

/**
 * Figures from the repository's benchmarks (README, examples/compare) and the
 * current wasm build. Bars: each figure as a share of its target, the 300 ms
 * load target (P1), no allocations, the 1.5 MB size budget (P7).
 */
const FIGURES = [
  { value: 43, share: 43 / 300 },
  { value: 73, share: 73 / 300 },
  { value: 0, share: 0 },
  { value: 1.4, digits: 1, share: 1_443_801 / 1_572_864 },
];

/** Live examples shown in the closing band (their pre-rendered images). */
const SHOWCASE = [
  { id: 'candles', path: '/charts/price/candles', wide: true },
  { id: 'depth', path: '/charts/orderflow/depth' },
  { id: 'heatmap', path: '/charts/orderflow/heatmap' },
];

/**
 * The homepage. With `open` (the page of one example, /charts/<category>/<chart>)
 * the viewer opens on that example once the page loads.
 */
export function Home({ locale = 'en', open }: { locale?: Locale; open?: { cat: Category; demo: Demo } }) {
  const t = messages(locale).home;
  const p = (path: string) => localePath(locale, path);
  const stats: Stat[] = FIGURES.map((f, i) => ({ ...f, ...t.stats[i] }));
  const features = t.features.map(([title, text], i) => [String(i + 1).padStart(2, '0'), title, text]);
  return (
    <CatalogProvider text={catalogText(locale, open?.demo.id)}>
      <SiteHeader locale={locale} floating />

      <main id="top">
        <section className={`relative flex min-h-[640px] items-center overflow-hidden ${open ? 'min-h-screen py-28' : 'h-screen'}`}>
          {open ? (
            <img
              src={`/thumbs/${open.demo.id}.jpg`}
              alt={open.demo.title}
              width={1280}
              height={800}
              className="absolute right-0 h-full w-full object-cover opacity-20 md:w-3/5 md:opacity-40"
            />
          ) : <HeroArt />}
          {/* A soft white light behind the headline, kept off the chart on the right (below the buttons on phones). */}
          <div aria-hidden="true" className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_90%_45%_at_30%_38%,rgb(255_255_255/0.9),rgb(255_255_255/0.5)_50%,transparent_100%)] md:bg-[radial-gradient(ellipse_42%_60%_at_20%_45%,rgb(255_255_255/0.9),rgb(255_255_255/0.5)_50%,transparent_100%)]" />
          <div className="pointer-events-none relative ml-[clamp(20px,6vw,96px)] max-w-[640px]">
            <p className={eyebrow}>{t.eyebrow}</p>
            <h1 className="hero-title text-[clamp(46px,7vw,104px)] leading-[0.95] tracking-[-0.05em]">
              {open ? open.demo.title : <>{t.headline1}<br /><span className="glow">{t.headline2}</span></>}
            </h1>
            <p className="mt-7 mb-9 max-w-[520px] text-[clamp(16px,1.4vw,19px)] text-pretty text-muted">
              {open ? open.demo.about.replace(/`/g, '') : t.lede}
            </p>
            <div className="pointer-events-auto flex flex-wrap items-center gap-3">
              <a
                className="group inline-flex h-[52px] items-center gap-2.5 rounded-full bg-ink pr-[22px] pl-[26px] text-[16.5px] font-medium text-white shadow-[0_8px_24px_-10px_rgb(13_15_20/0.6)] transition-colors duration-200 hover:bg-[#24252b]"
                href="#start"
              >
                {t.getStarted}
                <span aria-hidden="true" className="inline-block transition-transform duration-[250ms] ease-out group-hover:translate-x-1">→</span>
              </a>
              <a
                className="inline-flex h-[52px] items-center rounded-full bg-[linear-gradient(90deg,#0f9d7a,#1a8fd0_50%,#6d4cf5)] p-[1.5px] shadow-[0_10px_28px_-14px_rgb(91_94_240/0.7)] transition-shadow duration-300 hover:shadow-[0_14px_34px_-12px_rgb(91_94_240/0.85)]"
                href="https://ferrochart.com/pro"
              >
                <span className="flex h-full items-center gap-2.5 rounded-full bg-white pr-2 pl-[22px] text-[16.5px] font-medium text-ink">
                  {t.openPro}
                  <span className="flex h-[34px] items-center rounded-full bg-[#f1f0f9] px-3 text-[12.5px] font-medium text-[#4f46e5] max-sm:hidden">
                    {t.proTag}
                  </span>
                </span>
              </a>
            </div>
            <div className="mt-[26px] flex flex-wrap items-center gap-2 text-[12.5px] font-medium">
              <span className="rounded-full bg-[#ecebe5] px-[11px] py-[5px] text-muted">Apache-2.0</span>
            </div>
          </div>
          <a
            className="absolute bottom-7 left-1/2 -ml-[11px] h-9 w-[22px] rounded-xl border border-line"
            href="#library"
            aria-label={t.scroll}
          >
            <span className="scroll-wheel absolute top-[7px] left-1/2 -ml-[1.5px] h-[7px] w-[3px] rounded-sm bg-muted" />
          </a>
        </section>

        {open && (
          <section className={`${pad} py-12`} aria-label={messages(locale).viewer.code}>
            <h2 className="mb-5 text-2xl">{fmt(t.exampleOf, { title: open.demo.title, cat: open.cat.title })}</h2>
            <p className="mb-5 text-muted">{open.demo.api.join(' · ')}</p>
            <details className="rounded-2xl border border-line bg-white p-6">
              <summary className="cursor-pointer font-medium">{messages(locale).viewer.code}</summary>
              <pre className="mt-5 overflow-x-auto text-sm"><code>{`${preludeOf(open.demo)}\n${open.demo.code}`}</code></pre>
            </details>
          </section>
        )}

        <section id="performance" aria-label={t.perfAria} className="mx-auto max-w-[1200px] scroll-mt-24 px-[clamp(20px,6vw,40px)]">
          <div className="mb-7 flex flex-wrap items-baseline justify-between gap-x-5 gap-y-2">
            <h2 className="m-0 text-[clamp(26px,2.6vw,32px)] tracking-[-0.03em]">{t.perfTitle}</h2>
            <span className="text-[13px] text-faint">Apple M4 Max · Chrome + WebGPU</span>
          </div>
          <PerfStats stats={stats} />
          <p className="mt-3.5 text-xs text-faint">
            {t.perfNote}
          </p>
        </section>

        <section id="library" className={`${pad} pt-[clamp(96px,12vw,160px)]`}>
          <p className={eyebrow}>{t.examplesEyebrow}</p>
          <h2 className="text-[clamp(34px,4.6vw,64px)] leading-[1.02]">
            {fmt(t.examplesTitle1, { n: allDemos.length })}
            <br />
            {fmt(t.examplesTitle2, { c: categories.length })}
          </h2>
          <p className="mt-6 max-w-[56ch] text-[clamp(16px,1.3vw,18px)] text-muted">
            {t.examplesLede}
          </p>
          <Library initial={open && { cat: open.cat.id, demo: open.demo.id }} />
        </section>

        <section id="features" className={`${pad} scroll-mt-16 pt-[clamp(96px,12vw,160px)]`}>
          <p className={eyebrow}>{t.featuresEyebrow}</p>
          <h2 className="max-w-[18ch] text-[clamp(34px,4.6vw,64px)] leading-[1.02]">{t.featuresTitle}</h2>
          <div className="mt-12 grid gap-px overflow-hidden rounded-[20px] border border-line bg-line md:grid-cols-2 desk:grid-cols-3">
            {features.map(([n, title, text]) => (
              <div key={n} className="bg-white px-7 py-8">
                <span className="font-mono text-xs text-faint">{n}</span>
                <h3 className="mt-5 mb-2.5 text-[19px] font-semibold tracking-[-0.02em]">{title}</h3>
                <p className="m-0 text-[15px] text-muted">{text}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="start" className={`${pad} scroll-mt-16 grid items-center gap-x-[clamp(32px,5vw,80px)] gap-y-10 pt-[clamp(96px,12vw,160px)] desk:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]`}>
          <div>
            <p className={eyebrow}>{t.startEyebrow}</p>
            <h2 className="text-[clamp(34px,4.6vw,64px)] leading-[1.02]">{t.startTitle}</h2>
            <p className="mt-6 max-w-[44ch] text-[clamp(16px,1.3vw,18px)] text-muted">
              {t.startLede}
            </p>
            <ul className="mt-8 grid gap-2.5 p-0 text-[15px]">
              {t.langs.map((l) => (
                <li key={l} className="flex list-none items-center gap-3">
                  <span aria-hidden="true" className="size-1.5 rounded-full bg-ink" />
                  {l}
                </li>
              ))}
            </ul>
            <a href={p('/docs')} className="mt-8 inline-flex items-center gap-2 font-medium underline decoration-ink/25 underline-offset-4 hover:decoration-ink">
              {t.readDocumentation} <span aria-hidden="true">→</span>
            </a>
          </div>
          <CodeTabs />
        </section>

        <section className={`${pad} pt-[clamp(96px,12vw,160px)]`}>
          <div className="relative mx-auto grid max-w-[1400px] items-center gap-10 overflow-hidden rounded-[32px] bg-[#15161b] px-[clamp(24px,5vw,72px)] py-[clamp(48px,6vw,80px)] text-on-chart lg:grid-cols-2">
            <div aria-hidden="true" className="pointer-events-none absolute -top-[30%] -right-[10%] h-[160%] w-[70%] bg-[radial-gradient(closest-side,rgb(91_94_240/0.22),transparent_70%)]" />
            <div className="relative">
              <p className="m-0 font-mono text-xs uppercase tracking-[0.24em] text-on-chart-muted">FerroChart</p>
              <h2 className="mt-6 text-[clamp(40px,6vw,84px)] leading-[0.98] tracking-[-0.045em] text-white">
                {t.closing1}
                <br />
                {t.closing2}
                <br />
                <span className="bg-[linear-gradient(90deg,#1a8fd0,#6d4cf5_55%,#c04f9b)] bg-clip-text text-transparent">{t.closing3}</span>
              </h2>
              <p className="mt-6 mb-8 max-w-[440px] text-[clamp(16px,1.3vw,17px)] text-on-chart-muted">
                {t.closingLede}
              </p>
              <div className="flex flex-wrap gap-3">
                <a href="#start" className="group inline-flex h-12 items-center gap-2.5 rounded-full bg-white px-[22px] text-[15.5px] font-medium text-ink transition-colors hover:bg-[#e9e8ff]">
                  {t.getStarted} <span aria-hidden="true" className="transition-transform duration-[250ms] group-hover:translate-x-1">→</span>
                </a>
                <a href={p('/docs')} className="inline-flex h-12 items-center rounded-full border border-[#33343c] px-5 text-[15.5px] text-on-chart transition-colors hover:border-[#55565f] hover:text-white">
                  {t.readDocs}
                </a>
              </div>
            </div>
            <div className="relative grid min-w-0 grid-cols-[1.3fr_1fr] grid-rows-[200px_150px] gap-3.5 max-sm:grid-cols-1 max-sm:grid-rows-[200px_140px_140px]">
              {SHOWCASE.map((x, i) => (
                <a
                  key={x.id}
                  href={p(x.path)}
                  className={`group flex min-w-0 flex-col gap-2 rounded-[18px] border border-[#2a2b32] bg-[#1d1e24] p-3.5 transition-colors hover:border-[#3d3e48] ${x.wide ? 'sm:row-span-2' : ''}`}
                >
                  <span className="flex justify-between font-mono text-[11px] uppercase tracking-[0.06em] text-on-chart-muted">
                    {t.showcase[i]}
                    <span aria-hidden="true" className="opacity-0 transition-opacity group-hover:opacity-100">↗</span>
                  </span>
                  <span className="relative min-h-0 flex-1 overflow-hidden rounded-[10px]">
                    <span className="absolute inset-0"><Thumb id={x.id} /></span>
                  </span>
                </a>
              ))}
            </div>
          </div>
        </section>
      </main>

      <SiteFooter locale={locale} />
    </CatalogProvider>
  );
}
