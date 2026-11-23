// The catalog in another language: the same categories and demos (code, API,
// ids) with the translated text of i18n/catalog/<locale>.json.
import type { Category } from './types';

/** Translated text of the catalog (i18n/catalog/<locale>.json); missing entries stay English. */
export type CatalogText = {
  categories?: Record<string, { title?: string; blurb?: string }>;
  groups?: Record<string, string>;
  demos?: Record<string, { title?: string; about?: string }>;
};

export function localize(categories: Category[], text: CatalogText | null): Category[] {
  if (!text) return categories;
  return categories.map((c) => ({
    ...c,
    title: text.categories?.[c.id]?.title || c.title,
    blurb: text.categories?.[c.id]?.blurb || c.blurb,
    demos: c.demos.map((d) => ({
      ...d,
      title: text.demos?.[d.id]?.title || d.title,
      about: text.demos?.[d.id]?.about || d.about,
      group: d.group && (text.groups?.[d.group] || d.group),
    })),
  }));
}
