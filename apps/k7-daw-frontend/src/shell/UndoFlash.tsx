// UndoFlash.tsx — the undo or redo glyph at the centre of the screen, half its width and height, glowing, with each
// step's message under it, stacked while the same key is pressed again. It takes no pointer and fades on its own.
// What it shows is undoFlashStore.ts's; the reason it exists is in docs/shell.md ("Undo and redo answer at the centre").

import { CircleMinus, Redo2, Undo2 } from "@studio/ui/icons";
import { useSyncExternalStore } from "react";
import { flashSnapshot, subscribeFlash } from "./undoFlashStore";

export function UndoFlash() {
  const f = useSyncExternalStore(subscribeFlash, flashSnapshot, () => null);
  if (!f) return null;
  const Icon =
    f.kind === "undo" ? Undo2 : f.kind === "redo" ? Redo2 : CircleMinus;
  // Nothing to undo is the glyph alone: the sentence is still announced, never drawn.
  const silent = f.kind === "nothing";
  return (
    <div className="pointer-events-none fixed inset-0 z-(--k7-z-menu) flex flex-col items-center justify-center gap-4">
      <Icon
        key={f.seq}
        aria-hidden
        className="h-[50vh] w-[50vw] animate-[k7-undo-flash_1400ms_ease-out_forwards] text-lavender drop-shadow-[0_0_24px_var(--color-lavender)]"
        strokeWidth={1.5}
      />
      {/* The sentences are announced, as the toast they replace was: the glyph is only seen. */}
      <div
        key={`m${f.seq}`}
        role="status"
        aria-live="polite"
        className={
          silent
            ? "sr-only"
            : "flex max-w-[80vw] animate-[k7-undo-flash_1400ms_ease-out_forwards] flex-col items-center gap-1"
        }
      >
        {f.messages.map((msg, i) => (
          <p
            // biome-ignore lint/suspicious/noArrayIndexKey: two steps may say the same words; their place is the identity.
            key={i}
            className="type-body rounded-2xl bg-(--k7-row) px-3 py-1 text-center text-foreground shadow-lg"
          >
            {msg}
          </p>
        ))}
      </div>
    </div>
  );
}
