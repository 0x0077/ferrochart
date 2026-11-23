import type { MetadataRoute } from 'next';
import { categories } from '@/catalog';
import { apiItems } from '@/lib/apiref';
import { docPages } from '@/lib/docs';
import { HTML_LANG, LOCALES, localePath } from '@/lib/i18n';
import { SITE_URL } from '@/lib/seo';

export const dynamic = 'force-static';

type Entry = MetadataRoute.Sitemap[number];
type Freq = Entry['changeFrequency'];

/**
 * Every indexable page: the homepage, the examples, the guide and Pro in
 * every language (each listing the others), and the API reference (English).
 */
export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date();
  const everywhere = (path: string, changeFrequency: Freq, priority: number): Entry[] => {
    const languages = Object.fromEntries(LOCALES.map((l) => [HTML_LANG[l], SITE_URL + localePath(l, path)]));
    return LOCALES.map((l) => ({
      url: SITE_URL + localePath(l, path),
      lastModified,
      changeFrequency,
      priority: l === 'en' ? priority : Math.round(priority * 8) / 10,
      alternates: { languages },
    }));
  };
  const docs = docPages().filter((p) => !p.file.startsWith('api:'));
  return [
    ...everywhere('/', 'weekly', 1),
    ...categories.flatMap((c) => c.demos.flatMap((d) => everywhere(`/charts/${c.id}/${d.id}`, 'monthly', 0.6))),
    ...docs.flatMap((p) => everywhere(p.slug ? `/docs/${p.slug}` : '/docs', 'weekly', p.slug ? 0.7 : 0.9)),
    { url: `${SITE_URL}/docs/api`, lastModified, changeFrequency: 'weekly', priority: 0.8 },
    ...apiItems().map((i) => ({ url: `${SITE_URL}/docs/api/${i.name}`, lastModified, changeFrequency: 'monthly' as const, priority: 0.4 })),
    ...everywhere('/pro', 'monthly', 0.6),
  ];
}
