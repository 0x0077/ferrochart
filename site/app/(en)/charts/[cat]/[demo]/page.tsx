import type { Metadata } from 'next';
import { ChartPage, chartMeta, chartParams, type ChartParams } from '@/components/pages/ChartPage';

export const dynamicParams = false;
export const generateStaticParams = chartParams;

export async function generateMetadata({ params }: { params: Promise<ChartParams> }): Promise<Metadata> {
  return chartMeta('en', await params);
}

export default async function Page({ params }: { params: Promise<ChartParams> }) {
  return <ChartPage locale="en" params={await params} />;
}
