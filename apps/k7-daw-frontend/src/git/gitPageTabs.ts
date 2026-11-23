// gitPageTabs.ts — which service tab a git page opens on, and the name a clone takes from an address.
// What a reader gets wrong: an address already known (a connected workspace, another device's) always opens Other host.

import { canPickFrom, type ForgeAccount } from "./forgeState";

export type GitPageTab = "github" | "forgejo" | "other";

/** The first account that can list repositories leads; with none, or with an address to show, the ssh form. */
export function firstTab(
  accounts: readonly ForgeAccount[],
  knownUrl: string,
): GitPageTab {
  if (knownUrl.trim()) return "other";
  const pick = accounts.filter(canPickFrom);
  if (pick.some((a) => a.kind === "github")) return "github";
  if (pick.some((a) => a.kind === "forgejo")) return "forgejo";
  return "other";
}

/** The repository's own name in an ssh or file address (`git@host:me/songs.git` → `songs`), or "". */
export function nameFromUrl(url: string): string {
  const tail = url.trim().replace(/\/+$/, "").split(/[/:]/).pop() ?? "";
  return tail.replace(/\.git$/i, "");
}
