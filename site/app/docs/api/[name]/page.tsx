import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocArticle } from '@/components/DocArticle';
import { apiItems } from '@/lib/apiref';

export const dynamicParams = false;

export function generateStaticParams() {
  return apiItems().map((i) => ({ name: i.name }));
}

export async function generateMetadata({ params }: { params: Promise<{ name: string }> }): Promise<Metadata> {
  const { name } = await params;
  const item = apiItems().find((i) => i.name === name);
  return item ? { title: `${item.name} — FerroChart API`, description: item.summary.replace(/`/g, '') } : {};
}

export default async function ApiItemPage({ params }: { params: Promise<{ name: string }> }) {
  const { name } = await params;
  const item = apiItems().find((i) => i.name === name);
  if (!item) notFound();
  return <DocArticle page={{ slug: `api/${name}`, title: name, file: `api:${name}` }} />;
}
