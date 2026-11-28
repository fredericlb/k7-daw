// panZoom.ts — navigation for the media panes, which scale to FIT by default (so there is
// nothing to scroll until you zoom). useHZoom scales horizontally, anchored so the point under
// the cursor stays put — by trackpad pinch, by Cmd/Ctrl+wheel and by two fingers (useTwoFingers, which also pan); a bare wheel
// or a trackpad slide is the browser's scroll (wheelZooms), and one finger scrolls nothing. The arithmetic is Rust's (@studio/k7-wasm/gestureTicks);
// usePanScroll pans on a MIDDLE-mouse drag — not Space-drag, because Space is the transport
// toggle in every app that mounts these.
// Unit-free on purpose: the caller multiplies its own lane width, in beats or in seconds.

import {
  anchoredScrollLeft,
  isMouseWheel,
  isZoomWheel,
  pinchZoomTo,
  wheelFactor,
  wheelZoomTo,
  zoomCeiling,
} from "@studio/k7-wasm/gestureTicks";
import {
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useElementSize } from "./measure";
import { useTwoFingers } from "./twoFingers";

export { isMouseWheel, isZoomWheel, wheelFactor, zoomCeiling };

/** What one wheel event MEANS, as the whole rule in one testable place: only a pinch or Cmd/Ctrl zooms, a bare wheel scrolls — docs/panZoom.md. */
export const wheelZooms = isZoomWheel;

/** Horizontal zoom for a scroll container: 1 IS fit, `gutter` the non-scaling inset — docs/panZoom.md. */
export function useHZoom(
  ref: RefObject<HTMLElement | null>,
  // `gutter` may be told the pane's width, for a column that grows with it.
  opts: {
    gutter?: number | ((paneW: number) => number);
    min?: number;
    max?: number;
  } = {},
  // Values that gate the container's existence — a pane that renders "loading…" mounts its scroller only later.
  deps: unknown[] = [],
) {
  const { gutter: gutterOpt = 0, min = 1, max } = opts;
  const [zoom, setZoom] = useState(1);
  // The ceiling from the PANE, remeasured on resize: a narrower window can only zoom further.
  const paneW = useElementSize(ref, deps).w;
  const gutter = typeof gutterOpt === "function" ? gutterOpt(paneW) : gutterOpt;
  const ceiling = max ?? zoomCeiling(Math.max(0, paneW - gutter));
  const zoomRef = useRef(zoom);
  // The scrollLeft to apply AFTER the width change renders: computed on the old width, correct on the new.
  const pendingLeft = useRef<number | null>(null);

  useLayoutEffect(() => {
    zoomRef.current = zoom;
    const el = ref.current;
    if (el && pendingLeft.current != null) {
      el.scrollLeft = pendingLeft.current;
      pendingLeft.current = null;
    }
  }, [zoom, ref]);

  /** Zoom to `next`, keeping what is under `clientX` fixed — ONE implementation, both gestures; docs/panZoom.md. */
  const zoomAt = useCallback(
    (next: number, clientX: number) => {
      const el = ref.current;
      const cur = zoomRef.current;
      if (!el || next === cur) return;
      const offsetX = clientX - el.getBoundingClientRect().left;
      pendingLeft.current = anchoredScrollLeft(
        el.scrollLeft,
        offsetX,
        gutter,
        cur,
        next,
      );
      setZoom(next);
    },
    [ref, gutter],
  );

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      // A trackpad SLIDE is handed back to the browser, whose native scroll is the pan.
      if (!wheelZooms(e)) return;
      // preventDefault stops the page moving AND the browser zooming it on the same gesture.
      e.preventDefault();
      const cur = zoomRef.current;
      // Favour the dominant axis: Ctrl held on a horizontal wheel is still someone asking to zoom.
      const delta =
        Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
      if (delta === 0) return;
      // `e.ctrlKey` and not `isZoomWheel(e)`: only a ctrl-wheel is a pinch — see docs/panZoom.md.
      const next = wheelZoomTo(cur, delta, e.ctrlKey, min, ceiling);
      if (next === cur) return;
      zoomAt(next, e.clientX);
    };
    // Passive:false so preventDefault can stop the browser's page zoom on the pinch.
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ref, min, ceiling, zoomAt, ...deps]);

  // Two fingers: the start's zoom times the spread's ratio, anchored where they landed, and the scroll carried by their midpoint.
  useTwoFingers(
    ref,
    {
      onStart: () => {
        const el = ref.current;
        if (!el) return false;
        return {
          zoom: zoomRef.current,
          left: el.scrollLeft,
          top: el.scrollTop,
        };
      },
      onMove: (s, f) => {
        const el = ref.current;
        if (!el) return;
        const next =
          f.ratio === 1
            ? s.zoom
            : pinchZoomTo(s.zoom, f.ratio, 1, min, ceiling);
        const offsetX = f.origin.x - el.getBoundingClientRect().left;
        const left =
          anchoredScrollLeft(s.left, offsetX, gutter, s.zoom, next) - f.dx;
        el.scrollTop = s.top - f.dy;
        // A width still to render takes the scroll after it, as a zoom does; otherwise the scroll is set now.
        if (next === zoomRef.current && pendingLeft.current == null)
          el.scrollLeft = left;
        else {
          pendingLeft.current = left;
          setZoom(next);
        }
      },
    },
    deps,
  );

  return [zoom, setZoom, zoomAt, paneW] as const;
}

/** Drag-to-pan with the MIDDLE button, on whichever axis overflows — see docs/panZoom.md. */
export function usePanScroll(
  ref: RefObject<HTMLElement | null>,
  // See useHZoom's `deps` — re-bind once a late-mounting container exists.
  deps: unknown[] = [],
) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let panning = false;
    let startX = 0;
    let startY = 0;
    let startLeft = 0;
    let startTop = 0;

    const onMove = (e: PointerEvent) => {
      if (!panning) return;
      el.scrollLeft = startLeft - (e.clientX - startX);
      el.scrollTop = startTop - (e.clientY - startY);
    };
    const stop = () => {
      if (!panning) return;
      panning = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", stop);
      el.style.cursor = "";
    };
    const onPointerDown = (e: PointerEvent) => {
      if (e.button !== 1) return; // middle button only
      // Take the gesture: no autoscroll, and no child handler (the roll's note-draw) claiming the press first.
      e.preventDefault();
      e.stopPropagation();
      panning = true;
      startX = e.clientX;
      startY = e.clientY;
      startLeft = el.scrollLeft;
      startTop = el.scrollTop;
      el.style.cursor = "grabbing";
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", stop);
    };
    // The middle-click autoscroll puck is a mousedown default — cancel it there too.
    const onMouseDown = (e: MouseEvent) => {
      if (e.button === 1) e.preventDefault();
    };

    // Capture phase: a bubble listener loses to a child's stopPropagation and to Firefox's autoscroll.
    el.addEventListener("pointerdown", onPointerDown, { capture: true });
    el.addEventListener("mousedown", onMouseDown, { capture: true });
    return () => {
      el.removeEventListener("pointerdown", onPointerDown, { capture: true });
      el.removeEventListener("mousedown", onMouseDown, { capture: true });
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", stop);
      el.style.cursor = "";
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ref, ...deps]);
}
