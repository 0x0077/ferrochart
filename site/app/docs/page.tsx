import type { Metadata } from 'next';
import { DocArticle } from '@/components/DocArticle';
import { docPages } from '@/lib/docs';
import { pageMeta } from '@/lib/seo';

export const metadata: Metadata = pageMeta({
  title: 'Documentation — FerroChart',
  description: 'Guides and API reference for FerroChart: web, React and Vue, Rust, Python and C.',
  path: '/docs',
});

export default function DocsHome() {
  const page = docPages().find((p) => p.slug === '');
  return page ? <DocArticle page={page} /> : null;
}
