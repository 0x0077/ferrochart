import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocArticle } from '@/components/DocArticle';
import { docPages, docSource, renderDoc } from '@/lib/docs';
import { describe, pageMeta } from '@/lib/seo';

export const dynamicParams = false;

export function generateStaticParams() {
  return docPages().filter((p) => p.slug && !p.file.startsWith('api:')).map((p) => ({ slug: p.slug }));
}

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }): Promise<Metadata> {
  const { slug } = await params;
  const page = docPages().find((p) => p.slug === slug);
  if (!page) return {};
  const title = renderDoc(page.file).title || page.title;
  return pageMeta({
    title: `${title} — FerroChart docs`,
    description: describe(docSource(page.file).replace(/^#[^\n]*\n/, ''), `${title}: FerroChart documentation.`),
    path: `/docs/${slug}`,
    type: 'article',
  });
}

export default async function DocPageView({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const page = docPages().find((p) => p.slug === slug);
  if (!page) notFound();
  return <DocArticle page={page} />;
}
