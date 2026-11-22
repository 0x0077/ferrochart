// Motion for the viewer: the panel growing out of the clicked card (and
// shrinking back into it). Web Animations only, so every effect can be
// finished at once when the user skips it. The chart itself is drawn by the
// engine (`playIntro`, lib/stage.js). Browser only.

const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/** `inset()` that clips `el` (laid out, untransformed) to the screen rect `r`. */
function insetFor(el, r, radius) {
  const R = el.getBoundingClientRect();
  return `inset(${r.top - R.top}px ${R.right - r.right}px ${R.bottom - r.bottom}px ${r.left - R.left}px round ${radius}px)`;
}

const FULL = (radius) => `inset(0px 0px 0px 0px round ${radius}px)`;

/**
 * Container transform: `panel` grows out of `rect` (the card on screen) to
 * its own box. Resolves to the Animation (already playing).
 */
export function grow(panel, rect, { duration = 680, radius = 26 } = {}) {
  return panel.animate(
    [{ clipPath: insetFor(panel, rect, 16), opacity: 0.9 }, { clipPath: FULL(radius), opacity: 1 }],
    { duration, easing: 'cubic-bezier(.75,0,.15,1)' },
  );
}

/** The reverse of `grow`: `panel` shrinks into `rect`. Keeps the end state. */
export function shrink(panel, rect, { duration = 540, delay = 100, radius = 26 } = {}) {
  return panel.animate(
    [{ clipPath: FULL(radius) }, { clipPath: insetFor(panel, rect, 16) }],
    { duration, delay, easing: 'cubic-bezier(.75,0,.2,1)', fill: 'forwards' },
  );
}

export { reducedMotion };
