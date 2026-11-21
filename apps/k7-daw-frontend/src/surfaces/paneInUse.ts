// paneInUse.ts — which of THIS device's panes a remote shows, so this device cannot open it, as pure rules.
// A surface is a pane here by its declared id: timeline and turntables are panes, the picture is a window.
// What a reader gets wrong: a mastering surface has no pane on this device, so nothing of it is ever blocked.

/** The three controls of the main device a remote's surface can be. */
export type Blockable = "timeline" | "turntables" | "viewport";

export interface RemoteScreen {
  surface: string;
  device: string;
  since: number;
}

/** The id of the surface each control opens on this device, null where the project declares none. */
export type MainSurfaceIds = Record<Blockable, string | null>;

/** The device holding each control the remotes show; where two show one surface, the one that came first. */
export function inUseBy(
  screens: RemoteScreen[],
  ids: MainSurfaceIds,
): Partial<Record<Blockable, string>> {
  const out: Partial<Record<Blockable, string>> = {};
  const since: Partial<Record<Blockable, number>> = {};
  for (const s of screens) {
    for (const control of Object.keys(ids) as Blockable[]) {
      if (ids[control] !== s.surface) continue;
      if (since[control] === undefined || s.since < (since[control] ?? 0)) {
        out[control] = s.device;
        since[control] = s.since;
      }
    }
  }
  return out;
}

/** The next pane after `from` in `order`, wrapping, that is not blocked; null when every other one is. */
export function nextFree(
  order: readonly string[],
  from: string,
  blocked: ReadonlySet<string>,
): string | null {
  const at = order.indexOf(from);
  for (let i = 1; i <= order.length; i++) {
    const id = order[(at + i + order.length) % order.length];
    if (id !== from && !blocked.has(id)) return id;
  }
  return null;
}

/** Whether two lists name the same screens, so a poll that changed nothing re-renders nothing. */
export function sameScreens(a: RemoteScreen[], b: RemoteScreen[]): boolean {
  return (
    a.length === b.length &&
    a.every(
      (s, i) =>
        s.surface === b[i].surface &&
        s.device === b[i].device &&
        s.since === b[i].since,
    )
  );
}
