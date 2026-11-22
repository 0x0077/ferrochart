import type { MetadataRoute } from 'next';

export const dynamic = 'force-static';

export default function sitemap(): MetadataRoute.Sitemap {
  return [
    { url: 'https://ferrochart.com/', changeFrequency: 'weekly', priority: 1 },
    { url: 'https://ferrochart.com/pro', changeFrequency: 'monthly', priority: 0.6 },
  ];
}
