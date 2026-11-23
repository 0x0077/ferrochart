'use client';
// The language menu of the header: the same page in another language. The
// address is read when a language is picked, so it follows the viewer (which
// changes it without a navigation).
import { usePathname } from 'next/navigation';
import { LANGUAGE_NAME, LOCALES, englishOnly, localePath, splitPath, type Locale } from '@/lib/i18n';
import { useI18n } from './I18n';

/** `path` (a site path in any language) in `to`; English-only pages lead to the guide. */
function switchPath(path: string, to: Locale): string {
  const [, english] = splitPath(path.length > 1 ? path.replace(/\/$/, '') : path);
  if (to !== 'en' && englishOnly(english)) return localePath(to, '/docs');
  return localePath(to, english);
}

export function LanguageMenu() {
  const { locale, t } = useI18n();
  const pathname = usePathname() || '/';
  return (
    <details className="lang-menu relative">
      <summary
        aria-label={t.nav.language}
        title={t.nav.language}
        className="flex h-[34px] cursor-pointer list-none items-center gap-1.5 rounded-full px-2.5 text-[13.5px] text-muted transition-colors hover:bg-ink/[0.06] hover:text-ink [&::-webkit-details-marker]:hidden"
      >
        <svg viewBox="0 0 16 16" aria-hidden="true" className="size-[15px] fill-none stroke-current stroke-[1.3]">
          <circle cx="8" cy="8" r="6.5" />
          <path d="M1.5 8h13M8 1.5c-2 2.2-2 10.8 0 13M8 1.5c2 2.2 2 10.8 0 13" />
        </svg>
        <span className="hidden sm:inline">{LANGUAGE_NAME[locale]}</span>
      </summary>
      <ul className="absolute right-0 z-30 mt-2 min-w-[148px] rounded-2xl border border-line bg-white p-1.5 shadow-[0_20px_50px_-20px_rgb(13_15_20/0.35)]">
        {LOCALES.map((l) => (
          <li key={l} className="list-none">
            <a
              href={switchPath(pathname, l)}
              hrefLang={l === 'en' ? 'en' : l}
              lang={l}
              aria-current={l === locale ? 'true' : undefined}
              onClick={(e) => {
                e.preventDefault();
                location.href = switchPath(location.pathname, l) + location.search + location.hash;
              }}
              className="block rounded-xl px-3 py-2 text-[14px] text-ink/80 hover:bg-ink/[0.05] hover:text-ink aria-[current=true]:font-medium aria-[current=true]:text-ink"
            >
              {LANGUAGE_NAME[l]}
            </a>
          </li>
        ))}
      </ul>
    </details>
  );
}
