import type { Metadata } from 'next';
import { DocArticle } from '@/components/DocArticle';
import { docPages } from '@/lib/docs';

export const metadata: Metadata = {
  title: 'Documentation — FerroChart',
  description: 'Guides and API reference for FerroChart: web, React and Vue, Rust, Python and C.',
};

export default function DocsHome() {
  const page = docPages().find((p) => p.slug === '');
  return page ? <DocArticle page={page} /> : null;
}
