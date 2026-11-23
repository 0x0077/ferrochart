import { localePath, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { GitHubStar } from './GitHubStar';
import { HeaderScroll } from './HeaderScroll';
import { LanguageMenu } from './LanguageMenu';
import { Logo } from './Logo';

/**
 * The fixed header of every page: logo, sections, docs, the language menu
 * and the GitHub star.
 * `floating` (the homepage) turns it into a pill once the page scrolls.
 */
export function SiteHeader({ locale = 'en', current, floating = false }: { locale?: Locale; current?: 'docs'; floating?: boolean }) {
  const t = messages(locale).nav;
  const p = (path: string) => localePath(locale, path);
  const link = (href: string, label: string, active = false) => (
    <a
      className="rounded-full px-[13px] py-[7px] transition-colors duration-200 hover:bg-ink/[0.06] hover:text-ink"
      href={href}
      aria-current={active ? 'page' : undefined}
    >
      {label}
    </a>
  );
  return (
    <header id="site-header" className="site-header fixed inset-x-0 top-0 z-20 flex justify-center">
      <div className="site-header-bar flex w-full items-center justify-between gap-6">
        <a className="flex items-center gap-2.5 font-semibold tracking-tight" href={p('/')} aria-label={t.home}>
          <Logo className="size-[22px]" />
          FerroChart
        </a>
        <div className="flex items-center gap-[clamp(12px,2vw,20px)]">
          <nav className="site-nav hidden gap-1 text-[14.5px] text-muted md:flex">
            {link(p('/#library'), t.examples)}
            {link(p('/#features'), t.features)}
            {link(p('/#performance'), t.performance)}
            {link(p('/docs'), t.docs, current === 'docs')}
            {link('https://ferrochart.com/pro', t.pro)}
          </nav>
          <LanguageMenu />
          <GitHubStar label={t.star} labelCount={t.starCount} />
        </div>
      </div>
      {floating && <HeaderScroll />}
    </header>
  );
}
