import type { Metadata } from 'next';

/** The canonical origin of the site. */
export const SITE_URL = 'https://ferrochart.com';
export const SITE_NAME = 'FerroChart';
export const REPO_URL = 'https://github.com/0x0077/ferrochart';

/** The share image (1200×630, public/og.png). */
const OG_IMAGE = { url: '/og.png', width: 1200, height: 630, alt: 'FerroChart: GPU-rendered financial charts' };

/** A share image other than the default. */
export type OgImage = { url: string; width: number; height: number; alt: string };

/**
 * Metadata of one page: title, description, canonical URL and the Open
 * Graph / Twitter cards. Next.js replaces `openGraph` of the layout instead of
 * merging it, so every page sets all of it through here.
 */
export function pageMeta({
  title,
  description,
  path,
  type = 'website',
  image = OG_IMAGE,
}: {
  title: string;
  description: string;
  path: string;
  type?: 'website' | 'article';
  image?: OgImage;
}): Metadata {
  return {
    title,
    description,
    alternates: { canonical: path },
    openGraph: { type, url: path, siteName: SITE_NAME, title, description, images: [image], locale: 'en_US' },
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
