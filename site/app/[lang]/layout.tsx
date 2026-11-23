import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { RootDocument, rootMeta, rootViewport } from '@/components/pages/Root';
import { PREFIXED, isLocale, type Locale } from '@/lib/i18n';
import '../globals.css';

// Every language but English, under its prefix (/zh, /zh-tw, /ja, /ko).
export const dynamicParams = false;

export function generateStaticParams() {
  return PREFIXED.map((lang) => ({ lang }));
}

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params;
  return isLocale(lang) ? rootMeta(lang) : {};
}

export const viewport = rootViewport;

export default async function RootLayout({ children, params }: { children: React.ReactNode; params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  if (!isLocale(lang)) notFound();
  return <RootDocument locale={lang as Locale}>{children}</RootDocument>;
}
