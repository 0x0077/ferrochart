import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocArticle } from '@/components/DocArticle';
import { apiItems } from '@/lib/apiref';
import { pageMeta } from '@/lib/seo';

export const dynamicParams = false;

export function generateStaticParams() {
  return apiItems().map((i) => ({ name: i.name }));
}

export async function generateMetadata({ params }: { params: Promise<{ name: string }> }): Promise<Metadata> {
  const { name } = await params;
  const item = apiItems().find((i) => i.name === name);
  if (!item) return {};
  return pageMeta({
    title: `${item.name} — FerroChart API`,
    description: `${item.name} (${item.kind}) in @ferrochart/core, the FerroChart JavaScript API.${item.summary ? ` ${item.summary.replace(/`/g, '')}` : ''}`,
    path: `/docs/api/${name}`,
    type: 'article',
  });
}

export default async function ApiItemPage({ params }: { params: Promise<{ name: string }> }) {
  const { name } = await params;
  const item = apiItems().find((i) => i.name === name);
  if (!item) notFound();
  return <DocArticle locale="en" page={{ slug: `api/${name}`, title: name, file: `api:${name}`, href: `/docs/api/${name}` }} />;
}
