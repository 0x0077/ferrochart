import type { Metadata } from 'next';
import { DocArticle } from '@/components/DocArticle';
import { pageMeta } from '@/lib/seo';
import { JsonLd, docJsonLd } from '@/lib/jsonld';

export const metadata: Metadata = pageMeta({
  title: 'JavaScript API reference — FerroChart docs',
  description: 'Every class, function and type of @ferrochart/core, the FerroChart npm package.',
  path: '/docs/api',
});

export default function ApiIndex() {
  const page = { slug: 'api', title: 'JavaScript API reference', file: 'api:', href: '/docs/api' };
  return (
    <>
      <JsonLd data={docJsonLd(metadata, 'en', page.href, page.title)} />
      <DocArticle locale="en" page={page} />
    </>
  );
}
