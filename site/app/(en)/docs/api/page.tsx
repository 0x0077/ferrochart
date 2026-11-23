import type { Metadata } from 'next';
import { DocArticle } from '@/components/DocArticle';
import { pageMeta } from '@/lib/seo';

export const metadata: Metadata = pageMeta({
  title: 'JavaScript API reference — FerroChart docs',
  description: 'Every class, function and type of @ferrochart/core, the FerroChart npm package.',
  path: '/docs/api',
});

export default function ApiIndex() {
  return <DocArticle locale="en" page={{ slug: 'api', title: 'JavaScript API reference', file: 'api:', href: '/docs/api' }} />;
}
