// undoFlashStore.ts — what the centred undo/redo flash shows: its kind and the steps' messages, stacked while the person
// keeps pressing the same one. A redo after undos (or the reverse) starts a new stack; the flash fades on its own.
// Framework-free; UndoFlash.tsx reads it with useSyncExternalStore (docs/shell.md).

/** How long the flash stays after the last step. */
export const FLASH_MS = 1400;

export interface Flash {
  kind: "undo" | "redo";
  messages: readonly string[];
  /** Bumped on each step, so the fade restarts. */
  seq: number;
}

let current: Flash | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
const listeners = new Set<() => void>();
const emit = () => {
  for (const l of listeners) l();
};

/** One undo or redo landed (or found nothing to do): stack its message onto a flash of the same kind, or start one. */
export function flashStep(kind: Flash["kind"], message: string): void {
  const same = current && current.kind === kind;
  current = {
    kind,
    messages: same && current ? [...current.messages, message] : [message],
    seq: (current?.seq ?? 0) + 1,
  };
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    current = null;
    timer = null;
    emit();
  }, FLASH_MS);
  emit();
}

export function subscribeFlash(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

export const flashSnapshot = (): Flash | null => current;
