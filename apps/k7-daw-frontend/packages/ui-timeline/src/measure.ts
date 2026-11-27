// measure.ts — a scroller or a pane measured, once: the one ResizeObserver (plus the scroll listener,
// when the reading depends on the scroll) that every surface used to write out for itself.
// `useElementWatch` is the primitive, for a reading that must NOT re-render per event; the two hooks are
// state on top of it. See docs/measure.md.

import { type RefObject, useLayoutEffect, useRef, useState } from "react";

/** Call `read` now, on every resize of the element and, with `scroll`, on every scroll of it. */
export function useElementWatch(
  ref: RefObject<HTMLElement | null>,
  read: (el: HTMLElement) => void,
  opts: { scroll?: boolean } = {},
  // Values that gate the element's existence — a pane that renders "loading…" mounts its scroller only later.
  deps: unknown[] = [],
) {
  const latest = useRef(read);
  latest.current = read;
  const scroll = opts.scroll ?? false;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `deps` is the caller's own re-bind list.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const go = () => latest.current(el);
    go();
    const ro = new ResizeObserver(go);
    ro.observe(el);
    if (scroll) el.addEventListener("scroll", go, { passive: true });
    return () => {
      ro.disconnect();
      if (scroll) el.removeEventListener("scroll", go);
    };
  }, [ref, scroll, ...deps]);
}

/** The element's client width and height, 0 until it is there. */
export function useElementSize(
  ref: RefObject<HTMLElement | null>,
  deps: unknown[] = [],
): { w: number; h: number } {
  const [size, setSize] = useState({ w: 0, h: 0 });
  useElementWatch(
    ref,
    (el) =>
      setSize((s) =>
        s.w === el.clientWidth && s.h === el.clientHeight
          ? s
          : { w: el.clientWidth, h: el.clientHeight },
      ),
    {},
    deps,
  );
  return size;
}

const NO_REF: RefObject<HTMLElement | null> = { current: null };

/** A scroller's box: where it is scrolled to and how much of it shows. Re-renders on every scroll, so only for a reading that needs it. */
export function useScrollBox(
  ref: RefObject<HTMLElement | null> | undefined,
  deps: unknown[] = [],
): { left: number; width: number } {
  const [box, setBox] = useState({ left: 0, width: 0 });
  useElementWatch(
    ref ?? NO_REF,
    (el) =>
      setBox((b) =>
        b.left === el.scrollLeft && b.width === el.clientWidth
          ? b
          : { left: el.scrollLeft, width: el.clientWidth },
      ),
    { scroll: true },
    deps,
  );
  return box;
}
