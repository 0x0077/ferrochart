// Renders one example (?demo=<id>) full-page, for the card images in
// public/thumbs/ (tests/web/site-thumbs.mjs). Not linked from the site.
import type { Metadata } from 'next';
import { Suspense } from 'react';
import { ThumbChart } from '@/components/ThumbChart';

export const metadata: Metadata = { robots: { index: false, follow: false } };

export default function Thumb() {
  return (
    <Suspense>
      <ThumbChart />
    </Suspense>
  );
}
