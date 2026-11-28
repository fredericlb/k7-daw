// AutomationPoints.tsx — an automation row's body: the curve as a polyline, each point a handle to drag, and a press on the empty row adds one.
// The one thing a reader would get wrong: a point's value is 0 to 1 top to bottom, and every edit is a `curve point` verb, never local state.

import { cn } from "@studio/ui/lib/utils";
import { holdToArm } from "@studio/ui-timeline/hold";
import { useState } from "react";
import { m } from "../../paraglide/messages.js";

export interface AutoPoint {
  /** The point's moment in beats, and its timecode as the file says it. */
  b: number;
  tc: string;
  v: number;
}

const PAD = 3;
const MOVE_PX = 3;

/** Where a value sits in a row `h` tall: 1 at the top. */
const yOf = (v: number, h: number) => (1 - v) * (h - 2 * PAD) + PAD;
const valueAt = (y: number, h: number) =>
  Math.min(1, Math.max(0, 1 - (y - PAD) / (h - 2 * PAD)));

export function AutomationPoints({
  curve,
  param,
  points,
  rowH,
  x,
  beatAt,
  tcAt,
  readOnly,
  run,
  onMenu,
}: {
  curve: string;
  param: string;
  points: readonly AutoPoint[];
  rowH: number;
  /** A beat's x in the row, and the beat under a pointer x (px from the row's left edge). */
  x: (beat: number) => number;
  beatAt: (clientX: number) => number;
  /** The timecode of the tick nearest a beat, which is what the verbs take. */
  tcAt: (beat: number) => string;
  readOnly: boolean;
  run: (line: string) => void;
  onMenu: (tc: string, x: number, y: number) => void;
}) {
  const [drag, setDrag] = useState<{
    i: number;
    b: number;
    v: number;
    moved: boolean;
  } | null>(null);
  const shown = points.map((p, i) =>
    drag && drag.i === i ? { ...p, b: drag.b, v: drag.v } : p,
  );
  const line = (p: { b: number; v: number }) => `${x(p.b)},${yOf(p.v, rowH)}`;
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: the points are the keyboard's way in; this surface adds one by pointer
    <div
      className="absolute inset-0"
      onClick={(e) => {
        if (readOnly || e.target !== e.currentTarget) return;
        const r = e.currentTarget.getBoundingClientRect();
        const tc = tcAt(beatAt(e.clientX - r.left));
        run(
          `curve point add ${curve} ${tc} ${valueAt(e.clientY - r.top, rowH).toFixed(3)}`,
        );
      }}
    >
      <svg
        className="pointer-events-none absolute inset-0 h-full w-full"
        aria-hidden="true"
      >
        <title>{param}</title>
        {shown.length === 1 ? (
          <line
            x1={0}
            x2="100%"
            y1={yOf(shown[0].v, rowH)}
            y2={yOf(shown[0].v, rowH)}
            stroke="var(--k7-curve)"
            strokeWidth={1.5}
            vectorEffect="non-scaling-stroke"
          />
        ) : shown.length > 1 ? (
          // `vectorEffect` so the stroke keeps its width whatever the viewBox does — the x axis here is an extreme aspect.
          <polyline
            fill="none"
            stroke="var(--k7-curve)"
            strokeWidth={1.5}
            vectorEffect="non-scaling-stroke"
            points={shown.map(line).join(" ")}
          />
        ) : null}
      </svg>
      {shown.map((p, i) => (
        <button
          key={`${points[i].tc}-${i}`}
          type="button"
          aria-label={m["tl.autoPoint"]({
            tc: points[i].tc,
            value: points[i].v.toFixed(2),
          })}
          title={m["tl.autoPointHint"]()}
          className={cn(
            "absolute z-10 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-ground bg-(--k7-curve) after:absolute after:-inset-3.5 after:content-[''] focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none",
            !readOnly && "cursor-grab",
            drag?.i === i && drag.moved && "cursor-grabbing",
          )}
          style={{ left: x(p.b), top: yOf(p.v, rowH) }}
          onPointerDown={(e) => {
            e.stopPropagation();
            if (readOnly || e.button !== 0) return;
            const el = e.currentTarget;
            const pointerId = e.pointerId;
            // A finger holds first, so a stray swipe moves no point (@studio/ui-timeline/hold).
            holdToArm(
              e,
              {
                arm: () => {
                  el.setPointerCapture(pointerId);
                  setDrag({ i, b: points[i].b, v: points[i].v, moved: false });
                },
              },
              el,
            );
          }}
          onPointerMove={(e) => {
            if (!drag || drag.i !== i) return;
            const row = e.currentTarget.parentElement?.getBoundingClientRect();
            if (!row) return;
            const b = Math.max(0, beatAt(e.clientX - row.left));
            setDrag({
              i,
              b,
              v: valueAt(e.clientY - row.top, rowH),
              moved:
                drag.moved ||
                Math.abs(x(b) - x(points[i].b)) > MOVE_PX ||
                Math.abs(yOf(drag.v, rowH) - yOf(points[i].v, rowH)) > MOVE_PX,
            });
          }}
          onPointerUp={() => {
            const d = drag;
            setDrag(null);
            if (!d || d.i !== i || !d.moved) return;
            run(
              `curve point move ${curve} ${points[i].tc} ${tcAt(d.b)} ${d.v.toFixed(3)}`,
            );
          }}
          onPointerCancel={() => setDrag(null)}
          onClick={(e) => e.stopPropagation()}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onMenu(points[i].tc, e.clientX, e.clientY);
          }}
          onKeyDown={(e) => {
            if (readOnly) return;
            const p0 = points[i];
            if (e.key === "Delete" || e.key === "Backspace") {
              e.preventDefault();
              run(`curve point remove ${curve} ${p0.tc}`);
            } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
              e.preventDefault();
              const v = Math.min(
                1,
                Math.max(0, p0.v + (e.key === "ArrowUp" ? 0.05 : -0.05)),
              );
              run(
                `curve point move ${curve} ${p0.tc} ${p0.tc} ${v.toFixed(3)}`,
              );
            }
          }}
        />
      ))}
    </div>
  );
}
