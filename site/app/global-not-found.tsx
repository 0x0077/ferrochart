import type { Metadata } from 'next';
import { RootDocument } from '@/components/pages/Root';
import { SiteFooter } from '@/components/SiteFooter';
import { SiteHeader } from '@/components/SiteHeader';
import './globals.css';

// The 404 page of every address that matches no page (the site has a root
// layout per language, so there is no single layout to put it in).
// Next.js marks this page noindex itself.
export const metadata: Metadata = { title: 'Page not found — FerroChart' };

export default function NotFound() {
  return (
    <RootDocument locale="en">
      <SiteHeader />
      <main className="mx-auto flex min-h-[70vh] max-w-[720px] flex-col justify-center px-6 pt-24">
        <p className="font-mono text-xs font-medium tracking-[0.16em] text-muted uppercase">404</p>
        <h1 className="mt-4 text-[clamp(36px,5vw,56px)] leading-tight font-semibold tracking-[-0.03em]">This page does not exist.</h1>
        <p className="mt-5 text-[17px] text-muted">It may have moved. These are good places to start:</p>
        <ul className="mt-6 flex flex-wrap gap-3 text-[15px] font-medium">
          {[
            ['Home', '/'],
            ['Examples', '/#library'],
            ['Documentation', '/docs'],
            ['API reference', '/docs/api'],
          ].map(([label, href]) => (
            <li key={href} className="list-none">
              <a className="inline-flex rounded-full border border-line bg-white px-4 py-2 hover:border-ink/30" href={href}>
                {label}
              </a>
            </li>
          ))}
        </ul>
      </main>
      <SiteFooter />
    </RootDocument>
  );
}
