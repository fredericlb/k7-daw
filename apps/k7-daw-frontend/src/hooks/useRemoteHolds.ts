// useRemoteHolds.ts — the panes another device shows of this project: which ones this device cannot open, and the
// move away when a remote takes the one it is on (the remote is the screen the person just walked to).

import { showToast } from "@studio/project-shell/toast";
import { useCallback, useEffect, useMemo } from "react";
import { m } from "../paraglide/messages.js";
import type { Pane } from "../routes";
import { PANE_META, VIEWPORT_META } from "../shellMeta";
import { type Blockable, inUseBy, nextFree } from "../surfaces/paneInUse";
import { useRemoteScreens } from "./useRemoteScreens";

/** The switcher's order, which the move away from a taken pane follows. */
export const SWITCHER_ORDER: readonly Pane[] = [
  "timeline",
  "turntables",
  "chains",
  "editor",
];

export interface RemoteHolds {
  /** The device holding each control, where one does. */
  inUse: Partial<Record<Blockable, string>>;
  /** "In use on <device>" for a control a remote holds, else undefined; any pane id may be asked. */
  reasonOf: (control: string) => string | undefined;
}

export function useRemoteHolds(ids: {
  timeline: string | null;
  turntables: string | null;
  viewport: string | null;
}): RemoteHolds {
  const screens = useRemoteScreens();
  const { timeline, turntables, viewport } = ids;
  const inUse = useMemo(
    () => inUseBy(screens, { timeline, turntables, viewport }),
    [screens, timeline, turntables, viewport],
  );
  const reasonOf = useCallback(
    (control: string) => {
      const device = (inUse as Record<string, string | undefined>)[control];
      return device ? m["surfaces.inUseOn"]({ device }) : undefined;
    },
    [inUse],
  );
  return { inUse, reasonOf };
}

/** A remote opened the pane or window this device is on: move to the next free pane, or close the window, and say so. */
export function useRemoteTakeover({
  inUse,
  full,
  order,
  moveTo,
  viewportOpen,
  closeViewport,
}: {
  inUse: RemoteHolds["inUse"];
  full: Pane;
  /** The panes in the switcher's order, the ones this layout has. */
  order: readonly Pane[];
  moveTo: (p: Pane) => void;
  viewportOpen: boolean;
  closeViewport: () => void;
}): void {
  const device = (inUse as Record<string, string | undefined>)[full];
  const frameDevice = inUse.viewport;
  useEffect(() => {
    if (!device) return;
    const blocked = new Set<string>(
      Object.keys(inUse).filter((k) => (inUse as Record<string, unknown>)[k]),
    );
    const next = nextFree(order, full, blocked) as Pane | null;
    if (!next) return;
    moveTo(next);
    showToast({
      message: m["surfaces.openedOn"]({
        surface: PANE_META[full].label,
        device,
      }),
    });
  }, [device, full, order, inUse, moveTo]);
  useEffect(() => {
    if (!frameDevice || !viewportOpen) return;
    closeViewport();
    showToast({
      message: m["surfaces.openedOn"]({
        surface: VIEWPORT_META.label,
        device: frameDevice,
      }),
    });
  }, [frameDevice, viewportOpen, closeViewport]);
}
