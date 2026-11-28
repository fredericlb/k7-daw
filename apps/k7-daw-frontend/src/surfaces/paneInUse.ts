// paneInUse.ts — which of THIS device's panes a remote shows, so this device cannot open it: the rules are Rust's (the chains crate).
// A surface is a pane here by its declared id: timeline and turntables are panes, the picture is a window.
// What a reader gets wrong: a mastering surface has no pane on this device, so nothing of it is ever blocked.

import {
  inUseBy as inUse,
  type MainSurfaceIds,
  nextFree as next,
  type RemoteScreen,
  sameScreens,
} from "@studio/k7-wasm/surfaces";

export type { MainSurfaceIds, RemoteScreen };
export { sameScreens };

/** The three controls of the main device a remote's surface can be. */
export type Blockable = keyof MainSurfaceIds;

/** The device holding each control the remotes show; where two show one surface, the one that came first. */
export const inUseBy = (
  screens: RemoteScreen[],
  ids: MainSurfaceIds,
): Partial<Record<Blockable, string>> => inUse(screens, ids);

/** The next pane after `from` in `order`, wrapping, that is not blocked; null when every other one is. */
export const nextFree = (
  order: readonly string[],
  from: string,
  blocked: ReadonlySet<string>,
): string | null => next(order, from, [...blocked]);
