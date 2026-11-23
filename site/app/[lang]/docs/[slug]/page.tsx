import type { Metadata } from 'next';
import { DocChapter, docMeta, docParams } from '@/components/pages/DocsPages';
import type { Locale } from '@/lib/i18n';

type Params = { lang: string; slug: string };

export const dynamicParams = false;
export const generateStaticParams = docParams;

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const p = await params;
  return docMeta(p.lang as Locale, p.slug);
}

export default async function Page({ params }: { params: Promise<Params> }) {
  const p = await params;
  return <DocChapter locale={p.lang as Locale} slug={p.slug} />;
}
