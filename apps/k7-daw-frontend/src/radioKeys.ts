// radioKeys.ts — the arrow-key walk of a radiogroup built from `role="radio"` buttons: one Tab stop (the checked one).
// Selection follows focus, as the ARIA pattern has it.

import type React from "react";

/** The key handler of one radio: arrows and Home/End pick the neighbour and focus it. */
export function radioKeyDown<T>(
  e: React.KeyboardEvent<HTMLElement>,
  ids: readonly T[],
  current: T,
  pick: (id: T) => void,
): void {
  const at = ids.indexOf(current);
  const step =
    e.key === "ArrowRight" || e.key === "ArrowDown"
      ? 1
      : e.key === "ArrowLeft" || e.key === "ArrowUp"
        ? -1
        : 0;
  const to =
    e.key === "Home"
      ? 0
      : e.key === "End"
        ? ids.length - 1
        : step
          ? (at + step + ids.length) % ids.length
          : -1;
  if (to < 0 || ids.length === 0) return;
  e.preventDefault();
  pick(ids[to]);
  const radios =
    e.currentTarget.parentElement?.querySelectorAll<HTMLElement>(
      '[role="radio"]',
    );
  radios?.[to]?.focus();
}
