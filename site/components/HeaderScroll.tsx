'use client';

import { useEffect } from 'react';

/** Homepage sections the header links to, top to bottom. */
const SECTIONS = ['library', 'features', 'performance'];

/**
 * Header behaviour on the homepage: `data-scrolled` once the page has moved
 * (the bar becomes a floating pill), and `data-active` on the link of the
 * section in view. Renders nothing.
 */
export function HeaderScroll() {
  useEffect(() => {
    const header = document.getElementById('site-header');
    if (!header) return;
    const onScroll = () => header.toggleAttribute('data-scrolled', window.scrollY > 40);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });

    const links = new Map(SECTIONS.map((id) => [id, header.querySelector<HTMLAnchorElement>(`a[href="/#${id}"]`)]));
    const visible = new Set<string>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) (e.isIntersecting ? visible.add(e.target.id) : visible.delete(e.target.id));
        // The last section in view below the header's line wins.
        const active = [...SECTIONS].reverse().find((id) => visible.has(id));
        for (const [id, a] of links) a?.toggleAttribute('data-active', id === active);
      },
      { rootMargin: '-80px 0px -55% 0px' },
    );
    for (const id of SECTIONS) {
      const el = document.getElementById(id);
      if (el) io.observe(el);
    }
    return () => {
      window.removeEventListener('scroll', onScroll);
      io.disconnect();
    };
  }, []);
  return null;
}
