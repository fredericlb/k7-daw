// gitState.ts — what the workspace bar offers, read off the route's state by Rust (`barView`); the menu's lines are worded here.
// What a reader gets wrong: Push is never offered over a branch that is behind, and Pull never over changes.

import {
  type BarView,
  barView as barOf,
  type MenuView,
  menuView as menuOf,
  remoteName as nameOf,
} from "@studio/k7-wasm/git";
import type { GitState } from "./gitTypes";

export type { BarView, MenuView };

export const barView = (s: GitState | null): BarView => barOf(s);

/** Whether this state locks the writers of its workspace (a merge waits for choices). */
export const lockedBy = (s: GitState | null): boolean => barOf(s).readOnly;

/** A remote as a person reads it: the host and the repository, never the user or the scheme (`github.com/o/r`). */
export const remoteName = nameOf;

/** What the workspace menu offers on the git state: connect when there is no remote, else say it and offer to change it. */
export const menuView = (s: GitState | null): MenuView => menuOf(s);

/** The git lines of the workspace menu, in order: where it is connected, then its Sync settings. */
export const gitMenuEntries = (
  v: MenuView,
  // The demos are put back as shipped at each start, so they are never offered a remote.
  demos = false,
): ("connected" | "sync")[] =>
  demos
    ? v.connected
      ? ["connected"]
      : []
    : v.connected
      ? ["connected", "sync"]
      : ["sync"];
