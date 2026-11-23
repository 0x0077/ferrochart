import { DocsCopy } from './DocsCopy';
import { docContent, docPages, type DocPage } from '@/lib/docs';
import type { Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';

/** One chapter: the article, its outline and the previous and next chapters. */
export function DocArticle({ page, locale }: { page: DocPage; locale: Locale }) {
  const t = messages(locale).docs;
  const content = docContent(page, locale);
  if (!content) return null;
  const { html, headings } = content;
  const fallback = 'fallback' in content && content.fallback;
  // API items outside the sidebar (interfaces, types) have no previous or next page.
  const pages = docPages(locale);
  const i = pages.findIndex((p) => p.file === page.file);
  const prev = i >= 0 ? pages[i - 1] : undefined;
  const next = i >= 0 ? pages[i + 1] : undefined;
  return (
    <div className="grid gap-x-12 desk:grid-cols-[minmax(0,1fr)_200px]">
      <div className="min-w-0">
        {fallback && t.untranslated && (
          <p className="mb-6 rounded-xl border border-line bg-white px-4 py-3 text-sm text-muted">{t.untranslated}</p>
        )}
        <article className="doc" lang={fallback ? 'en' : undefined} dangerouslySetInnerHTML={{ __html: html }} />
        <DocsCopy copy={t.copy} copied={t.copied} label={t.copyAria} />
        <nav aria-label={t.chapters} className="mt-16 grid gap-3 border-t border-line pt-8 sm:grid-cols-2">
          {prev ? (
            <a href={prev.href} className="group rounded-xl border border-line bg-white px-5 py-4 transition hover:border-ink/20">
              <span className="block text-xs text-faint">{t.previous}</span>
              <span className="font-medium group-hover:text-ink">{prev.title}</span>
            </a>
          ) : <span />}
          {next && (
            <a href={next.href} className="group rounded-xl border border-line bg-white px-5 py-4 text-right transition hover:border-ink/20">
              <span className="block text-xs text-faint">{t.next}</span>
              <span className="font-medium group-hover:text-ink">{next.title}</span>
            </a>
          )}
        </nav>
      </div>
      {headings.length > 1 && (
        <aside className="sticky top-26 hidden max-h-[calc(100vh-8rem)] self-start overflow-y-auto desk:block">
          <p className="mb-3 font-mono text-[11px] font-medium uppercase tracking-[0.14em] text-faint">{t.onThisPage}</p>
          <ul className="m-0 grid gap-1.5 border-l border-line p-0 text-[13px]">
            {headings.map((h) => (
              <li key={h.id} className="list-none">
                <a href={`#${h.id}`} className={`-ml-px block border-l border-transparent text-muted hover:border-ink hover:text-ink ${h.depth === 3 ? 'pl-6' : 'pl-3'}`}>
                  {h.text}
                </a>
              </li>
            ))}
          </ul>
        </aside>
      )}
    </div>
  );
}
