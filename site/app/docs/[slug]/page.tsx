import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocArticle } from '@/components/DocArticle';
import { docPages, renderDoc } from '@/lib/docs';

export const dynamicParams = false;

export function generateStaticParams() {
  return docPages().filter((p) => p.slug && !p.file.startsWith('api:')).map((p) => ({ slug: p.slug }));
}

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }): Promise<Metadata> {
  const { slug } = await params;
  const page = docPages().find((p) => p.slug === slug);
  return page ? { title: `${renderDoc(page.file).title || page.title} — FerroChart docs` } : {};
}

export default async function DocPageView({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const page = docPages().find((p) => p.slug === slug);
  if (!page) notFound();
  return <DocArticle page={page} />;
}
