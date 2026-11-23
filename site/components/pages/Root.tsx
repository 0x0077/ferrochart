// The root of every page in one language: <html lang>, the page metadata
// shared by all pages and the interface text for client components.
import type { Metadata, Viewport } from 'next';
import type { ReactNode } from 'react';
import { I18nProvider } from '@/components/I18n';
import { HTML_LANG, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { SITE_NAME, SITE_URL, pageMeta } from '@/lib/seo';

export function rootMeta(locale: Locale): Metadata {
  const t = messages(locale).home;
  return {
    ...pageMeta({ title: t.title, description: t.description, path: '/', locale }),
    metadataBase: new URL(SITE_URL),
    applicationName: SITE_NAME,
    keywords: [
      'financial charts',
      'candlestick chart',
      'charting library',
      'trading chart',
      'WebGPU',
      'WebAssembly',
      'Rust',
      'technical indicators',
      'order flow',
      'footprint chart',
      'lightweight-charts alternative',
    ],
    authors: [{ name: 'FerroChart contributors' }],
    creator: 'FerroChart contributors',
    category: 'technology',
    formatDetection: { telephone: false, email: false, address: false },
    robots: { index: true, follow: true },
  };
}

export const rootViewport: Viewport = {
  colorScheme: 'light',
  themeColor: '#f8f7f4',
};

export function RootDocument({ locale, children }: { locale: Locale; children: ReactNode }) {
  return (
    <html lang={HTML_LANG[locale]}>
      <body>
        <I18nProvider locale={locale} t={messages(locale)}>
          {children}
        </I18nProvider>
      </body>
    </html>
  );
}
