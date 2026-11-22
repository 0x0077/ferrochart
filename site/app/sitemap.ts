import type { MetadataRoute } from 'next';
import { apiItems } from '@/lib/apiref';
import { docPages, hrefOf } from '@/lib/docs';
import { SITE_URL } from '@/lib/seo';

export const dynamic = 'force-static';

/** Every indexable page: the homepage, the guide, the API reference and Pro. */
export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date();
  const docs = docPages().filter((p) => !p.file.startsWith('api:'));
  return [
    { url: `${SITE_URL}/`, lastModified, changeFrequency: 'weekly', priority: 1 },
    ...docs.map((p) => ({
      url: SITE_URL + hrefOf(p.slug),
      lastModified,
      changeFrequency: 'weekly' as const,
      priority: p.slug ? 0.7 : 0.9,
    })),
    { url: `${SITE_URL}/docs/api`, lastModified, changeFrequency: 'weekly', priority: 0.8 },
    ...apiItems().map((i) => ({ url: `${SITE_URL}/docs/api/${i.name}`, lastModified, changeFrequency: 'monthly' as const, priority: 0.4 })),
    { url: `${SITE_URL}/pro`, lastModified, changeFrequency: 'monthly', priority: 0.6 },
  ];
}
