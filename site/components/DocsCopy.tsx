'use client';

import { useEffect } from 'react';

/** Adds a copy button to every code block of the article. */
export function DocsCopy() {
  useEffect(() => {
    for (const block of document.querySelectorAll<HTMLElement>('.doc .code')) {
      if (block.querySelector('.copy')) continue;
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'copy';
      b.textContent = 'Copy';
      b.setAttribute('aria-label', 'Copy the code');
      b.onclick = () => {
        const text = block.querySelector('pre')?.innerText ?? '';
        navigator.clipboard?.writeText(text).then(() => {
          b.textContent = 'Copied';
          setTimeout(() => (b.textContent = 'Copy'), 1400);
        });
      };
      block.append(b);
    }
  }, []);
  return null;
}
