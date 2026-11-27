// slider.tsx — a native range with the page's one commit rule: a gesture ends on pointer up or cancel, key up, or blur.
// It stays a native input so the keyboard and the accessible name come with it; `onValueCommit` fires once per change.

import * as React from "react";
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
  const dirty = React.useRef(false);
  const commit = (el: HTMLInputElement) => {
    if (!dirty.current) return;
    dirty.current = false;
    onValueCommit?.(Number(el.value));
  };
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
        dirty.current = true;
        onValueChange(Number(e.currentTarget.value));
      }}
      onPointerUp={(e) => {
        onPointerUp?.(e);
        commit(e.currentTarget);
      }}
      onPointerCancel={(e) => {
        onPointerCancel?.(e);
        commit(e.currentTarget);
      }}
      onKeyUp={(e) => {
        onKeyUp?.(e);
        commit(e.currentTarget);
      }}
      onBlur={(e) => {
        onBlur?.(e);
        commit(e.currentTarget);
      }}
      {...rest}
    />
  );
}
