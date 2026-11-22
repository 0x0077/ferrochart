import { CodeTabs } from '@/components/CodeTabs';
import { HeroArt } from '@/components/HeroArt';
import { Library } from '@/components/Library';
import { PerfStats, type Stat } from '@/components/PerfStats';
import { SiteFooter } from '@/components/SiteFooter';
import { SiteHeader } from '@/components/SiteHeader';
import { Thumb } from '@/components/Thumb';
import { allDemos, categories, type Category, type Demo } from '@/catalog';

const pad = 'px-[clamp(20px,6vw,96px)]';
const eyebrow = 'mb-[18px] font-mono text-xs font-medium uppercase tracking-[0.16em] text-muted';

/**
 * Figures from the repository's benchmarks (README, examples/compare) and the
 * current wasm build. Bars: each figure as a share of its target, the 300 ms
 * load target (P1), no allocations, the 1.5 MB size budget (P7).
 */
const stats: Stat[] = [
  { value: 43, unit: 'ms', label: 'Load 1 million bars and draw the first frame in the browser', note: 'WASM · WebGPU', share: 43 / 300 },
  { value: 73, unit: 'ms', label: 'Load 10 million bars natively, level-of-detail pyramid included', note: 'native Rust', share: 73 / 300 },
  { value: 0, unit: 'allocs', label: 'Heap allocations per frame while panning, zooming and streaming', note: 'steady state', share: 0 },
  { value: 1.4, digits: 1, unit: 'MB', label: 'WebAssembly package size, gzip', note: 'all features', share: 1_443_801 / 1_572_864 },
];

/** Live examples shown in the closing band (their pre-rendered images). */
const showcase = [
  { id: 'candles', href: '/charts/price/candles', label: 'BTCUSD · 1m · Candles', wide: true },
  { id: 'depth', href: '/charts/orderflow/depth', label: 'Depth' },
  { id: 'heatmap', href: '/charts/orderflow/heatmap', label: 'Heatmap' },
];

/** What the engine does, each claim backed by the repository (tests, benches, docs). */
const features = [
  ['01', 'Cross-platform rendering', 'A single Rust core with WebGPU and WebGL2 backends in the browser, wgpu on Metal, Vulkan and DirectX 12 in native applications, and a CPU renderer for servers. Golden-image tests keep the backends consistent.'],
  ['02', 'Performance independent of data size', 'Bar data stays on the GPU and coordinate transforms run in shaders, so panning and zooming update only a few uniforms. A level-of-detail pyramid keeps tens of millions of bars responsive.'],
  ['03', '30 built-in indicators', 'Moving averages, channels and oscillators from SMA to Ichimoku. Values are updated incrementally on each tick and tested against reference data.'],
  ['04', '29 drawing tools', 'Trend lines, channels, Fibonacci tools, pitchforks, Gann fans, patterns, Elliott waves, ranges, positions and text, with undo and redo. Drawings and chart state serialize to JSON.'],
  ['05', 'Synchronized charts', 'Sync groups link the crosshair and the visible range across multiple charts.'],
  ['06', 'Extensible', 'Plugin interfaces for custom series, layers and drawing tools. Bindings for JavaScript, Rust and Python, and a C ABI for C#, C++, Qt and Flutter.'],
];

const langs = ['JavaScript & TypeScript', 'Rust', 'Python & Jupyter', 'C, C#, C++'];

/**
 * The homepage. With `open` (the page of one example, /charts/<category>/<chart>)
 * the viewer opens on that example once the page loads.
 */
export function Home({ open }: { open?: { cat: Category; demo: Demo } }) {
  return (
    <>
      <SiteHeader floating />

      <main id="top">
        {open && (
          // What the page is about, for screen readers and search engines (the viewer shows it on screen).
          <section className="sr-only" aria-label={open.demo.title}>
            <h2>
              {open.demo.title}: {open.cat.title} example
            </h2>
            <p>{open.demo.about.replace(/`/g, '')}</p>
          </section>
        )}
        <section className="relative flex h-screen min-h-[640px] items-center overflow-hidden">
          <HeroArt />
          {/* A soft white light behind the headline, kept off the chart on the right (below the buttons on phones). */}
          <div aria-hidden="true" className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_90%_45%_at_30%_38%,rgb(255_255_255/0.9),rgb(255_255_255/0.5)_50%,transparent_100%)] md:bg-[radial-gradient(ellipse_42%_60%_at_20%_45%,rgb(255_255_255/0.9),rgb(255_255_255/0.5)_50%,transparent_100%)]" />
          <div className="pointer-events-none relative ml-[clamp(20px,6vw,96px)] max-w-[640px]">
            <p className={eyebrow}>Open-source charting engine</p>
            <h1 className="text-[clamp(46px,7vw,104px)] leading-[0.95] tracking-[-0.05em]">
              Financial charts
              <br />
              <span className="glow">on the GPU.</span>
            </h1>
            <p className="mt-7 mb-9 max-w-[520px] text-[clamp(16px,1.4vw,19px)] text-pretty text-muted">
              One Rust core for candlesticks, indicators, drawings and order flow, on every platform.
            </p>
            <div className="pointer-events-auto flex flex-wrap items-center gap-3">
              <a
                className="group inline-flex h-[52px] items-center gap-2.5 rounded-full bg-ink pr-[22px] pl-[26px] text-[16.5px] font-medium text-white shadow-[0_8px_24px_-10px_rgb(13_15_20/0.6)] transition-colors duration-200 hover:bg-[#24252b]"
                href="#start"
              >
                Get started
                <span aria-hidden="true" className="inline-block transition-transform duration-[250ms] ease-out group-hover:translate-x-1">→</span>
              </a>
              <a
                className="inline-flex h-[52px] items-center rounded-full bg-[linear-gradient(90deg,#0f9d7a,#1a8fd0_50%,#6d4cf5)] p-[1.5px] shadow-[0_10px_28px_-14px_rgb(91_94_240/0.7)] transition-shadow duration-300 hover:shadow-[0_14px_34px_-12px_rgb(91_94_240/0.85)]"
                href="https://ferrochart.com/pro"
              >
                <span className="flex h-full items-center gap-2.5 rounded-full bg-white pr-2 pl-[22px] text-[16.5px] font-medium text-ink">
                  Open Pro terminal
                  <span className="flex h-[34px] items-center rounded-full bg-[#f1f0f9] px-3 text-[12.5px] font-medium text-[#4f46e5] max-sm:hidden">
                    K-line · tools · indicators ↗
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
            aria-label="Scroll to the examples"
          >
            <span className="scroll-wheel absolute top-[7px] left-1/2 -ml-[1.5px] h-[7px] w-[3px] rounded-sm bg-muted" />
          </a>
        </section>

        <section id="performance" aria-label="Measured performance" className="mx-auto max-w-[1200px] scroll-mt-24 px-[clamp(20px,6vw,40px)]">
          <div className="mb-7 flex flex-wrap items-baseline justify-between gap-x-5 gap-y-2">
            <h2 className="m-0 text-[clamp(26px,2.6vw,32px)] tracking-[-0.03em]">Built for tick-level data.</h2>
            <span className="text-[13px] text-faint">Apple M4 Max · Chrome + WebGPU</span>
          </div>
          <PerfStats stats={stats} />
          <p className="mt-3.5 text-xs text-faint">
            Browser figures: Google Chrome with WebGPU; the 10 million bar load is native Rust. Bars show each figure against
            its target: 300 ms for a load, no allocations, 1.5 MB for the package.
          </p>
        </section>

        <section id="library" className={`${pad} pt-[clamp(96px,12vw,160px)]`}>
          <p className={eyebrow}>Examples</p>
          <h2 className="text-[clamp(34px,4.6vw,64px)] leading-[1.02]">
            {allDemos.length} examples
            <br />
            across {categories.length} use cases.
          </h2>
          <p className="mt-6 max-w-[56ch] text-[clamp(16px,1.3vw,18px)] text-muted">
            Each example is a live chart rendered by FerroChart in your browser, with its source code alongside. All data
            is synthetic.
          </p>
          <Library initial={open && { cat: open.cat.id, demo: open.demo.id }} />
        </section>

        <section id="features" className={`${pad} scroll-mt-16 pt-[clamp(96px,12vw,160px)]`}>
          <p className={eyebrow}>Features</p>
          <h2 className="max-w-[18ch] text-[clamp(34px,4.6vw,64px)] leading-[1.02]">Designed for market data.</h2>
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
            <p className={eyebrow}>Get started</p>
            <h2 className="text-[clamp(34px,4.6vw,64px)] leading-[1.02]">One API in every language.</h2>
            <p className="mt-6 max-w-[44ch] text-[clamp(16px,1.3vw,18px)] text-muted">
              Every binding follows the same model: create a chart, add series, set data and add indicators. Data is passed
              as columns, from typed arrays, Rust slices or DataFrames.
            </p>
            <ul className="mt-8 grid gap-2.5 p-0 text-[15px]">
              {langs.map((l) => (
                <li key={l} className="flex list-none items-center gap-3">
                  <span aria-hidden="true" className="size-1.5 rounded-full bg-ink" />
                  {l}
                </li>
              ))}
            </ul>
            <a href="/docs" className="mt-8 inline-flex items-center gap-2 font-medium underline decoration-ink/25 underline-offset-4 hover:decoration-ink">
              Read the documentation <span aria-hidden="true">→</span>
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
                Every market.
                <br />
                Every screen.
                <br />
                <span className="bg-[linear-gradient(90deg,#1a8fd0,#6d4cf5_55%,#c04f9b)] bg-clip-text text-transparent">One engine.</span>
              </h2>
              <p className="mt-6 mb-8 max-w-[440px] text-[clamp(16px,1.3vw,17px)] text-on-chart-muted">
                Open source under Apache-2.0, from the browser to the trading desk.
              </p>
              <div className="flex flex-wrap gap-3">
                <a href="#start" className="group inline-flex h-12 items-center gap-2.5 rounded-full bg-white px-[22px] text-[15.5px] font-medium text-ink transition-colors hover:bg-[#e9e8ff]">
                  Get started <span aria-hidden="true" className="transition-transform duration-[250ms] group-hover:translate-x-1">→</span>
                </a>
                <a href="/docs" className="inline-flex h-12 items-center rounded-full border border-[#33343c] px-5 text-[15.5px] text-on-chart transition-colors hover:border-[#55565f] hover:text-white">
                  Read the docs
                </a>
              </div>
            </div>
            <div className="relative grid min-w-0 grid-cols-[1.3fr_1fr] grid-rows-[200px_150px] gap-3.5 max-sm:grid-cols-1 max-sm:grid-rows-[200px_140px_140px]">
              {showcase.map((x) => (
                <a
                  key={x.id}
                  href={x.href}
                  className={`group flex min-w-0 flex-col gap-2 rounded-[18px] border border-[#2a2b32] bg-[#1d1e24] p-3.5 transition-colors hover:border-[#3d3e48] ${x.wide ? 'sm:row-span-2' : ''}`}
                >
                  <span className="flex justify-between font-mono text-[11px] uppercase tracking-[0.06em] text-on-chart-muted">
                    {x.label}
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

      <SiteFooter />
    </>
  );
}
