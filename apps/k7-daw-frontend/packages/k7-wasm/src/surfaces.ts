// surfaces.ts — the Surfaces tab's rows, the panes remotes hold, what a History row lights, the mastering strip's modules: the `chains` crate's rules.
// A reader would get wrong that a result here is final for the page: devices are sorted by locale and each row's address is spelled by the caller.

import { call } from "./abi.ts";
import type {
  HistoryMarkLists,
  InUse,
  MainSurfaceIds,
  MarkGraph,
  RemoteScreen,
  RowGroup,
  SurfaceBinding,
  SurfaceRowsArgs,
} from "./types/index.ts";

export type {
  HistoryMarkLists,
  InUse,
  MainSurfaceIds,
  RemoteScreen,
  RowBlocked,
  RowGroup,
  RowPeer,
  RowPeerAnswer,
  SurfaceRow,
  SurfaceRowsArgs,
} from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `surfaces.${op}`, args });

/** One group per device, in the order the peers came, each row an address (as its pieces) or the one reason it is not. */
export const surfaceGroupsOf = (args: SurfaceRowsArgs): RowGroup[] =>
  ask("rows", args);

/** The device holding each control the remotes show; where two show one surface, the one that came first. */
export const inUseBy = (screens: RemoteScreen[], ids: MainSurfaceIds): InUse =>
  ask("inUse", { screens, ids });

/** The next pane after `from` in `order`, wrapping, that is not blocked; null when every other one is. */
export const nextFree = (
  order: readonly string[],
  from: string,
  blocked: readonly string[],
): string | null => ask("nextFree", { order, from, blocked });

/** Whether two lists name the same screens, so a poll that changed nothing re-renders nothing. */
export const sameScreens = (a: RemoteScreen[], b: RemoteScreen[]): boolean =>
  ask("sameScreens", { a, b });

/** What one History row touched, from its project-relative `paths` and a journal step's `label`. */
export const historyMarksOf = (
  paths: readonly string[],
  label: string,
  graph?: MarkGraph,
): HistoryMarkLists => ask("history", { paths, label, graph });

/** The modules a `mastering` mapping names, deduplicated, in the order met. */
export const masterModules = (
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
): string[] => ask("masterModules", { mapping });
