import type { ReactNode } from 'react';
import { DocsNav } from '@/components/DocsNav';
import { Logo } from '@/components/Logo';
import { SiteHeader } from '@/components/SiteHeader';
import { docGroups } from '@/lib/docs';

export default function DocsLayout({ children }: { children: ReactNode }) {
  return (
    <>
      <SiteHeader current="docs" />
      <div className="mx-auto grid max-w-[1440px] gap-x-[clamp(24px,3vw,56px)] px-[clamp(20px,4vw,48px)] pt-16 tab:grid-cols-[220px_minmax(0,1fr)]">
        <DocsNav groups={docGroups()} />
        <div className="min-w-0 py-10">{children}</div>
      </div>
      <footer className="mt-10 flex justify-between gap-4 border-t border-line px-[clamp(20px,4vw,48px)] py-7 text-[13px] text-faint">
        <span className="flex items-center gap-2"><Logo className="size-4" />© 2026 FerroChart contributors · Apache-2.0</span>
      </footer>
    </>
  );
}
