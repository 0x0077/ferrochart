import type { MetadataRoute } from 'next';

export const dynamic = 'force-static';

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: 'FerroChart',
    short_name: 'FerroChart',
    description: 'GPU-rendered financial charts for every platform.',
    start_url: '/',
    display: 'browser',
    background_color: '#f8f7f4',
    theme_color: '#f8f7f4',
    icons: [
      { src: '/icon.svg', type: 'image/svg+xml', sizes: 'any' },
      { src: '/icon.png', type: 'image/png', sizes: '512x512' },
      { src: '/apple-icon.png', type: 'image/png', sizes: '180x180' },
    ],
  };
}
