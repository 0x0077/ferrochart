import type { Metadata } from 'next';
import { DocsIndex, docsIndexMeta } from '@/components/pages/DocsPages';
import type { Locale } from '@/lib/i18n';

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  return docsIndexMeta((await params).lang as Locale);
}

export default async function Page({ params }: { params: Promise<{ lang: string }> }) {
  return <DocsIndex locale={(await params).lang as Locale} />;
}
