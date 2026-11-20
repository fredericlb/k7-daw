// useEditorDock.ts — the editor dock's own state: open, expanded, and the height the person dragged it to.
// The one thing a reader would get wrong: the dock is NOT one of the `panel` values — it coexists with
// a right panel, so `panel` is derived from it and from the right-hand one; one occupant at a time (docs/shell.md).

import { useCallback, useEffect, useState } from "react";
import type { ShellLayout } from "../useIsMobile";

/** What the dock holds: the editors, the Mixer or a track's chain — one at a time. */
export type DockOccupant = "events" | "mixer" | "chain";

/** The normal heights the design gives each device, in px (theme.css: --k7-dock-h-pad, --k7-dock-h-desk). */
const NORMAL = { pad: 380, desk: 400, phone: 0 } as const;

// Kept for the session, so closing the dock and opening it again keeps the height it was dragged to.
let dragged: number | null = null;

export interface EditorDockState {
  open: boolean;
  occupant: DockOccupant;
  /** Open the dock on `occupant`, or switch it there; the height and the expanded state are kept. */
  show: (occupant: DockOccupant) => void;
  setOpen: (open: boolean | ((cur: boolean) => boolean)) => void;
  expanded: boolean;
  setExpanded: (expanded: boolean | ((cur: boolean) => boolean)) => void;
  height: number;
  setHeight: (h: number) => void;
}

export function useEditorDock(layout: ShellLayout): EditorDockState {
  const [open, setOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [occupant, setOccupant] = useState<DockOccupant>("events");
  const [height, setH] = useState(dragged);
  const show = useCallback((o: DockOccupant) => {
    setOccupant(o);
    setOpen(true);
  }, []);
  // A dock that closes forgets it was expanded: it opens at its normal height.
  useEffect(() => {
    if (!open) setExpanded(false);
  }, [open]);
  return {
    open,
    occupant,
    show,
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
