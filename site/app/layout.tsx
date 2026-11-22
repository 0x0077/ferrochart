import type { Metadata, Viewport } from 'next';
import './globals.css';

const title = 'FerroChart — open-source financial charting engine';
const description =
  'FerroChart is an open-source financial charting engine written in Rust. It renders with WebGPU and WebGL2 in the browser and Metal, Vulkan and DirectX 12 natively, with bindings for JavaScript, Python and C.';

export const metadata: Metadata = {
  metadataBase: new URL('https://ferrochart.com'),
  title,
  description,
  alternates: { canonical: '/' },
  openGraph: { type: 'website', url: '/', siteName: 'FerroChart', title, description },
  twitter: { card: 'summary_large_image', title, description },
};

export const viewport: Viewport = {
  colorScheme: 'dark',
  themeColor: '#07080b',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
