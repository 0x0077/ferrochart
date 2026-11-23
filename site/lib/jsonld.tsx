import type { Metadata } from 'next';
import { HTML_LANG, localePath, type Locale } from './i18n';
import { messages } from './messages';
import { SITE_URL } from './seo';

export const SOFTWARE_ID = `${SITE_URL}/#software`;
export const WEBSITE_ID = `${SITE_URL}/#website`;

/** JSON-LD is data, but its script container still needs HTML-safe serialization. */
export function JsonLd({ data }: { data: unknown }) {
  return <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(data).replace(/</g, '\\u003c') }} />;
}

type Crumb = { name: string; path: string };

export function pageJsonLd({ type, metadata, locale, path, breadcrumbs }: {
  type: 'WebPage' | 'TechArticle';
  metadata: Metadata;
  locale: Locale;
  path: string;
  breadcrumbs: Crumb[];
}) {
  const absolute = (path: string) => new URL(localePath(locale, path), SITE_URL).href;
  return [
    {
      '@context': 'https://schema.org',
      '@type': 'BreadcrumbList',
      itemListElement: breadcrumbs.map((crumb, i) => ({
        '@type': 'ListItem',
        position: i + 1,
        name: crumb.name,
        item: absolute(crumb.path),
      })),
    },
    {
      '@context': 'https://schema.org',
      '@type': type,
      [type === 'WebPage' ? 'name' : 'headline']: metadata.title,
      description: metadata.description,
      inLanguage: HTML_LANG[locale],
      url: absolute(path),
      isPartOf: { '@id': WEBSITE_ID },
      ...(type === 'TechArticle' ? { about: { '@id': SOFTWARE_ID } } : {}),
    },
  ];
}

/** Index pages end at their own level rather than repeating the same breadcrumb. */
export function docJsonLd(metadata: Metadata, locale: Locale, path: string, name: string) {
  const t = messages(locale);
  const breadcrumbs: Crumb[] = [
    { name: t.nav.home, path: '/' },
    { name: t.nav.docs, path: '/docs' },
  ];
  if (path.startsWith('/docs/api')) breadcrumbs.push({ name: t.docs.jsApi, path: '/docs/api' });
  if (path !== '/docs' && path !== '/docs/api') breadcrumbs.push({ name, path });
  return pageJsonLd({ type: 'TechArticle', metadata, locale, path, breadcrumbs });
}
