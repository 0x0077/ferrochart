'use client';

import { useEffect, useRef, useState } from 'react';

/** The pre-rendered image of an example (public/thumbs), faded in on load. */
export function Thumb({ id, eager = false, onLoad }: { id: string; eager?: boolean; onLoad?: () => void }) {
  const [ready, setReady] = useState(false);
  const ref = useRef<HTMLImageElement>(null);
  const done = () => {
    setReady(true);
    onLoad?.();
  };
  // An image can finish loading before React attaches `onLoad` (cached,
  // or loaded while the page hydrates): check once mounted.
  useEffect(() => {
    const img = ref.current;
    if (img?.complete && img.naturalWidth > 0) done();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);
  return (
    <img
      ref={ref}
      alt=""
      src={`/thumbs/${id}.jpg`}
      loading={eager ? 'eager' : 'lazy'}
      decoding="async"
      onLoad={done}
      className={`block size-full object-cover transition-opacity duration-700 ease-soft ${ready ? 'ready opacity-100' : 'opacity-0'}`}
    />
  );
}
