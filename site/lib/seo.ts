import type { Metadata } from 'next';
import { HTML_LANG, LOCALES, OG_LOCALE, englishOnly, localePath, type Locale } from './i18n';

/** The canonical origin of the site. */
export const SITE_URL = 'https://ferrochart.com';
export const SITE_NAME = 'FerroChart';
/** Release shared by the website's structured data and text documentation. */
export const SITE_VERSION = '0.5.0';
export const REPO_URL = 'https://github.com/0x0077/ferrochart';

/** The share image (1200×630, public/og.png). */
const OG_IMAGE = { url: '/og.png', width: 1200, height: 630, alt: 'FerroChart: GPU-rendered financial charts' };

/** A share image other than the default. */
export type OgImage = { url: string; width: number; height: number; alt: string };

/**
 * Metadata of one page: title, description, canonical URL, the other
 * languages of the page (hreflang) and the Open Graph / Twitter cards.
 * `path` is the English path; the page's own address is that path in
 * `locale`. Next.js replaces `openGraph` of the layout instead of merging
 * it, so every page sets all of it through here.
 */
export function pageMeta({
  title,
  description,
  path,
  type = 'website',
  image = OG_IMAGE,
  locale = 'en',
}: {
  title: string;
  description: string;
  path: string;
  type?: 'website' | 'article';
  image?: OgImage;
  locale?: Locale;
}): Metadata {
  const url = localePath(locale, path);
  const languages = englishOnly(path)
    ? undefined
    : { ...Object.fromEntries(LOCALES.map((l) => [HTML_LANG[l], localePath(l, path)])), 'x-default': path };
  return {
    title,
    description,
    alternates: { canonical: url, languages },
    openGraph: {
      type,
      url,
      siteName: SITE_NAME,
      title,
      description,
      images: [image],
      locale: OG_LOCALE[locale],
      alternateLocale: LOCALES.filter((l) => l !== locale).map((l) => OG_LOCALE[l]),
    },
    twitter: { card: 'summary_large_image', title, description, images: [image.url] },
  };
}

/** A one-line description from Markdown: its first paragraph, as plain text, ≤ 160 characters. */
export function describe(markdown: string, fallback: string): string {
  const para = markdown
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .find((p) => p && !/^(#|```|\||>|-|\*|\d+\.|<)/.test(p));
  if (!para) return fallback;
  const text = para
    .replace(/!\[[^\]]*]\([^)]*\)/g, '')
    .replace(/\[([^\]]*)]\([^)]*\)/g, '$1')
    .replace(/[`*_]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  if (text.length <= 160) return text;
  const cut = text.slice(0, 157);
  return `${cut.slice(0, Math.max(cut.lastIndexOf(' '), 120))}…`;
}
