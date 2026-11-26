// FloatingCommandBar.tsx — the command bar as a MODAL card: hidden until summoned (Ctrl/⌘+Shift+P, double-Shift,
// the bar's button), a scrim under it, focused. It goes away on Escape, the scrim or a run, never on blur. While a
// command waits on its answers the scrim lets pointers through, since a gesture in a pane may be the answer.
// See ../../docs/commandBar.md.
import { useModalFocus } from "@studio/ui/modal-focus";
import { forwardRef, useRef } from "react";
import { useWindowKey } from "../hooks/useWindowKey";
import { m } from "../paraglide/messages.js";
import { usePopin } from "../popins";
import {
  CommandBar,
  type CommandBarHandle,
  type CommandBarProps,
} from "./CommandBar";

/** Ctrl/⌘+Shift+P summons the bar — captured, so it works from inside the text editor too. */
export function useSummonKey(onSummon: () => void, enabled = true): void {
  useWindowKey(
    (e) => {
      if (!(e.ctrlKey || e.metaKey) || !e.shiftKey || e.altKey) return;
      if (e.key.toLowerCase() !== "p") return;
      e.preventDefault();
      e.stopPropagation();
      onSummon();
    },
    { enabled, capture: true },
  );
}

export const FloatingCommandBar = forwardRef<
  CommandBarHandle,
  Omit<CommandBarProps, "autoFocus" | "onClose" | "onFocusChange"> & {
    open: boolean;
    onDismiss: () => void;
    /** Whether a command is waiting on its answers: the scrim then lets a gesture through to the panes. */
    pending: boolean;
  }
>(function FloatingCommandBar(
  { open, onDismiss, pending, onRun, ...bar },
  ref,
) {
  const card = useRef<HTMLDivElement>(null);
  useModalFocus(card, { active: open });
  // On the Escape ladder, so Esc from a pane clicked to answer an argument closes the bar and not the panel behind it.
  usePopin(open, onDismiss);
  if (!open) return null;
  return (
    <div className="pointer-events-none fixed inset-0 z-(--k7-z-palette)">
      <button
        type="button"
        tabIndex={-1}
        aria-label={m["common.close"]()}
        onClick={onDismiss}
        className={`scrim absolute inset-0 cursor-default ${pending ? "pointer-events-none" : "pointer-events-auto"}`}
      />
      <div
        ref={card}
        role="dialog"
        aria-label={m["shell.commandBar"]()}
        className="panel-dialog pointer-events-auto absolute top-[calc(var(--k7-header-h)+8px)] left-1/2 flex w-[min(560px,calc(100%-16px))] -translate-x-1/2 flex-col rounded-[22px] px-3.5 py-1.5 max-[700px]:top-[62px]"
      >
        <CommandBar
          ref={ref}
          {...bar}
          onRun={(line) => {
            onRun(line);
            onDismiss();
          }}
          onClose={onDismiss}
          autoFocus
        />
      </div>
    </div>
  );
});
