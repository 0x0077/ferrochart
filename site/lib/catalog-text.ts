// Server only: the translated catalog text of a locale, read at build time.
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { categories } from '@/catalog';
import { localize, type CatalogText } from '@/catalog/localize';
import type { Locale } from './i18n';

type Entry = { title?: string; about?: string; blurb?: string; src?: string };
type File = { categories?: Record<string, Entry>; groups?: Record<string, string>; demos?: Record<string, Entry> };

function read(locale: Locale): File | null {
  if (locale === 'en') return null;
  const file = path.join(process.cwd(), 'i18n', 'catalog', `${locale}.json`);
  return existsSync(file) ? (JSON.parse(readFileSync(file, 'utf8')) as File) : null;
}

/**
 * What a page needs at first: category titles and blurbs, group names and
 * example titles, and the description of `open` (the page's example). The
 * other descriptions come from /i18n/<locale>.json (`catalogAbouts`).
 */
export function catalogText(locale: Locale, open?: string): CatalogText | null {
  const t = read(locale);
  if (!t) return null;
  const pick = (o: Record<string, Entry> | undefined, f: (id: string, e: Entry) => Entry) =>
    o && Object.fromEntries(Object.entries(o).map(([id, e]) => [id, f(id, e)]));
  return {
    categories: pick(t.categories, (_, e) => ({ title: e.title, blurb: e.blurb })),
    groups: t.groups,
    demos: pick(t.demos, (id, e) => (id === open ? { title: e.title, about: e.about } : { title: e.title })),
  };
}

/** Every example description in `locale`, by id. */
export function catalogAbouts(locale: Locale): Record<string, string> {
  const t = read(locale);
  return Object.fromEntries(Object.entries(t?.demos ?? {}).flatMap(([id, e]) => (e.about ? [[id, e.about]] : [])));
}

/** The catalog in `locale`, all of it. */
export function localCategories(locale: Locale) {
  const t = read(locale);
  return localize(categories, t && { categories: t.categories, groups: t.groups, demos: t.demos });
}
