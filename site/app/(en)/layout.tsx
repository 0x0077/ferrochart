import type { Metadata } from 'next';
import { RootDocument, rootMeta, rootViewport } from '@/components/pages/Root';
import '../globals.css';

// The English site (no path prefix). The other languages: app/[lang].
export const metadata: Metadata = rootMeta('en');
export const viewport = rootViewport;

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <RootDocument locale="en">{children}</RootDocument>;
}
