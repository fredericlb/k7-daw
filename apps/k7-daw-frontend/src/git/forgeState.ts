// forgeState.ts — the accounts of Settings → Services as data: what `GET /git/forges` lists, one entry per forge account.
// What a reader gets wrong: GitHub's page still calls `/git/github/*`, an alias of `/git/forges/github.com/*`.

import type { ForgeAccount } from "@studio/k7-wasm/git";

export type {
  ForgeAccount,
  ForgeKind,
  ForgeProbe,
  ForgeStart,
  ForgeStatus,
  PollAnswer as ForgePoll,
} from "@studio/k7-wasm/git";

export const forgejoServers = (a: ForgeAccount[]): ForgeAccount[] =>
  a.filter((x) => x.kind === "forgejo");

/** Whether an account can list repositories: GitHub needs `repo`, a Forgejo server asked its scopes with the sign-in. */
export const canPickFrom = (a: ForgeAccount): boolean =>
  a.available &&
  Boolean(a.login) &&
  (a.kind === "forgejo" || a.scopes.includes("repo"));

/** A refusal that says the sign-in is gone and must be done again, for either forge. */
export const isSignout = (code: string): boolean => code.endsWith("-signout");

/** Servers whose sign-in the front dropped while this page ran; Settings then says "Sign in again". */
export const droppedSignIns = new Set<string>();

/** What a screen calls an account: GitHub by its name, a server by its host (the routes name every Forgejo "Forgejo"). */
export const accountLabel = (a: ForgeAccount): string =>
  a.kind === "forgejo" ? a.id : a.name;
