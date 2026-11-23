import type { Metadata } from 'next';
import { DocChapter, docMeta, docParams } from '@/components/pages/DocsPages';

export const dynamicParams = false;
export const generateStaticParams = docParams;

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }): Promise<Metadata> {
  return docMeta('en', (await params).slug);
}

export default async function Page({ params }: { params: Promise<{ slug: string }> }) {
  return <DocChapter locale="en" slug={(await params).slug} />;
}
