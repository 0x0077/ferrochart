import type { Metadata, Viewport } from 'next';
import { SITE_NAME, SITE_URL, pageMeta } from '@/lib/seo';
import './globals.css';

const home = pageMeta({
  title: 'FerroChart — GPU-rendered financial charts for every platform',
  description:
    'Open-source financial charting engine in Rust. Millions of candlesticks at 60 fps with WebGPU, WebGL2, Metal, Vulkan and DirectX 12; 30 indicators, 29 drawing tools, order flow; for JavaScript, Rust, Python and C.',
  path: '/',
});

export const metadata: Metadata = {
  ...home,
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

export const viewport: Viewport = {
  colorScheme: 'light',
  themeColor: '#f8f7f4',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
