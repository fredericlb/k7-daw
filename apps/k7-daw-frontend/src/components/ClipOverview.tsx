// ClipOverview.tsx — "WHOLE CLIP": a 56 px envelope of the file with a lavender frame on the part the zoom shows.
// The frame drags: the zoom's bar width is untouched, only where it starts moves (the panel owns the window).

import { useRef } from "react";
import { m } from "../paraglide/messages.js";

/** `view` and `act` are seconds into the file; `onMove` receives the new START of the zoom window. */
export function ClipOverview({
  rms,
  seconds,
  view,
  act,
  onMove,
}: {
  rms: readonly number[];
  seconds: number;
  view: { start: number; end: number };
  act: { from: number; to: number } | null;
  onMove: (startSec: number) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  if (seconds <= 0 || rms.length === 0) return null;
  const peak = Math.max(1e-6, ...rms);
  const step = Math.max(1, Math.floor(rms.length / 240));
  const bars: number[] = [];
  for (let i = 0; i < rms.length; i += step) bars.push(rms[i] / peak);
  const pct = (sec: number) => `${(sec / seconds) * 100}%`;
  const grab = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = box.current;
    if (!el) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const w = el.getBoundingClientRect();
    const span = view.end - view.start;
    const move = (clientX: number) =>
      onMove(
        Math.max(
          0,
          Math.min(
            seconds - span,
            ((clientX - w.left) / w.width) * seconds - span / 2,
          ),
        ),
      );
    move(e.clientX);
    const h = (ev: PointerEvent) => move(ev.clientX);
    const up = () => {
      window.removeEventListener("pointermove", h);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", h);
    window.addEventListener("pointerup", up);
  };
  return (
    <div className="shrink-0">
      <div className="mb-0.5 font-bold text-[11px] text-fg-3 uppercase tracking-wider">
        {m["clip.wholeClip"]()}
      </div>
      <div
        ref={box}
        role="slider"
        tabIndex={0}
        aria-label={m["clip.wholeClip"]()}
        aria-valuemin={0}
        aria-valuemax={Math.round(seconds * 100) / 100}
        aria-valuenow={Math.round(view.start * 100) / 100}
        onPointerDown={grab}
        onKeyDown={(e) => {
          const d = (view.end - view.start) / 4;
          if (e.key === "ArrowLeft") onMove(Math.max(0, view.start - d));
          if (e.key === "ArrowRight")
            onMove(Math.min(seconds - (view.end - view.start), view.start + d));
        }}
        className="relative h-14 cursor-pointer touch-none overflow-hidden rounded-xl bg-(--k7-row) focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
      >
        <div className="absolute inset-0 flex items-center gap-px px-1">
          {bars.map((v, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: a fixed-length drawing, never reordered
            <span
              key={i}
              className="min-w-px flex-1 rounded-full bg-foreground/40"
              style={{ height: `${Math.max(4, v * 100)}%` }}
            />
          ))}
        </div>
        {act && (
          <>
            <span
              className="absolute inset-y-0 w-0.5 bg-lavender/70"
              style={{ left: pct(act.from) }}
            />
            <span
              className="absolute inset-y-0 w-0.5 bg-lavender/70"
              style={{ left: pct(act.to) }}
            />
          </>
        )}
        <div
          className="pointer-events-none absolute inset-y-0 rounded-lg border-2 border-lavender bg-lavender/15"
          style={{
            left: pct(view.start),
            width: `${((view.end - view.start) / seconds) * 100}%`,
          }}
        />
      </div>
      <div className="mt-1 font-bold text-[11px] text-fg-3 uppercase tracking-wider">
        {m["clip.zoom"]()}
      </div>
    </div>
  );
}
