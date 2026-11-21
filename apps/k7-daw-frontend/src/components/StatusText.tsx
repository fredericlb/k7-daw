// StatusText.tsx — the status line's text, the one reader of statusLine.ts's store, so a new
// line re-renders this span and nothing above it.

import { TriangleAlert } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import type { StatusLineStore } from "../statusLine";

export function StatusText({
  store,
  fallback,
}: {
  store: StatusLineStore;
  /** Shown when nobody has anything to say — the project's root. */
  fallback?: string;
}) {
  const { text, refused, hint } = useSyncExternalStore(
    store.subscribe,
    store.get,
  );
  // The open menu speaks FIRST and borrows the line rather than writing it, so closing gives back what the app last said.
  const shown = hint || text || fallback;
  // A refusal is told apart by a glyph and a spoken prefix, not by colour alone.
  const refusal = !hint && refused && !!text;
  return (
    <span
      // This line is the shell's whole error channel, so it is also its only live region; a refusal interrupts.
      role={refusal ? "alert" : "status"}
      data-testid="project-status-line"
      aria-live={refusal ? "assertive" : "polite"}
      // Truncation is the layout's, not the message's: the full text has to stay reachable.
      title={shown}
      className={cn(
        "truncate",
        // Only when the app's OWN last word was a refusal: a menu hint borrows this line and is nobody's verdict.
        refusal && "text-warn",
      )}
    >
      {refusal && (
        <>
          <TriangleAlert
            aria-hidden
            className="mr-1 inline size-3.5 align-[-2px]"
          />
          <span className="sr-only">{m["status.cant"]()} </span>
        </>
      )}
      {shown}
    </span>
  );
}
