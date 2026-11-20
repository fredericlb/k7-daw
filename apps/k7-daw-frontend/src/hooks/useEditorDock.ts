// useEditorDock.ts — the editor dock's own state: open, expanded, and the height the person dragged it to.
// The one thing a reader would get wrong: the dock is NOT one of the `panel` values — it coexists with
// a right panel or the chain, so `panel` is derived from it and from the right-hand one (docs/shell.md).

import { useEffect, useState } from "react";
import type { ShellLayout } from "../useIsMobile";

/** The normal heights the design gives each device, in px (theme.css: --k7-dock-h-pad, --k7-dock-h-desk). */
const NORMAL = { pad: 380, desk: 400, phone: 0 } as const;

// Kept for the session, so closing the dock and opening it again keeps the height it was dragged to.
let dragged: number | null = null;

export interface EditorDockState {
  open: boolean;
  setOpen: (open: boolean | ((cur: boolean) => boolean)) => void;
  expanded: boolean;
  setExpanded: (expanded: boolean | ((cur: boolean) => boolean)) => void;
  height: number;
  setHeight: (h: number) => void;
}

export function useEditorDock(layout: ShellLayout): EditorDockState {
  const [open, setOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [height, setH] = useState(dragged);
  // A dock that closes forgets it was expanded: it opens at its normal height.
  useEffect(() => {
    if (!open) setExpanded(false);
  }, [open]);
  return {
    open,
    setOpen,
    expanded,
    setExpanded,
    height: height ?? NORMAL[layout],
    setHeight: (h) => {
      dragged = h;
      setH(h);
    },
  };
}
