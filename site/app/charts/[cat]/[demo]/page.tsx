// The page of one example: the homepage with the viewer open on it, so every
// example has its own address, title and share card. The viewer keeps the
// address on the example shown (components/Viewer.tsx).
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { categories } from '@/catalog';
import { Home } from '@/components/Home';
import { describe, pageMeta } from '@/lib/seo';

type Params = { cat: string; demo: string };

export const dynamicParams = false;

export function generateStaticParams(): Params[] {
  return categories.flatMap((c) => c.demos.map((d) => ({ cat: c.id, demo: d.id })));
}

function find({ cat, demo }: Params) {
  const c = categories.find((x) => x.id === cat);
  const d = c?.demos.find((x) => x.id === demo);
  return c && d ? { cat: c, demo: d } : null;
}

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const found = find(await params);
  if (!found) return {};
  const { cat, demo } = found;
  return pageMeta({
    title: `${demo.title} · ${cat.title} chart example — FerroChart`,
    description: describe(demo.about, `${demo.title}: a live ${cat.title.toLowerCase()} chart rendered by FerroChart, with its source code.`),
    path: `/charts/${cat.id}/${demo.id}`,
    // The example's pre-rendered image (public/thumbs, 1280×800).
    image: { url: `/thumbs/${demo.id}.jpg`, width: 1280, height: 800, alt: `${demo.title}, rendered by FerroChart` },
  });
}

export default async function ChartPage({ params }: { params: Promise<Params> }) {
  const found = find(await params);
  if (!found) notFound();
  return <Home open={found} />;
}
