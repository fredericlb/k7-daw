// knob.tsx — a 270 degree arc knob for a panel (36 to 44 px): drag up or down, arrow keys, Home and End.
// The arc runs clockwise from 7:30 to 4:30; a peak chip turns error-filled while the signal clips.

import * as React from "react";
import { cn } from "./lib/utils";

const START = 135;
const SWEEP = 270;

function polar(r: number, deg: number): [number, number] {
  const a = (deg * Math.PI) / 180;
  return [20 + r * Math.cos(a), 20 + r * Math.sin(a)];
}

function arc(from: number, to: number): string {
  const [x1, y1] = polar(16, START + from * SWEEP);
  const [x2, y2] = polar(16, START + to * SWEEP);
  const large = (to - from) * SWEEP > 180 ? 1 : 0;
  return `M ${x1} ${y1} A 16 16 0 ${large} 1 ${x2} ${y2}`;
}

export function Knob({
  value,
  min = 0,
  max = 1,
  step,
  onValueChange,
  label,
  size = 40,
  format,
  clipping,
  peak,
  className,
}: {
  value: number;
  min?: number;
  max?: number;
  /** Keyboard step; a hundredth of the range when omitted. */
  step?: number;
  onValueChange: (v: number) => void;
  /** The accessible name and the caption under the arc. */
  label: string;
  /** 36 to 44. */
  size?: number;
  /** The value as text ("-6.0 dB"); the raw number when omitted. */
  format?: (v: number) => string;
  /** The signal is clipping: the peak chip fills with the error colour. */
  clipping?: boolean;
  /** The peak chip's text; no chip without it. */
  peak?: string;
  className?: string;
}) {
  const range = max - min;
  const t = range === 0 ? 0 : Math.min(1, Math.max(0, (value - min) / range));
  const inc = step ?? range / 100;
  const set = (v: number) => onValueChange(Math.min(max, Math.max(min, v)));
  const drag = React.useRef<{ y: number; v: number } | null>(null);

  return (
    <div className={cn("flex flex-col items-center gap-1", className)}>
      <div
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={format?.(value)}
        className="cursor-ns-resize touch-none rounded-full outline-none focus-visible:shadow-(--k7-focus-ring)"
        style={{ width: size, height: size }}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          drag.current = { y: e.clientY, v: value };
        }}
        onPointerMove={(e) => {
          const d = drag.current;
          if (d) set(d.v + ((d.y - e.clientY) / 150) * range);
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        onKeyDown={(e) => {
          const k = e.key;
          if (k === "ArrowUp" || k === "ArrowRight") set(value + inc);
          else if (k === "ArrowDown" || k === "ArrowLeft") set(value - inc);
          else if (k === "Home") set(min);
          else if (k === "End") set(max);
          else return;
          e.preventDefault();
        }}
      >
        <svg viewBox="0 0 40 40" className="size-full" fill="none" aria-hidden>
          <path
            d={arc(0, 1)}
            stroke="rgba(255,255,255,0.16)"
            strokeWidth="4"
            strokeLinecap="round"
          />
          {t > 0 ? (
            <path
              d={arc(0, t)}
              stroke="var(--k7-accent-text)"
              strokeWidth="4"
              strokeLinecap="round"
            />
          ) : null}
        </svg>
      </div>
      <span className="type-mono-value text-fg-2">
        {format ? format(value) : value}
      </span>
      {peak ? (
        <span
          className={cn(
            "type-badge rounded-full px-1.5 py-0.5 tabular-nums",
            clipping ? "bg-error text-ink" : "bg-(--k7-row) text-fg-2",
          )}
        >
          {peak}
        </span>
      ) : null}
    </div>
  );
}
