// RollOverview.tsx — "WHOLE CLIP" for the piano roll: every note of the clip as a bar, and a lavender frame on the span the roll shows.
// The frame drags by scrolling the roll's own scroller; the zoom stays where the wheel left it.

import { type RefObject, useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";

interface Bar {
  key: number;
  time: number;
  duration: number;
}

export function RollOverview({
  notes,
  origin,
  beats,
  scroller,
  gutter,
  laneW,
}: {
  notes: readonly Bar[];
  origin: number;
  beats: number;
  scroller: RefObject<HTMLDivElement | null>;
  /** The key column's width: it scrolls with the canvas but is not part of the beats. */
  gutter: number;
  laneW: number;
}) {
  const [view, setView] = useState({ left: 0, width: 1 });
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const read = () =>
      setView({ left: el.scrollLeft, width: el.clientWidth - gutter });
    read();
    el.addEventListener("scroll", read, { passive: true });
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", read);
      ro.disconnect();
    };
  }, [scroller, gutter]);
  if (beats <= 0 || laneW <= 0) return null;
  const lo = notes.reduce((a, n) => Math.min(a, n.key), 127);
  const hi = notes.reduce((a, n) => Math.max(a, n.key), 0);
  const span = Math.max(1, hi - lo + 1);
  const frameL = Math.min(1, Math.max(0, view.left / laneW));
  const frameW = Math.min(1 - frameL, Math.max(0.02, view.width / laneW));
  const scrollTo = (clientX: number, box: DOMRect) => {
    const el = scroller.current;
    if (!el) return;
    el.scrollLeft = ((clientX - box.left) / box.width - frameW / 2) * laneW;
  };
  return (
    <div className="shrink-0 px-3 pt-2">
      <div className="mb-0.5 font-bold text-[11px] text-fg-3 uppercase tracking-wider">
        {m["clip.wholeClip"]()}
      </div>
      <div
        role="slider"
        tabIndex={0}
        aria-label={m["clip.wholeClip"]()}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(frameL * 100)}
        onKeyDown={(e) => {
          const el = scroller.current;
          if (!el) return;
          if (e.key === "ArrowLeft") el.scrollLeft -= view.width / 4;
          if (e.key === "ArrowRight") el.scrollLeft += view.width / 4;
        }}
        onPointerDown={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          scrollTo(e.clientX, box);
          const move = (ev: PointerEvent) => scrollTo(ev.clientX, box);
          const up = () => {
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", up);
          };
          window.addEventListener("pointermove", move);
          window.addEventListener("pointerup", up);
        }}
        className="relative h-14 cursor-pointer touch-none overflow-hidden rounded-xl bg-(--k7-row) focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
      >
        {notes.map((n) => (
          <span
            key={`${n.key}@${n.time}`}
            className="absolute h-1 rounded-full bg-lavender/70"
            style={{
              left: `${((n.time - origin) / beats) * 100}%`,
              width: `${Math.max(0.4, (n.duration / beats) * 100)}%`,
              top: `${4 + ((hi - n.key) / span) * 80}%`,
            }}
          />
        ))}
        <div
          className="pointer-events-none absolute inset-y-0 rounded-lg border-2 border-lavender bg-lavender/15"
          style={{ left: `${frameL * 100}%`, width: `${frameW * 100}%` }}
        />
      </div>
      <div className="mt-1 font-bold text-[11px] text-fg-3 uppercase tracking-wider">
        {m["clip.zoom"]()}
      </div>
    </div>
  );
}
