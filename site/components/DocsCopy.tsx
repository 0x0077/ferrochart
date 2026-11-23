'use client';

import { useEffect } from 'react';

/** Adds a copy button to every code block of the article. */
export function DocsCopy({ copy, copied, label }: { copy: string; copied: string; label: string }) {
  useEffect(() => {
    for (const block of document.querySelectorAll<HTMLElement>('.doc .code')) {
      if (block.querySelector('.copy')) continue;
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'copy';
      b.textContent = copy;
      b.setAttribute('aria-label', label);
      b.onclick = () => {
        const text = block.querySelector('pre')?.innerText ?? '';
        navigator.clipboard?.writeText(text).then(() => {
          b.textContent = copied;
          setTimeout(() => (b.textContent = copy), 1400);
        });
      };
      block.append(b);
    }
  }, [copy, copied, label]);
  return null;
}
