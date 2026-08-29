import { type RefObject, useEffect, useState } from "react";

export interface ElementSize {
  width: number;
  height: number;
}

/** Beobachtet die CSS-Pixelgröße eines Elements per `ResizeObserver`. */
export function useElementSize(ref: RefObject<HTMLElement | null>): ElementSize {
  const [size, setSize] = useState<ElementSize>({ width: 0, height: 0 });

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      setSize({ width, height });
    });
    observer.observe(element);
    setSize({ width: element.clientWidth, height: element.clientHeight });

    return () => observer.disconnect();
  }, [ref]);

  return size;
}
