'use client';

import { useEffect, useRef } from 'react';
import { registerHighlightBox } from '@/lib/gridHighlight';

export function useGridHighlight<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    // Register the element
    const cleanup = registerHighlightBox(element);

    return cleanup;
  }, []);

  return ref;
}
