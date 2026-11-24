// lozenge.ts — where the selected tab sits, so one pill can slide under the tabs instead of each tab painting its own.
// A tab is "selected" when it matches `selector`; jsdom has no observers, so there the lozenge stays hidden.

import { type RefObject, useLayoutEffect, useState } from "react";

export interface LozengeBox {
  x: number;
  w: number;
  y: number;
  h: number;
}

const WATCHED = ["data-state", "aria-checked", "aria-selected", "aria-current"];

export function useLozenge(
  ref: RefObject<HTMLElement | null>,
  selector: string,
): LozengeBox | null {
  const [box, setBox] = useState<LozengeBox | null>(null);
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const measure = () => {
      const el = root.querySelector<HTMLElement>(selector);
      setBox(
        el
          ? {
              x: el.offsetLeft,
              w: el.offsetWidth,
              y: el.offsetTop,
              h: el.offsetHeight,
            }
          : null,
      );
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const resize = new ResizeObserver(measure);
    resize.observe(root);
    const mutate = new MutationObserver(measure);
    mutate.observe(root, {
      attributes: true,
      subtree: true,
      childList: true,
      attributeFilter: WATCHED,
    });
    return () => {
      resize.disconnect();
      mutate.disconnect();
    };
  }, [ref, selector]);
  return box;
}
