// The page of one example: the homepage with the viewer open on it, so every
// example has its own address, title and share card. The viewer keeps the
// address on the example shown (components/Viewer.tsx).
import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { categories } from '@/catalog';
import { Home } from '@/components/Home';
import { localCategories } from '@/lib/catalog-text';
import { fmt, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { JsonLd, pageJsonLd } from '@/lib/jsonld';
import { describe, pageMeta } from '@/lib/seo';

export type ChartParams = { cat: string; demo: string };

export function chartParams(): ChartParams[] {
  return categories.flatMap((c) => c.demos.map((d) => ({ cat: c.id, demo: d.id })));
}

function find(locale: Locale, { cat, demo }: ChartParams) {
  const c = localCategories(locale).find((x) => x.id === cat);
  const d = c?.demos.find((x) => x.id === demo);
  return c && d ? { cat: c, demo: d } : null;
}

export function chartMeta(locale: Locale, params: ChartParams): Metadata {
  const found = find(locale, params);
  if (!found) return {};
  const { cat, demo } = found;
  const t = messages(locale).chart;
  const catName = locale === 'en' ? cat.title.toLowerCase() : cat.title;
  return pageMeta({
    title: fmt(t.title, { title: demo.title, cat: cat.title }),
    description: describe(demo.about, fmt(t.description, { title: demo.title, cat: catName })),
    path: `/charts/${cat.id}/${demo.id}`,
    locale,
    // The example's pre-rendered image (public/thumbs, 1280×800).
    image: { url: `/thumbs/${demo.id}.jpg`, width: 1280, height: 800, alt: fmt(t.imageAlt, { title: demo.title }) },
  });
}

export function ChartPage({ locale, params }: { locale: Locale; params: ChartParams }) {
  const found = find(locale, params);
  if (!found) notFound();
  const { cat, demo } = found;
  return (
    <>
      <JsonLd data={pageJsonLd({
        type: 'WebPage',
        metadata: chartMeta(locale, params),
        locale,
        path: `/charts/${cat.id}/${demo.id}`,
        breadcrumbs: [
          { name: messages(locale).nav.home, path: '/' },
          // The category navigation opens its first example; there is no separate category page.
          { name: cat.title, path: `/charts/${cat.id}/${cat.demos[0].id}` },
          { name: demo.title, path: `/charts/${cat.id}/${demo.id}` },
        ],
      })} />
      <Home locale={locale} open={found} />
    </>
  );
}
