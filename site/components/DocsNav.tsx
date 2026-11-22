'use client';

import { usePathname } from 'next/navigation';
import type { DocGroup } from '@/lib/docs';

const href = (slug: string) => (slug ? `/docs/${slug}` : '/docs');

/** The chapter list: a sidebar on wide screens, a collapsible menu on narrow ones. */
export function DocsNav({ groups }: { groups: DocGroup[] }) {
  const path = usePathname().replace(/\/$/, '') || '/docs';
  const list = (
    <nav aria-label="Documentation" className="grid gap-7 text-[14px]">
      {groups.map((g) => (
        <div key={g.title}>
          <p className="mb-2 px-2.5 font-mono text-[11px] font-medium uppercase tracking-[0.14em] text-faint">{g.title}</p>
          <ul className="m-0 grid gap-0.5 p-0">
            {g.pages.map((p) => {
              const active = href(p.slug) === path;
              return (
                <li key={p.file} className="list-none">
                  <a
                    href={href(p.slug)}
                    aria-current={active ? 'page' : undefined}
                    className={`block rounded-lg px-2.5 py-1.5 transition ${active ? 'bg-ink/[0.06] font-medium text-ink' : 'text-muted hover:text-ink'}`}
                  >
                    {p.title}
                  </a>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
  return (
    <>
      <aside className="sticky top-16 hidden h-[calc(100vh-4rem)] overflow-y-auto py-10 pr-4 tab:block">{list}</aside>
      <details className="docs-menu mb-2 rounded-xl border border-line bg-white/70 tab:hidden">
        <summary className="cursor-pointer px-4 py-3 text-sm font-medium">Documentation menu</summary>
        <div className="border-t border-line px-4 py-5">{list}</div>
      </details>
    </>
  );
}
