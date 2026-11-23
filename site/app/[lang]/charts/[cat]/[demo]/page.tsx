import type { Metadata } from 'next';
import { ChartPage, chartMeta, chartParams, type ChartParams } from '@/components/pages/ChartPage';
import type { Locale } from '@/lib/i18n';

type Params = ChartParams & { lang: string };

export const dynamicParams = false;
export const generateStaticParams = chartParams;

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const p = await params;
  return chartMeta(p.lang as Locale, p);
}

export default async function Page({ params }: { params: Promise<Params> }) {
  const p = await params;
  return <ChartPage locale={p.lang as Locale} params={p} />;
}
