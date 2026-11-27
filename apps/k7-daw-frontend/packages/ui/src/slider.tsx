// slider.tsx — a native range with the page's one commit rule: a gesture ends on pointer up or cancel, key up, or blur.
// It stays a native input so the keyboard and the accessible name come with it; `onValueCommit` fires once per change.

import * as React from "react";
import { rangeGesture } from "./lib/rangeGesture";
import { cn } from "./lib/utils";

export interface SliderProps
  extends Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    "type" | "value" | "onChange" | "min" | "max" | "step"
  > {
  /** The accessible name. */
  label?: string;
  value: number;
  min: number;
  max: number;
  step: number;
  /** The value as said aloud ("-6.0 dB"); the screen shows it elsewhere. */
  format?: (v: number) => string;
  onValueChange: (v: number) => void;
  /** The gesture's end, with the value it left. */
  onValueCommit?: (v: number) => void;
}

export function Slider({
  label,
  value,
  min,
  max,
  step,
  format,
  onValueChange,
  onValueCommit,
  className,
  onPointerUp,
  onPointerCancel,
  onKeyUp,
  onBlur,
  ...rest
}: SliderProps) {
  const commitRef = React.useRef(onValueCommit);
  commitRef.current = onValueCommit;
  const gesture = React.useRef<ReturnType<typeof rangeGesture> | null>(null);
  gesture.current ??= rangeGesture((v) => commitRef.current?.(v));
  const end = () => gesture.current?.end();
  return (
    <input
      type="range"
      aria-label={label}
      aria-valuetext={format?.(value)}
      min={min}
      max={max}
      step={step}
      value={value}
      className={cn(
        "h-5 cursor-pointer accent-(--k7-accent-text) disabled:cursor-default",
        className,
      )}
      onChange={(e) => {
        const v = Number(e.currentTarget.value);
        gesture.current?.change(v);
        onValueChange(v);
      }}
      onPointerUp={(e) => {
        onPointerUp?.(e);
        end();
      }}
      onPointerCancel={(e) => {
        onPointerCancel?.(e);
        end();
      }}
      onKeyUp={(e) => {
        onKeyUp?.(e);
        end();
      }}
      onBlur={(e) => {
        onBlur?.(e);
        end();
      }}
      {...rest}
    />
  );
}
