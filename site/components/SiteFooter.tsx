import { localePath, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { site } from '@/lib/site';
import { Logo } from './Logo';

/** The homepage footer: the product, link columns and the licence line. */
export function SiteFooter({ locale = 'en' }: { locale?: Locale }) {
  const m = messages(locale);
  const t = m.footer;
  const p = (path: string) => localePath(locale, path);
  // Every link goes to a page that exists.
  const COLUMNS: { title: string; links: [string, string][] }[] = [
    { title: t.product, links: [[m.nav.examples, p('/#library')], [m.nav.features, p('/#features')], [m.nav.performance, p('/#performance')], ['Pro', 'https://ferrochart.com/pro']] },
    { title: t.docs, links: [[t.gettingStarted, p('/docs/web')], [t.guides, p('/docs')], [t.apiReference, '/docs/api'], [t.building, p('/docs/building')]] },
    { title: t.resources, links: [['GitHub', `https://github.com/${site.githubRepo}`], [t.rendering, p('/docs/rendering')], [t.performanceGuide, p('/docs/performance')], [t.migrate, p('/docs/migrate')]] },
  ];
  return (
    <footer className="mt-[clamp(64px,7vw,104px)] border-t border-line">
      <div className="grid grid-cols-2 gap-10 px-[clamp(20px,6vw,96px)] pt-16 pb-8 md:grid-cols-[minmax(200px,1.3fr)_repeat(3,minmax(0,1fr))]">
        <div className="col-span-full flex flex-col gap-3.5 md:col-span-1">
          <span className="flex items-center gap-2.5 text-base font-semibold">
            <Logo className="size-[18px]" />
            FerroChart
          </span>
          <p className="m-0 max-w-[240px] text-sm leading-[1.55] text-muted">{t.tagline}</p>
        </div>
        {COLUMNS.map((col) => (
          <nav key={col.title} aria-label={col.title} className="flex flex-col gap-[11px]">
            <p className="m-0 mb-1 font-mono text-[11.5px] uppercase tracking-[0.14em] text-faint">{col.title}</p>
            {col.links.map(([label, href]) => (
              <a
                key={label}
                href={href}
                {...(href.startsWith('http') ? { target: '_blank', rel: 'noopener' } : {})}
                className="text-[14.5px] text-ink/80 transition-colors hover:text-ink"
              >
                {label}
              </a>
            ))}
          </nav>
        ))}
      </div>
      <div className="mx-[clamp(20px,6vw,96px)] flex flex-wrap justify-between gap-3 border-t border-line/70 pt-[22px] pb-9 text-[13px] text-faint">
        <span>© 2026 FerroChart contributors · Apache-2.0</span>
        <span>{t.synthetic}</span>
      </div>
    </footer>
  );
}
