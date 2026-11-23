// The user guide in one language: the layout (header, chapter list), the
// index (/docs) and the chapters (/docs/<name>).
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import type { ReactNode } from 'react';
import { DocArticle } from '@/components/DocArticle';
import { DocsNav } from '@/components/DocsNav';
import { Logo } from '@/components/Logo';
import { SiteHeader } from '@/components/SiteHeader';
import { docGroups, docPages, docSource, renderDoc } from '@/lib/docs';
import { fmt, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { describe, pageMeta } from '@/lib/seo';

export function DocsLayout({ locale, children }: { locale: Locale; children: ReactNode }) {
  const t = messages(locale).docs;
  return (
    <>
      <SiteHeader locale={locale} current="docs" />
      <div className="mx-auto grid max-w-[1440px] gap-x-[clamp(24px,3vw,56px)] px-[clamp(20px,4vw,48px)] pt-16 tab:grid-cols-[220px_minmax(0,1fr)]">
        <DocsNav groups={docGroups(locale)} label={t.nav} menu={t.menu} />
        <div className="min-w-0 py-10">{children}</div>
      </div>
      <footer className="mt-10 flex justify-between gap-4 border-t border-line px-[clamp(20px,4vw,48px)] py-7 text-[13px] text-faint">
        <span className="flex items-center gap-2"><Logo className="size-4" />© 2026 FerroChart contributors · Apache-2.0</span>
      </footer>
    </>
  );
}

export function docsIndexMeta(locale: Locale): Metadata {
  const t = messages(locale).docs;
  return pageMeta({ title: t.title, description: t.description, path: '/docs', locale });
}

export function DocsIndex({ locale }: { locale: Locale }) {
  const page = docPages(locale).find((p) => p.slug === '');
  return page ? <DocArticle page={page} locale={locale} /> : null;
}

export function docParams(): { slug: string }[] {
  return docPages().filter((p) => p.slug && !p.file.startsWith('api:')).map((p) => ({ slug: p.slug }));
}

export function docMeta(locale: Locale, slug: string): Metadata {
  const page = docPages(locale).find((p) => p.slug === slug);
  if (!page) return {};
  const t = messages(locale).docs;
  const title = renderDoc(page.file, locale).title || page.title;
  return pageMeta({
    title: fmt(t.pageTitle, { title }),
    description: describe(docSource(page.file, locale).replace(/^#[^\n]*\n/, ''), fmt(t.pageDescription, { title })),
    path: `/docs/${slug}`,
    type: 'article',
    locale,
  });
}

export function DocChapter({ locale, slug }: { locale: Locale; slug: string }) {
  const page = docPages(locale).find((p) => p.slug === slug);
  if (!page || page.file.startsWith('api:')) notFound();
  return <DocArticle page={page} locale={locale} />;
}
