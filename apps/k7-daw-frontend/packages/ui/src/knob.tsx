// knob.tsx — a 270 degree arc knob for a panel (36 to 44 px): drag up or down (Shift finer), arrow keys, Home and End.
// The arc runs clockwise from 7:30 to 4:30; a peak chip turns error-filled while the signal clips.

import { usePointerDrag } from "@studio/ui-timeline/drag";
import * as React from "react";
import { cn } from "./lib/utils";

const START = 135;
const SWEEP = 270;
/** Pixels of drag over the whole range; Shift is ten times finer. */
const TRAVEL = 150;
const FINE = 0.1;

function polar(r: number, deg: number, c: number): [number, number] {
  const a = (deg * Math.PI) / 180;
  return [c + r * Math.cos(a), c + r * Math.sin(a)];
}

/** An SVG arc of radius `r` from fraction `from` to `to` (0 to 1) of the knob's 270 degrees, about `c` (20 on the 40 box). */
export function arcPath(r: number, from: number, to: number, c = 20): string {
  const [x1, y1] = polar(r, START + from * SWEEP, c);
  const [x2, y2] = polar(r, START + to * SWEEP, c);
  const large = (to - from) * SWEEP > 180 ? 1 : 0;
  return `M ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2}`;
}

export function Knob({
  value,
  min = 0,
  max = 1,
  step,
  onValueChange,
  onValueCommit,
  defaultValue,
  label,
  size = 40,
  format,
  clipping,
  peak,
  snap,
  unit,
  pointer,
  fill,
  fillColor,
  valueClassName,
  title,
  caption,
  className,
}: {
  value: number;
  min?: number;
  max?: number;
  /** Keyboard step; a hundredth of the range when omitted. Shift divides it by ten. */
  step?: number;
  onValueChange: (v: number) => void;
  /** Fires once with the last value a gesture reached, on release (pointer up, key up); a cancelled drag commits nothing. */
  onValueCommit?: (v: number) => void;
  /** Where a double-click puts the knob back; no reset without it. */
  defaultValue?: number;
  /** The accessible name. */
  label: string;
  /** 36 to 44. */
  size?: number;
  /** The value as text ("-6.0 dB"); the raw number and `unit` when omitted. */
  format?: (v: number) => string;
  /** The signal is clipping: the peak chip fills with the error colour. */
  clipping?: boolean;
  /** The peak chip's text; no chip without it. */
  peak?: string;
  /** Round every value to a multiple of this. */
  snap?: number;
  /** Text after the raw value when there is no `format`. */
  unit?: string;
  /** Draw a tick from the hub to the arc's end: the value's pointer. */
  pointer?: boolean;
  /** An arc 0 to 1 in `fillColor` drawn INSTEAD of the value's own (a live level); read the value off the `pointer`. */
  fill?: number;
  fillColor?: string;
  /** Added to the value's text, e.g. a warning colour. */
  valueClassName?: string;
  title?: string;
  /** A visible name between the dial and its value. */
  caption?: string;
  className?: string;
}) {
  const range = max - min;
  const t = range === 0 ? 0 : Math.min(1, Math.max(0, (value - min) / range));
  const inc = step ?? range / 100;
  const pending = React.useRef<number | null>(null);
  const set = (v: number) => {
    const snapped = snap ? Math.round(v / snap) * snap : v;
    const c = Math.min(max, Math.max(min, snapped));
    pending.current = c;
    onValueChange(c);
  };
  const release = () => {
    const v = pending.current;
    pending.current = null;
    if (v !== null) onValueCommit?.(v);
  };
  const drag = React.useRef<{
    y: number;
    v: number;
    fine: boolean;
    from: number;
  } | null>(null);
  const press = usePointerDrag({
    onStart: (e) => {
      drag.current = { y: e.clientY, v: value, fine: e.shiftKey, from: value };
      return undefined;
    },
    onMove: (ev) => {
      const d = drag.current;
      if (!d) return;
      // Shift pressed or let go mid-drag: carry on from here at the other speed, with no jump.
      if (ev.shiftKey !== d.fine) {
        d.v = pending.current ?? d.v;
        d.y = ev.clientY;
        d.fine = ev.shiftKey;
      }
      set(d.v + ((d.y - ev.clientY) / TRAVEL) * range * (d.fine ? FINE : 1));
    },
    onEnd: () => {
      drag.current = null;
      release();
    },
    // A cancelled gesture shows the value it started from and commits nothing.
    onCancel: () => {
      const d = drag.current;
      drag.current = null;
      pending.current = null;
      if (d) onValueChange(d.from);
    },
  });
  const hub = polar(9, START + t * SWEEP, 20);
  const tip = polar(16, START + t * SWEEP, 20);

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
        title={title}
        {...press}
        onKeyUp={release}
        onDoubleClick={() => {
          if (defaultValue === undefined) return;
          set(defaultValue);
          release();
        }}
        onKeyDown={(e) => {
          const k = e.key;
          const by = e.shiftKey ? inc * FINE : inc;
          if (k === "ArrowUp" || k === "ArrowRight") set(value + by);
          else if (k === "ArrowDown" || k === "ArrowLeft") set(value - by);
          else if (k === "Home") set(min);
          else if (k === "End") set(max);
          else return;
          e.preventDefault();
        }}
      >
        <svg viewBox="0 0 40 40" className="size-full" fill="none" aria-hidden>
          <path
            d={arcPath(16, 0, 1)}
            stroke="rgba(255,255,255,0.16)"
            strokeWidth="4"
            strokeLinecap="round"
          />
          {fill === undefined && t > 0 ? (
            <path
              d={arcPath(16, 0, t)}
              stroke="var(--k7-accent-text)"
              strokeWidth="4"
              strokeLinecap="round"
            />
          ) : null}
          {fill !== undefined && fill > 0.001 ? (
            <path
              d={arcPath(16, 0, Math.min(1, fill))}
              stroke={fillColor}
              strokeWidth="4"
              strokeLinecap="round"
            />
          ) : null}
          {pointer ? (
            <line
              x1={hub[0]}
              y1={hub[1]}
              x2={tip[0]}
              y2={tip[1]}
              stroke="var(--color-foreground)"
              strokeWidth="2.5"
              strokeLinecap="round"
            />
          ) : null}
        </svg>
      </div>
      {caption ? (
        <span className="type-badge text-fg-3 uppercase">{caption}</span>
      ) : null}
      <span className={cn("type-mono-value text-fg-2", valueClassName)}>
        {format ? format(value) : `${value}${unit ?? ""}`}
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
