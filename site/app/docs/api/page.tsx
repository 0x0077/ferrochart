import type { Metadata } from 'next';
import { DocArticle } from '@/components/DocArticle';

export const metadata: Metadata = {
  title: 'JavaScript API reference — FerroChart docs',
  description: 'Every class, function and type of @ferrochart/core.',
};

export default function ApiIndex() {
  return <DocArticle page={{ slug: 'api', title: 'JavaScript API reference', file: 'api:' }} />;
}
