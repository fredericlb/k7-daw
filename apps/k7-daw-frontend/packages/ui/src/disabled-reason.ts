// disabled-reason.ts — a control that is disabled for a stated reason stays pressable and says why.
// The host registers how the reason is shown (a toast); the kit cannot import the shell's toast itself.

import type * as React from "react";

type Say = (reason: string) => void;

let say: Say | null = null;

/** The host's way of showing a reason on press; returns the unregister. */
export function setDisabledReasonSink(fn: Say): () => void {
  say = fn;
  return () => {
    if (say === fn) say = null;
  };
}

/** The reason a control is disabled, when it is disabled and says why (`disabledReason`). */
export function reasonOf(
  disabled: boolean | undefined,
  disabledReason: string | undefined,
): string | undefined {
  return disabled && disabledReason ? disabledReason : undefined;
}

/** Shows a reason through the host's sink. */
export function showReason(reason: string): void {
  say?.(reason);
}

/** A pressable "disabled" button: no native `disabled` (it takes no press nor focus), a press shows the reason. */
export function reasonPress<E extends React.SyntheticEvent>(
  reason: string,
): (e: E) => void {
  return (e) => {
    e.preventDefault();
    e.stopPropagation();
    showReason(reason);
  };
}
